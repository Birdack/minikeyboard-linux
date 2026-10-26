//! Device access through Linux hidraw (no libusb, no kernel driver detach).

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::protocol::{self as proto, Action, DeviceInfo, Error, Layout, Packet};

/// Every VID/PID probed by the vendor software (Widget::timeoutSlot)
pub const VENDOR_IDS: [u16; 2] = [0x1189, 0x514C];
pub const PRODUCT_IDS: [u16; 8] = [0x8842, 0x8840, 0x8830, 0x8831, 0x8832, 0x8833, 0x8850, 0x8851];

#[derive(Debug)]
pub enum DevError {
    PermissionDenied(String),
    Disconnected,
    /// technical detail, in English
    Io(String),
}

impl From<DevError> for Error {
    fn from(e: DevError) -> Self {
        match e {
            DevError::PermissionDenied(path) => Error::new("permission").arg(path),
            DevError::Disconnected => Error::new("disconnected"),
            DevError::Io(detail) => Error::new("io").arg(detail),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub interface: i32,
    pub name: String,
    /// sysfs path of the USB device, groups its interfaces
    pub usb_path: String,
    /// canonical sysfs dir of the HID device, new on every enumeration
    #[serde(skip)]
    pub hid_dir: String,
    pub report_ids: Vec<u8>,
    pub vendor_page: bool,
}

impl Node {
    /// How likely this interface is the configuration channel.
    pub fn score(&self) -> u32 {
        let mut s = 0;
        if self.report_ids.contains(&proto::REPORT_ID) {
            s += 4;
        }
        if self.vendor_page {
            s += 2;
        }
        if self.interface == 0 {
            s += 1; // what the vendor software opens
        }
        s
    }
}

/// Report ids and whether a vendor defined usage page is used.
pub fn parse_report_descriptor(data: &[u8]) -> (Vec<u8>, bool) {
    let (mut ids, mut vendor, mut i) = (Vec::new(), false, 0usize);
    while i < data.len() {
        let prefix = data[i];
        if prefix == 0xFE {
            // long item
            i += 3 + *data.get(i + 1).unwrap_or(&0) as usize;
            continue;
        }
        let size = [0usize, 1, 2, 4][(prefix & 0x03) as usize];
        let mut value = 0u32;
        for (n, b) in data.iter().skip(i + 1).take(size).enumerate() {
            value |= (*b as u32) << (8 * n);
        }
        match prefix & 0xFC {
            0x84 => ids.push(value as u8),
            0x04 if value >= 0xFF00 => vendor = true,
            _ => {}
        }
        i += 1 + size;
    }
    ids.sort();
    ids.dedup();
    (ids, vendor)
}

pub fn list_nodes(sysfs: &Path) -> Vec<Node> {
    let mut entries: Vec<_> = fs::read_dir(sysfs).into_iter().flatten().flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    let mut nodes = Vec::new();
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("hidraw") {
            continue;
        }
        let dev = entry.path().join("device");
        let uevent = fs::read_to_string(dev.join("uevent")).unwrap_or_default();
        let props: HashMap<&str, &str> = uevent.lines().filter_map(|l| l.split_once('=')).collect();
        let ids: Vec<u32> = props
            .get("HID_ID")
            .map(|v| v.split(':').filter_map(|x| u32::from_str_radix(x, 16).ok()).collect())
            .unwrap_or_default();
        let [bus, vid, pid] = ids[..] else { continue };
        let (vid, pid) = (vid as u16, pid as u16);
        if bus != 0x03 || !VENDOR_IDS.contains(&vid) || !PRODUCT_IDS.contains(&pid) {
            continue;
        }
        let Ok(hid_dir) = fs::canonicalize(&dev) else { continue };
        let intf_dir = hid_dir.parent().unwrap_or(&hid_dir).to_path_buf();
        let interface = fs::read_to_string(intf_dir.join("bInterfaceNumber"))
            .ok()
            .and_then(|s| i32::from_str_radix(s.trim(), 16).ok())
            .unwrap_or(-1);
        let desc = fs::read(hid_dir.join("report_descriptor")).unwrap_or_default();
        let (report_ids, vendor_page) = parse_report_descriptor(&desc);
        nodes.push(Node {
            path: format!("/dev/{name}"),
            vid,
            pid,
            interface,
            name: props.get("HID_NAME").unwrap_or(&"").to_string(),
            usb_path: intf_dir.parent().unwrap_or(&intf_dir).to_string_lossy().into(),
            hid_dir: hid_dir.to_string_lossy().into(),
            report_ids,
            vendor_page,
        });
    }
    nodes
}

/// Best hidraw node of every connected macropad.
pub fn find_keyboards() -> Vec<Node> {
    let mut best: BTreeMap<String, Node> = BTreeMap::new();
    for node in list_nodes(Path::new("/sys/class/hidraw")) {
        let better = match best.get(&node.usb_path) {
            None => true,
            Some(cur) => (node.score(), -node.interface) > (cur.score(), -cur.interface),
        };
        if better {
            best.insert(node.usb_path.clone(), node);
        }
    }
    best.into_values().collect()
}

pub trait Transport: Send {
    fn node(&self) -> &Node;
    fn write(&mut self, packet: &Packet) -> Result<(), DevError>;
    /// One input report, or None after `timeout`.
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, DevError>;
    /// False once the kernel reports the device gone.
    fn alive(&self) -> bool {
        true
    }
}

pub struct Hidraw {
    node: Node,
    file: File,
}

/// Errno values the kernel returns for a device that was unplugged or reset.
fn is_gone(errno: i32) -> bool {
    matches!(errno, libc::ENODEV | libc::EIO | libc::ESHUTDOWN | libc::EPROTO | libc::EPIPE | libc::ETIMEDOUT)
}

fn io_error(e: std::io::Error, what: &str) -> DevError {
    match e.raw_os_error() {
        Some(n) if is_gone(n) => DevError::Disconnected,
        _ => DevError::Io(format!("{what}: {e}")),
    }
}

/// poll(2) on the device, retried when interrupted by a signal.
fn poll_fd(file: &File, events: libc::c_short, ms: i32) -> Result<libc::c_short, std::io::Error> {
    loop {
        let mut pfd = libc::pollfd { fd: file.as_raw_fd(), events, revents: 0 };
        // SAFETY: pfd is a valid pollfd for the duration of the call
        let r = unsafe { libc::poll(&mut pfd, 1, ms) };
        if r >= 0 {
            return Ok(pfd.revents);
        }
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() != Some(libc::EINTR) {
            return Err(e);
        }
    }
}

impl Hidraw {
    pub fn open(node: Node) -> Result<Self, DevError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&node.path)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::PermissionDenied => DevError::PermissionDenied(node.path.clone()),
                // the node vanished between enumeration and open, or udev is not done yet
                std::io::ErrorKind::NotFound => DevError::Disconnected,
                _ => io_error(e, &node.path),
            })?;
        Ok(Self { node, file })
    }
}

impl Transport for Hidraw {
    fn node(&self) -> &Node {
        &self.node
    }

    fn write(&mut self, packet: &Packet) -> Result<(), DevError> {
        log::debug!("OUT {}", hex(packet));
        let n = self.file.write(packet).map_err(|e| io_error(e, "write"))?;
        if n != packet.len() {
            return Err(DevError::Io("short write".into()));
        }
        Ok(())
    }

    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, DevError> {
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        let revents = poll_fd(&self.file, libc::POLLIN, ms).map_err(|e| io_error(e, "poll"))?;
        if revents == 0 {
            return Ok(None);
        }
        if revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 && revents & libc::POLLIN == 0 {
            return Err(DevError::Disconnected);
        }
        let mut buf = vec![0u8; 128];
        match self.file.read(&mut buf) {
            Ok(n) => {
                buf.truncate(n);
                log::debug!("IN  {}", hex(&buf));
                Ok(Some(buf))
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(io_error(e, "read")),
        }
    }

    fn alive(&self) -> bool {
        match poll_fd(&self.file, 0, 0) {
            Ok(revents) => revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) == 0,
            Err(_) => false,
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

/// Simulated macropad for --demo and the tests; keeps what was stored.
pub struct FakeDevice {
    node: Node,
    keys: u8,
    knobs: u8,
    /// answers the identify request
    pub identifies: bool,
    pub readable: bool,
    pub flash: HashMap<(u8, u8), Packet>,
    pending: HashMap<(u8, u8), Packet>,
    pub leds: HashMap<u8, u8>,
    replies: Vec<Vec<u8>>,
}

impl FakeDevice {
    pub fn new(keys: u8, knobs: u8) -> Self {
        Self {
            node: Node {
                path: "demo".into(),
                vid: 0x1189,
                pid: 0x8840,
                interface: 0,
                name: "Demo".into(),
                usb_path: "demo".into(),
                hid_dir: "demo".into(),
                report_ids: vec![3],
                vendor_page: true,
            },
            keys,
            knobs,
            identifies: true,
            readable: true,
            flash: HashMap::new(),
            pending: HashMap::new(),
            leds: HashMap::new(),
            replies: Vec::new(),
        }
    }
}

impl Transport for FakeDevice {
    fn node(&self) -> &Node {
        &self.node
    }

    fn write(&mut self, p: &Packet) -> Result<(), DevError> {
        assert_eq!(p[0], proto::REPORT_ID);
        match p[1] {
            0xFB if self.identifies => {
                let mut r = vec![3, 0xFB, self.keys, self.knobs];
                r.resize(64, 0);
                self.replies.push(r);
            }
            0xFA if self.readable => {
                let info = DeviceInfo { keys: self.keys, knobs: self.knobs, variant: 0 };
                let layer = p[4];
                for id in proto::layout_for_device(info, self.node.pid).key_ids() {
                    let stored = self.flash.get(&(layer, id)).copied().unwrap_or_else(|| {
                        let mut d = [0u8; proto::PACKET_SIZE];
                        d[..5].copy_from_slice(&[3, 0xFD, id, layer, 1]);
                        d
                    });
                    let mut r = vec![3, 0xFA];
                    r.extend_from_slice(&stored[2..]);
                    self.replies.push(r);
                }
            }
            0xFD if p[2] == 0xFE && p[3] == 0xFF => {
                self.flash.extend(self.pending.drain());
            }
            0xFD => {
                self.pending.insert((p[3], p[2]), *p);
            }
            0xFE => {
                self.leds.insert(p[3], p[12]);
            }
            _ => {}
        }
        Ok(())
    }

    fn read(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>, DevError> {
        Ok((!self.replies.is_empty()).then(|| self.replies.remove(0)))
    }
}

/// High level operations on an open device.
pub struct Keyboard {
    pub dev: Box<dyn Transport>,
    pub write_pause: Duration,
    /// Widget::HID_write sleeps 1 s after storing a layer
    pub store_pause: Duration,
}

impl Keyboard {
    pub fn new(dev: Box<dyn Transport>) -> Self {
        Self { dev, write_pause: Duration::from_millis(2), store_pause: Duration::from_secs(1) }
    }

    fn drain(&mut self) -> Result<(), DevError> {
        while self.dev.read(Duration::ZERO)?.is_some() {}
        Ok(())
    }

    fn wait_reply(&mut self, accept: impl Fn(&[u8]) -> bool, timeout: Duration) -> Result<Option<Vec<u8>>, DevError> {
        let end = Instant::now() + timeout;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(None);
            }
            match self.dev.read(left)? {
                None => return Ok(None),
                Some(r) if r.first() == Some(&proto::REPORT_ID) && accept(&r) => return Ok(Some(r)),
                Some(_) => {} // other input reports (key presses) are ignored
            }
        }
    }

    pub fn identify(&mut self) -> Result<Option<DeviceInfo>, DevError> {
        self.drain()?;
        self.dev.write(&proto::identify_packet())?;
        let reply = self.wait_reply(|r| r.len() > 3 && r[1] == 0xFB, Duration::from_millis(600))?;
        Ok(reply.and_then(|r| proto::parse_identify(&r)))
    }

    /// Actions of one layer; keys that did not answer are absent.
    pub fn read_layer(&mut self, layout: &Layout, layer: u8) -> Result<BTreeMap<u8, Action>, DevError> {
        self.drain()?;
        self.dev.write(&proto::read_layer_packet(layout.keys, layout.knobs, layer))?;
        let mut wanted: Vec<u8> = layout.key_ids();
        let mut out = BTreeMap::new();
        while !wanted.is_empty() {
            let Some(r) = self.wait_reply(|r| r.len() > 50 && r[1] == 0xFA, Duration::from_millis(500))? else {
                break;
            };
            if let Some((id, l, action)) = proto::parse_key_report(&r) {
                if l == layer {
                    if let Some(pos) = wanted.iter().position(|&w| w == id) {
                        wanted.remove(pos);
                        out.insert(id, action);
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn write_plan(
        &mut self,
        plan: &[(u8, Vec<Packet>)],
        mut progress: impl FnMut(usize, usize),
    ) -> Result<(), DevError> {
        let total = plan.iter().map(|(_, p)| p.len()).sum::<usize>().max(1);
        let mut done = 0;
        for (_layer, packets) in plan {
            for p in packets {
                self.dev.write(p)?;
                done += 1;
                progress(done, total);
                let store = p[1..4] == [0xFD, 0xFE, 0xFF];
                sleep(if store { self.store_pause } else { self.write_pause });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{parse_action, write_plan, KeyChange};

    fn h(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    #[test]
    fn descriptor_parser() {
        let vendor = h("06 00 ff 09 01 a1 01 85 03 15 00 26 ff 00 75 08 95 40 09 01 91 02 c0");
        let kbd = h("05 01 09 06 a1 01 05 07 19 e0 29 e7 15 00 25 01 75 01 95 08 81 02 c0");
        assert_eq!(parse_report_descriptor(&vendor), (vec![3], true));
        assert_eq!(parse_report_descriptor(&kbd), (vec![], false));
    }

    #[test]
    fn sysfs_discovery_prefers_vendor_interface() {
        let root = std::env::temp_dir().join(format!("mk-sysfs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mk = |n: u32, intf: u32, desc: &[u8]| {
            let usb = root.join("devices/usb1/1-2");
            let idir = usb.join(format!("1-2:1.{intf}"));
            let hid = idir.join(format!("0003:1189:8840.000{n}"));
            fs::create_dir_all(&hid).unwrap();
            fs::write(idir.join("bInterfaceNumber"), format!("0{intf}\n")).unwrap();
            fs::write(hid.join("uevent"), "HID_ID=0003:00001189:00008840\nHID_NAME=Pad\n").unwrap();
            fs::write(hid.join("report_descriptor"), desc).unwrap();
            let cls = root.join(format!("class/hidraw/hidraw{n}"));
            fs::create_dir_all(&cls).unwrap();
            std::os::unix::fs::symlink(&hid, cls.join("device")).unwrap();
        };
        mk(1, 0, &h("05 01 09 06 a1 01 c0"));
        mk(2, 1, &h("06 00 ff 09 01 a1 01 85 03 c0"));
        let nodes = list_nodes(&root.join("class/hidraw"));
        assert_eq!(nodes.len(), 2);
        let best = nodes.iter().max_by_key(|n| n.score()).unwrap();
        assert_eq!((best.path.as_str(), best.interface), ("/dev/hidraw2", 1));
        assert!(best.hid_dir.ends_with("/1-2:1.1/0003:1189:8840.0002"), "{}", best.hid_dir);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unplug_errors_mean_disconnected() {
        for n in [libc::ENODEV, libc::EIO, libc::ESHUTDOWN, libc::EPROTO, libc::EPIPE, libc::ETIMEDOUT] {
            assert!(matches!(io_error(std::io::Error::from_raw_os_error(n), "x"), DevError::Disconnected), "{n}");
        }
        for n in [libc::EINVAL, libc::EBUSY, libc::EAGAIN] {
            assert!(matches!(io_error(std::io::Error::from_raw_os_error(n), "x"), DevError::Io(_)), "{n}");
        }
    }

    #[test]
    fn device_errors_have_codes() {
        let code = |e: DevError| Error::from(e);
        assert_eq!(code(DevError::PermissionDenied("/dev/hidraw3".into())), Error::new("permission").arg("/dev/hidraw3"));
        assert_eq!(code(DevError::Disconnected), Error::new("disconnected"));
        assert_eq!(code(io_error(std::io::Error::from_raw_os_error(libc::EBUSY), "write")).code, "io");
    }

    #[test]
    fn missing_node_is_not_ready() {
        let mut node = FakeDevice::new(1, 0).node;
        node.path = "/dev/hidraw-does-not-exist".into();
        assert!(matches!(Hidraw::open(node), Err(DevError::Disconnected)));
    }

    #[test]
    fn fake_write_then_read_back() {
        let mut kb = Keyboard::new(Box::new(FakeDevice::new(4, 1)));
        kb.store_pause = Duration::ZERO;
        let info = kb.identify().unwrap().unwrap();
        let layout = proto::layout_for_device(info, 0x8840);
        let changes = vec![
            KeyChange { layer: 1, key_id: 2, action: parse_action("ctrl-z", 0).unwrap() },
            KeyChange { layer: 2, key_id: 0x12, action: parse_action("volumeup", 0).unwrap() },
            KeyChange { layer: 3, key_id: 4, action: parse_action("a,b", 500).unwrap() },
        ];
        let plan = write_plan(&changes, &[]).unwrap();
        let mut steps = 0;
        kb.write_plan(&plan, |d, t| {
            steps = d;
            assert!(d <= t)
        })
        .unwrap();
        assert_eq!(steps, 7); // 3 keys + 1 delay copy + 3 stores
        for c in &changes {
            let layer = kb.read_layer(&layout, c.layer).unwrap();
            assert_eq!(layer.len(), layout.key_ids().len());
            assert_eq!(layer[&c.key_id], c.action);
        }
    }
}
