//! Wire protocol of the MINI_KEYBOARD macropads (0x884x family).
//!
//! Reconstructed from the vendor's macOS application (Widget::HID_write,
//! Widget::read_Hidkey_Data, Widget::Identify_KeyBoard_style, Widget::Set*Key).
//! Every packet is an HID output report with report id 3, 65 bytes with the id:
//!
//! ```text
//! 03 FB FB FB                          identify -> [2]=keys [3]=knobs [4]=variant
//! 03 FA keys knobs layer               read one layer, one reply per key
//! 03 FD key layer mode dlo dhi 0 0 0 n (mod code)*   program one key
//! 03 FE B0 layer 08 0 0 0 0 0 01 00 (color<<4|mode)  LED of one layer
//! 03 FD FE FF                          store the layer just sent
//! ```
//!
//! Keys are numbered from 1, knob actions (ccw, press, cw) from 0x10. The
//! vendor software has 24 control ids, so the 12+4 and 21+1 models number
//! their knobs right after the last key.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::keycodes as kc;

pub const REPORT_ID: u8 = 0x03;
pub const PACKET_SIZE: usize = 65;
pub const LAYERS: u8 = 3;
pub const KNOB_BASE: u8 = 0x10;
pub const MAX_KEY_ID: u8 = 24;

pub const MODE_KEYS: u8 = 0x01;
pub const MODE_MEDIA: u8 = 0x02;
pub const MODE_MOUSE: u8 = 0x03;
pub const MODE_DELAY: u8 = 0x05;
pub const MODE_LED: u8 = 0x08;

pub const MAX_CHORDS: usize = 17;
pub const MAX_DELAY_MS: u16 = 6000;

pub type Packet = [u8; PACKET_SIZE];

/// A failure the UI shows in the user's language: `code` picks the message
/// (`err.<code>` in ui/i18n.js), `args` fill its `{0}`, `{1}`… placeholders.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Error {
    pub code: &'static str,
    pub args: Vec<String>,
    /// id of the key the error is about, when checking a list of keys
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<u8>,
}

impl Error {
    pub fn new(code: &'static str) -> Self {
        Self { code, args: Vec::new(), key: None }
    }

    pub fn arg(mut self, arg: impl ToString) -> Self {
        self.args.push(arg.to_string());
        self
    }

    pub fn in_key(mut self, key_id: u8) -> Self {
        self.key = Some(key_id);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    None,
    Keys,
    Media,
    Mouse,
}

/// What a key does.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Action {
    pub kind: Kind,
    /// [modifiers, keycode] pairs for `keys`
    pub chords: Vec<[u8; 2]>,
    /// ms between chords, `keys` only
    pub delay: u16,
    /// consumer usage for `media`
    pub media: u16,
    pub buttons: u8,
    /// 0, WHEEL_UP or WHEEL_DOWN
    pub wheel: u8,
    pub mouse_mod: u8,
}

/// An action plus its textual syntax, as sent to the UI (which builds the labels).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionView {
    #[serde(flatten)]
    pub action: Action,
    pub text: String,
}

impl Action {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.kind == Kind::None
    }

    /// Serialise to the macropad_tool compatible syntax.
    pub fn text(&self) -> String {
        match self.kind {
            Kind::Keys => self
                .chords
                .iter()
                .map(|[m, c]| {
                    let mut parts: Vec<String> =
                        kc::modifier_names(*m).into_iter().map(String::from).collect();
                    if *c != 0 {
                        parts.push(kc::key_name(*c));
                    }
                    parts.join("-")
                })
                .collect::<Vec<_>>()
                .join(","),
            Kind::Media => kc::media_name(self.media),
            Kind::Mouse => {
                let mut parts: Vec<&str> = kc::modifier_names(self.mouse_mod);
                parts.extend(kc::MOUSE_BUTTONS.iter().filter(|b| self.buttons & b.0 != 0).map(|b| b.1));
                match self.wheel {
                    kc::WHEEL_UP => parts.push("wheelup"),
                    kc::WHEEL_DOWN => parts.push("wheeldown"),
                    _ => {}
                }
                parts.join("-")
            }
            Kind::None => String::new(),
        }
    }

    pub fn view(self) -> ActionView {
        ActionView { text: self.text(), action: self }
    }

    /// Reject values the device cannot store.
    pub fn validate(&self) -> Result<(), Error> {
        match self.kind {
            Kind::Keys if self.chords.is_empty() => Err(Error::new("empty_sequence")),
            Kind::Keys if self.chords.len() > MAX_CHORDS => Err(Error::new("too_many_chords").arg(MAX_CHORDS)),
            _ if self.delay > MAX_DELAY_MS => Err(Error::new("delay_too_long").arg(MAX_DELAY_MS)),
            Kind::Media if self.media == 0 => Err(Error::new("media_missing")),
            Kind::Mouse if self.buttons == 0 && self.wheel == 0 => Err(Error::new("mouse_empty")),
            Kind::Mouse if self.wheel != 0 && self.wheel != kc::WHEEL_UP && self.wheel != kc::WHEEL_DOWN => {
                Err(Error::new("wheel_invalid"))
            }
            _ => Ok(()),
        }
    }

    /// Empty sequences / mouse actions collapse to "no function".
    pub fn normalized(mut self) -> Self {
        let empty = match self.kind {
            Kind::Keys => self.chords.is_empty(),
            Kind::Media => self.media == 0,
            Kind::Mouse => self.buttons == 0 && self.wheel == 0,
            Kind::None => true,
        };
        if empty {
            return Self::none();
        }
        if self.kind != Kind::Keys {
            self.delay = 0;
        }
        self
    }
}

/// Parse the textual syntax: `ctrl-c,ctrl-v`, `volumeup`, `ctrl-wheelup`, `<110>`.
pub fn parse_action(text: &str, delay: u16) -> Result<Action, Error> {
    let text: String = text.trim().to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if text.is_empty() {
        return Ok(Action::none());
    }
    let chords: Vec<&str> = text.split(',').collect();
    if chords.iter().any(|c| c.is_empty()) {
        return Err(Error::new("empty_chord"));
    }
    let tokens: Vec<&str> = chords[0].split('-').collect();

    if chords.len() == 1 {
        if let Some(code) = kc::media_by_name(tokens[tokens.len() - 1]) {
            if tokens.len() > 1 {
                return Err(Error::new("media_with_modifiers"));
            }
            return Ok(Action { kind: Kind::Media, media: code, ..Default::default() });
        }
        if let Some(inner) = text.strip_prefix("media<").and_then(|t| t.strip_suffix('>')) {
            let code: u32 = inner.parse().map_err(|_| Error::new("media_code_invalid").arg(&text))?;
            if code == 0 || code > 0xFFFF {
                return Err(Error::new("media_code_range").arg(&text));
            }
            return Ok(Action { kind: Kind::Media, media: code as u16, ..Default::default() });
        }
    }

    let is_mouse = |t: &str| matches!(t, "click" | "rclick" | "mclick" | "wheelup" | "wheeldown");
    if chords.iter().any(|c| c.split('-').any(is_mouse)) {
        if chords.len() > 1 {
            return Err(Error::new("mouse_in_sequence"));
        }
        let mut action = Action { kind: Kind::Mouse, ..Default::default() };
        for t in tokens {
            if let Some(b) = kc::MOUSE_BUTTONS.iter().find(|b| b.1 == t) {
                action.buttons |= b.0;
            } else if t == "wheelup" || t == "wheeldown" {
                if action.wheel != 0 {
                    return Err(Error::new("wheel_twice"));
                }
                action.wheel = if t == "wheelup" { kc::WHEEL_UP } else { kc::WHEEL_DOWN };
            } else if let Some(bit) = kc::modifier_by_name(t) {
                action.mouse_mod |= bit;
            } else {
                return Err(Error::new("mouse_token_invalid").arg(t));
            }
        }
        return Ok(action);
    }

    let mut out = Vec::new();
    for chord in &chords {
        let (mut mods, mut code) = (0u8, 0u8);
        for t in chord.split('-') {
            if let Some(bit) = kc::modifier_by_name(t) {
                mods |= bit;
            } else if kc::media_by_name(t).is_some() {
                return Err(Error::new("media_combined"));
            } else {
                if code != 0 {
                    return Err(Error::new("one_key_per_chord").arg(chord));
                }
                code = parse_key_token(t)?;
            }
        }
        out.push([mods, code]);
    }
    let action = Action { kind: Kind::Keys, chords: out, delay, ..Default::default() };
    action.validate()?;
    Ok(action)
}

fn parse_key_token(token: &str) -> Result<u8, Error> {
    if let Some(inner) = token.strip_prefix('<').and_then(|t| t.strip_suffix('>')) {
        return match inner.parse::<u16>() {
            Ok(c) if (1..256).contains(&c) => Ok(c as u8),
            _ => Err(Error::new("key_code_range").arg(token)),
        };
    }
    kc::key_by_name(token).ok_or_else(|| Error::new("unknown_key").arg(token))
}

fn packet(head: &[u8]) -> Packet {
    let mut buf = [0u8; PACKET_SIZE];
    buf[0] = REPORT_ID;
    buf[1..1 + head.len()].copy_from_slice(head);
    buf
}

pub fn identify_packet() -> Packet {
    packet(&[0xFB, 0xFB, 0xFB])
}

pub fn read_layer_packet(keys: u8, knobs: u8, layer: u8) -> Packet {
    packet(&[0xFA, keys, knobs, layer])
}

pub fn store_packet() -> Packet {
    packet(&[0xFD, 0xFE, 0xFF])
}

pub fn led_packet(layer: u8, mode: u8, color: u8) -> Packet {
    let mut buf = packet(&[0xFE, 0xB0, layer, MODE_LED]);
    buf[10] = 0x01;
    buf[12] = ((color & 0x0F) << 4) | (mode & 0x0F);
    buf
}

/// Packets that program one key; a delayed sequence needs a second one.
pub fn key_packets(key_id: u8, layer: u8, action: &Action) -> Vec<Packet> {
    let mut buf = packet(&[0xFD, key_id, layer, MODE_KEYS]);
    match action.kind {
        Kind::Keys => {
            buf[10] = action.chords.len().min(MAX_CHORDS) as u8;
            for (i, [m, c]) in action.chords.iter().take(MAX_CHORDS).enumerate() {
                buf[11 + 2 * i] = *m;
                buf[12 + 2 * i] = *c;
            }
        }
        Kind::Media => {
            // Widget::SetMulKey: 16 bit usage little endian, length always 2
            buf[4] = MODE_MEDIA;
            buf[10] = 0x02;
            buf[11..13].copy_from_slice(&action.media.to_le_bytes());
        }
        Kind::Mouse => {
            buf[4] = MODE_MOUSE;
            buf[10] = 0x01;
            buf[11] = action.mouse_mod;
            buf[12] = action.buttons;
            buf[15] = action.wheel;
        }
        Kind::None => {}
    }
    let mut out = vec![buf];
    if action.kind == Kind::Keys && action.delay > 0 {
        let mut delayed = buf;
        delayed[4] = MODE_DELAY;
        delayed[5..7].copy_from_slice(&action.delay.to_le_bytes());
        out.push(delayed);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceInfo {
    pub keys: u8,
    pub knobs: u8,
    pub variant: u8,
}

pub fn parse_identify(report: &[u8]) -> Option<DeviceInfo> {
    (report.len() >= 4 && report[0] == REPORT_ID).then(|| DeviceInfo {
        keys: report[2],
        knobs: report[3],
        variant: report.get(4).copied().unwrap_or(0),
    })
}

/// Decode a reply to read_layer_packet: (key_id, layer, action).
pub fn parse_key_report(r: &[u8]) -> Option<(u8, u8, Action)> {
    if r.len() < 51 || r[0] != REPORT_ID || r[1] != 0xFA {
        return None;
    }
    let (key_id, layer, mode, count) = (r[2], r[3], r[4], r[10] as usize);
    let action = match mode {
        MODE_MEDIA => Action { kind: Kind::Media, media: u16::from_le_bytes([r[11], r[12]]), ..Default::default() },
        MODE_MOUSE => Action {
            kind: Kind::Mouse,
            mouse_mod: r[11],
            buttons: r[12] & 0x07,
            wheel: if r[15] == kc::WHEEL_UP || r[15] == kc::WHEEL_DOWN { r[15] } else { 0 },
            ..Default::default()
        },
        MODE_KEYS | MODE_DELAY => {
            let chords = (0..count.min(20))
                .map(|i| [r[11 + 2 * i], r[12 + 2 * i]])
                .filter(|[m, c]| *m != 0 || *c != 0)
                .take(MAX_CHORDS)
                .collect();
            let delay = if mode == MODE_DELAY { u16::from_le_bytes([r[5], r[6]]).min(MAX_DELAY_MS) } else { 0 };
            Action { kind: Kind::Keys, chords, delay, ..Default::default() }
        }
        _ => Action::none(),
    };
    Some((key_id, layer, action.normalized()))
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

/// How the UI names a layout; the label itself is built there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutKind {
    /// keys, then knobs with all three controls
    #[default]
    Standard,
    /// Widget::Set_Keyboard_0add1: a full knob plus a wheel without press
    KnobWheel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    /// as reported by the device, also sent back in read requests
    pub keys: u8,
    pub knobs: u8,
    #[serde(default)]
    pub kind: LayoutKind,
    /// (ccw, press, cw) ids per knob; None = control not present
    pub groups: Vec<[Option<u8>; 3]>,
}

/// Ids past 255 (impossible models) come out as None.
fn knob_ids(knob: u8, keys: u8, knobs: u8) -> [Option<u8>; 3] {
    let (keys, knobs, knob) = (keys as u16, knobs as u16, knob as u16);
    let fits = keys < KNOB_BASE as u16 && KNOB_BASE as u16 + 3 * knobs <= MAX_KEY_ID as u16 + 1;
    let base = if fits { KNOB_BASE as u16 } else { keys + 1 } + 3 * knob;
    [0, 1, 2].map(|p| u8::try_from(base + p).ok())
}

/// Widget::Set_Keyboard_0add1: a full knob plus a second one without press.
const KNOB_AND_WHEEL: [[Option<u8>; 3]; 2] = [[Some(0x10), Some(0x11), Some(0x12)], [Some(0x13), None, Some(0x15)]];

impl Layout {
    pub fn new(keys: u8, knobs: u8) -> Self {
        Self {
            keys,
            knobs,
            kind: LayoutKind::Standard,
            groups: (0..knobs).map(|k| knob_ids(k, keys, knobs)).collect(),
        }
    }

    fn knob_and_wheel(keys: u8, knobs: u8) -> Self {
        Self { keys, knobs, kind: LayoutKind::KnobWheel, groups: KNOB_AND_WHEEL.to_vec() }
    }

    pub fn key_ids(&self) -> Vec<u8> {
        let mut ids: Vec<u8> = (1..=self.keys).collect();
        ids.extend(self.groups.iter().flatten().flatten());
        ids
    }

    /// (knob index, part index) of a knob key id, or None for a key.
    #[cfg(test)]
    pub fn knob_part(&self, key_id: u8) -> Option<(usize, usize)> {
        self.groups
            .iter()
            .enumerate()
            .find_map(|(k, g)| g.iter().position(|&id| id == Some(key_id)).map(|p| (k, p)))
    }
}

/// Models the vendor software draws (Widget::Identify_KeyBoard_style).
pub fn known_layouts() -> Vec<Layout> {
    let mut out = vec![Layout::knob_and_wheel(0, 1)];
    for (k, n) in [
        (0, 2), (0, 3), (1, 0), (2, 0), (3, 0), (3, 1), (4, 0), (4, 1), (4, 3), (5, 0), (6, 0),
        (6, 1), (6, 2), (9, 2), (9, 3), (11, 3), (12, 2), (12, 3), (12, 4), (15, 3), (16, 0), (21, 1),
    ] {
        out.push(Layout::new(k, n));
    }
    out
}

pub fn find_layout(keys: u8, knobs: u8) -> Option<Layout> {
    known_layouts().into_iter().find(|l| l.keys == keys && l.knobs == knobs)
}

/// Same choice as Widget::Identify_KeyBoard_style.
pub fn layout_for_device(info: DeviceInfo, pid: u16) -> Layout {
    if (info.keys, info.knobs) == (0, 2) && pid != 0x8850 {
        return Layout::knob_and_wheel(0, 2);
    }
    // unknown combinations are drawn as the 15+3 model by the vendor software
    find_layout(info.keys, info.knobs).unwrap_or_else(|| Layout::new(15, 3))
}

// ---------------------------------------------------------------------------
// Writing
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyChange {
    pub layer: u8,
    pub key_id: u8,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Led {
    pub mode: u8,
    pub color: u8,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LedChange {
    pub layer: u8,
    #[serde(flatten)]
    pub led: Led,
}

/// Packets grouped per layer, each group closed by store_packet().
pub fn write_plan(keys: &[KeyChange], leds: &[LedChange]) -> Result<Vec<(u8, Vec<Packet>)>, Error> {
    let mut plan = Vec::new();
    for layer in 1..=LAYERS {
        let mut packets = Vec::new();
        for change in keys.iter().filter(|c| c.layer == layer) {
            if change.key_id == 0 || change.key_id > MAX_KEY_ID {
                return Err(Error::new("key_id_invalid").arg(change.key_id));
            }
            let action = change.action.clone().normalized();
            action.validate().map_err(|e| e.in_key(change.key_id))?;
            packets.extend(key_packets(change.key_id, layer, &action));
        }
        for led in leds.iter().filter(|l| l.layer == layer) {
            if led.led.mode > 5 || !(1..=7).contains(&led.led.color) {
                return Err(Error::new("led_invalid"));
            }
            packets.push(led_packet(layer, led.led.mode, led.led.color));
        }
        if !packets.is_empty() {
            packets.push(store_packet());
            plan.push((layer, packets));
        }
    }
    Ok(plan)
}

// ---------------------------------------------------------------------------
// Profile files (same format as the Python version)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileKey {
    pub action: String,
    #[serde(default)]
    pub delay: u16,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileLayer {
    #[serde(default)]
    pub keys: BTreeMap<String, FileKey>,
    #[serde(default)]
    pub led: Option<Led>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileFile {
    pub format: String,
    pub version: u32,
    pub keys: u8,
    pub knobs: u8,
    #[serde(default)]
    pub knob_ids: Option<Vec<[Option<u8>; 3]>>,
    pub layers: Vec<FileLayer>,
}

/// Most knobs a profile may declare (the vendor software draws at most 4).
pub const MAX_KNOBS: u8 = 8;

/// Every control id within 1..=MAX_KEY_ID and used once.
fn check_layout(layout: &Layout) -> Result<(), Error> {
    let mut ids = layout.key_ids();
    let in_range = ids.iter().all(|&id| (1..=MAX_KEY_ID).contains(&id));
    let total = ids.len();
    ids.sort_unstable();
    ids.dedup();
    if !in_range || ids.len() != total || layout.groups.iter().any(|g| g.iter().all(Option::is_none)) {
        return Err(Error::new("profile_knob_ids"));
    }
    Ok(())
}

/// Profile as the UI holds it: actions per layer keyed by id.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub layout: Layout,
    /// [layer] -> key id -> action
    pub layers: Vec<BTreeMap<u8, Action>>,
    pub leds: Vec<Option<Led>>,
}

impl ProfileFile {
    pub fn from_profile(p: &Profile) -> Self {
        let layers = (0..LAYERS as usize)
            .map(|i| FileLayer {
                keys: p.layers.get(i).map_or_else(BTreeMap::new, |m| {
                    m.iter()
                        .filter(|(_, a)| !a.is_empty())
                        .map(|(id, a)| (id.to_string(), FileKey { action: a.text(), delay: a.delay }))
                        .collect()
                }),
                led: p.leds.get(i).copied().flatten(),
            })
            .collect();
        Self {
            format: "minikeyboard-profile".into(),
            version: 1,
            keys: p.layout.keys,
            knobs: p.layout.knobs,
            knob_ids: Some(p.layout.groups.clone()),
            layers,
        }
    }

    pub fn into_profile(self) -> Result<Profile, Error> {
        if self.format != "minikeyboard-profile" {
            return Err(Error::new("not_a_profile"));
        }
        if self.keys > MAX_KEY_ID || self.knobs > MAX_KNOBS {
            return Err(Error::new("profile_too_big").arg(self.keys).arg(self.knobs).arg(MAX_KEY_ID).arg(MAX_KNOBS));
        }
        let mut layout = find_layout(self.keys, self.knobs).unwrap_or_else(|| Layout::new(self.keys, self.knobs));
        if let Some(groups) = self.knob_ids {
            // the knob + wheel pads report 1 or 2 knobs but always use these two groups
            let knob_wheel = groups == KNOB_AND_WHEEL;
            if !knob_wheel && groups.len() != self.knobs as usize {
                return Err(Error::new("profile_knob_count"));
            }
            layout.kind = if knob_wheel { LayoutKind::KnobWheel } else { LayoutKind::Standard };
            layout.groups = groups;
        }
        check_layout(&layout)?;
        let mut layers = vec![BTreeMap::new(); LAYERS as usize];
        let mut leds = vec![None; LAYERS as usize];
        for (i, layer) in self.layers.into_iter().take(LAYERS as usize).enumerate() {
            for (id, key) in layer.keys {
                let id: u8 = id.parse().map_err(|_| Error::new("key_id_invalid").arg(&id))?;
                let action = parse_action(&key.action, key.delay).map_err(|e| e.in_key(id))?;
                layers[i].insert(id, action);
            }
            leds[i] = layer.led;
        }
        Ok(Profile { layout, layers, leds })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace().map(|b| u8::from_str_radix(b, 16).unwrap()).collect()
    }

    fn check(text: &str, key: u8, layer: u8, expected: &str) {
        let action = parse_action(text, 0).unwrap();
        let packets = key_packets(key, layer, &action);
        assert_eq!(packets.len(), 1, "{text}");
        let exp = hex(expected);
        assert_eq!(&packets[0][..exp.len()], &exp[..], "{text}");
        assert!(packets[0][exp.len()..].iter().all(|&b| b == 0), "{text}: trailing bytes");
    }

    /// Byte layouts captured from real devices (macropad_tool's tests).
    #[test]
    fn packet_vectors() {
        check("ctrl-a,ctrl-s", 1, 1, "03 fd 01 01 01 00 00 00 00 00 02 01 04 01 16");
        check("shift-p", 6, 1, "03 fd 06 01 01 00 00 00 00 00 01 02 13");
        check("win-enter", 0x11, 3, "03 fd 11 03 01 00 00 00 00 00 01 08 28");
        check("ctrl-shift-v", 1, 1, "03 fd 01 01 01 00 00 00 00 00 01 03 19");
        check("ctrl-alt-delete", 1, 1, "03 fd 01 01 01 00 00 00 00 00 01 05 4c");
        check("ctrl-alt-f3", 1, 1, "03 fd 01 01 01 00 00 00 00 00 01 05 3c");
        check("volumedown", 0x10, 1, "03 fd 10 01 02 00 00 00 00 00 02 ea");
        check("calculator", 1, 1, "03 fd 01 01 02 00 00 00 00 00 02 92 01");
        check("ctrl-wheelup", 1, 2, "03 fd 01 02 03 00 00 00 00 00 01 01 00 00 00 01");
        check("ctrl-wheeldown", 2, 2, "03 fd 02 02 03 00 00 00 00 00 01 01 00 00 00 ff");
        check("click", 1, 2, "03 fd 01 02 03 00 00 00 00 00 01 00 01");
        check("mclick", 2, 2, "03 fd 02 02 03 00 00 00 00 00 01 00 04");
        check("rclick", 3, 2, "03 fd 03 02 03 00 00 00 00 00 01 00 02");
        assert_eq!(&led_packet(3, 3, 6)[..13], &hex("03 fe b0 03 08 00 00 00 00 00 01 00 63")[..]);
        assert_eq!(&store_packet()[..4], &hex("03 fd fe ff")[..]);
        assert_eq!(&identify_packet()[..4], &hex("03 fb fb fb")[..]);
        assert_eq!(&read_layer_packet(12, 2, 3)[..5], &hex("03 fa 0c 02 03")[..]);
        assert_eq!(&key_packets(4, 2, &Action::none())[0][..11], &hex("03 fd 04 02 01 00 00 00 00 00 00")[..]);
    }

    #[test]
    fn delay_sends_mode5_copy() {
        let p = key_packets(1, 1, &parse_action("f,o,o", 1000).unwrap());
        assert_eq!(p.len(), 2);
        assert_eq!((p[0][4], p[0][5], p[0][6]), (MODE_KEYS, 0, 0));
        assert_eq!((p[1][4], p[1][5], p[1][6]), (MODE_DELAY, 0xE8, 0x03));
        assert_eq!(p[0][10..], p[1][10..]);
    }

    #[test]
    fn syntax_roundtrip_and_errors() {
        for t in ["ctrl-c,ctrl-v", "shift", "win-rctrl-backspace", "volumeup", "ctrl-wheeldown",
                  "click-rclick", "ctrl-shift-click", "<110>", "f13,numpad5"] {
            let a = parse_action(t, 0).unwrap();
            assert_eq!(parse_action(&a.text(), 0).unwrap(), a, "{t}");
        }
        for t in ["ctrl-a-b", "volumeup,a", "ctrl-volumeup", "foo", "click,click", "a,,b",
                  "media<70000>", "media<x>", &vec!["a"; 18].join(",")] {
            assert!(parse_action(t, 0).is_err(), "{t}");
        }
        assert_eq!(parse_action("cmd-opt-esc", 0), parse_action("win-alt-escape", 0));
        let odd = Action { kind: Kind::Mouse, buttons: 1, mouse_mod: 0x0B, ..Default::default() };
        assert_eq!(parse_action(&odd.text(), 0).unwrap(), odd);
    }

    #[test]
    fn reply_is_inverse_of_write() {
        for (t, d) in [("ctrl-a,ctrl-s", 0), ("h,e,l,l,o", 250), ("mute", 0), ("webpageback", 0),
                       ("alt-wheelup", 0), ("mclick", 0)] {
            let a = parse_action(t, d).unwrap();
            let p = key_packets(0x12, 3, &a);
            let mut reply = p[p.len() - 1];
            reply[1] = 0xFA;
            assert_eq!(parse_key_report(&reply), Some((0x12, 3, a)), "{t}");
        }
    }

    #[test]
    fn knob_ids_match_vendor_layouts() {
        assert_eq!(Layout::new(6, 1).key_ids(), vec![1, 2, 3, 4, 5, 6, 16, 17, 18]);
        assert_eq!(Layout::new(15, 3).key_ids()[15..], (16..=24).collect::<Vec<u8>>()[..]);
        assert_eq!(Layout::new(4, 3).key_ids()[4..], (16..=24).collect::<Vec<u8>>()[..]);
        assert_eq!(Layout::new(12, 4).groups[0], [Some(13), Some(14), Some(15)]);
        assert_eq!(Layout::new(12, 4).key_ids(), (1..=24).collect::<Vec<u8>>());
        assert_eq!(Layout::new(21, 1).key_ids(), (1..=24).collect::<Vec<u8>>());
        assert_eq!(Layout::new(21, 1).knob_part(23), Some((0, 1)));
        assert_eq!(find_layout(0, 1).unwrap().key_ids(), vec![16, 17, 18, 19, 21]);
        for l in known_layouts() {
            let ids = l.key_ids();
            let mut uniq = ids.clone();
            uniq.sort();
            uniq.dedup();
            assert_eq!(uniq.len(), ids.len(), "{l:?}");
            assert!(ids.iter().all(|&i| i <= MAX_KEY_ID), "{l:?}");
        }
        let info = |k, n| DeviceInfo { keys: k, knobs: n, variant: 0 };
        assert_eq!(layout_for_device(info(0, 2), 0x8850), Layout::new(0, 2));
        assert_eq!(layout_for_device(info(0, 2), 0x8840).key_ids(), vec![16, 17, 18, 19, 21]);
        assert_eq!(layout_for_device(info(7, 7), 0x8840), Layout::new(15, 3));
        assert!(find_layout(4, 2).is_none());
    }

    #[test]
    fn plan_and_profile_file() {
        let keys = vec![KeyChange { layer: 2, key_id: 3, action: parse_action("ctrl-c", 0).unwrap() }];
        let leds = vec![LedChange { layer: 2, led: Led { mode: 1, color: 4 } }];
        let plan = write_plan(&keys, &leds).unwrap();
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].0, 2);
        assert_eq!(plan[0].1.iter().map(|p| p[1]).collect::<Vec<_>>(), vec![0xFD, 0xFE, 0xFD]);
        assert!(write_plan(&[], &[LedChange { layer: 1, led: Led { mode: 9, color: 1 } }]).is_err());

        let layout = layout_for_device(DeviceInfo { keys: 0, knobs: 2, variant: 0 }, 0x8840);
        let mut layers = vec![BTreeMap::new(); 3];
        layers[1].insert(0x15, parse_action("volumeup", 0).unwrap());
        layers[0].insert(0x10, parse_action("a,b", 300).unwrap());
        let profile = Profile { layout: layout.clone(), layers, leds: vec![Some(Led { mode: 2, color: 7 }), None, None] };
        let json = serde_json::to_string(&ProfileFile::from_profile(&profile)).unwrap();
        let back: ProfileFile = serde_json::from_str(&json).unwrap();
        let back = back.into_profile().unwrap();
        assert_eq!(back.layout.groups, layout.groups);
        assert_eq!(back.layers, profile.layers);
        assert_eq!(back.leds, profile.leds);
    }

    /// Profiles written by the Python version must load.
    #[test]
    fn python_profile_compat() {
        let json = r#"{"format": "minikeyboard-profile", "version": 1, "keys": 6, "knobs": 1,
            "layers": [{"keys": {"1": {"action": "ctrl-c,ctrl-v", "delay": 300},
                                 "17": {"action": "mute", "delay": 0}},
                        "led": {"mode": 2, "color": 7}}, {"keys": {}, "led": null}]}"#;
        let p = serde_json::from_str::<ProfileFile>(json).unwrap().into_profile().unwrap();
        assert_eq!(p.layout, Layout::new(6, 1));
        assert_eq!(p.layers[0][&1], parse_action("ctrl-c,ctrl-v", 300).unwrap());
        assert_eq!(p.layers[0][&17].kind, Kind::Media);
        assert_eq!(p.leds[0], Some(Led { mode: 2, color: 7 }));
    }

    #[test]
    fn knob_ids_never_overflow() {
        assert_eq!(knob_ids(84, 255, 85), [None, None, None]);
        assert_eq!(knob_ids(0, 253, 1), [Some(254), Some(255), None]);
        assert_eq!(knob_ids(2, 12, 3), [Some(22), Some(23), Some(24)]);
        assert_eq!(knob_ids(0, 24, 1), [Some(25), Some(26), Some(27)]);
    }

    #[test]
    fn crafted_profiles_are_rejected() {
        let load = |json: &str| serde_json::from_str::<ProfileFile>(json).unwrap().into_profile();
        let head = r#""format": "minikeyboard-profile", "version": 1, "layers": []"#;
        assert!(load(&format!("{{{head}, \"keys\": 255, \"knobs\": 255}}")).is_err());
        assert!(load(&format!("{{{head}, \"keys\": 25, \"knobs\": 0}}")).is_err());
        assert!(load(&format!("{{{head}, \"keys\": 4, \"knobs\": 9}}")).is_err());
        assert!(load(&format!("{{{head}, \"keys\": 24, \"knobs\": 1}}")).is_err()); // knob ids past 24
        let ids = |k: &str| load(&format!("{{{head}, \"keys\": 6, \"knobs\": 1, \"knob_ids\": {k}}}"));
        assert!(ids("[[16, 17, 18]]").is_ok());
        assert!(ids("[[19, null, 21]]").is_ok());
        assert!(ids("[[16, 17, 18], [19, 20, 21]]").is_err()); // more groups than knobs
        assert!(ids("[[0, 17, 18]]").is_err());
        assert!(ids("[[16, 17, 25]]").is_err());
        assert!(ids("[[16, 16, 18]]").is_err());
        assert!(ids("[[6, 17, 18]]").is_err()); // clashes with key 6
        assert!(ids("[[null, null, null]]").is_err());
        assert_eq!(ids("[[16, 17, 255]]").unwrap_err(), Error::new("profile_knob_ids"));
        assert_eq!(load(&format!("{{{head}, \"keys\": 25, \"knobs\": 0}}")).unwrap_err().code, "profile_too_big");
        assert_eq!(ids("[[19, null, 21]]").unwrap().layout.kind, LayoutKind::Standard);
        let wheel = load(&format!("{{{head}, \"keys\": 0, \"knobs\": 2, \"knob_ids\": [[16, 17, 18], [19, null, 21]]}}"));
        assert_eq!(wheel.unwrap().layout.kind, LayoutKind::KnobWheel);
    }

    /// The knob + wheel pads report one knob but use two groups: their profiles must reload.
    #[test]
    fn knob_wheel_profile_roundtrip() {
        for layout in [find_layout(0, 1).unwrap(), layout_for_device(DeviceInfo { keys: 0, knobs: 2, variant: 0 }, 0x8840)] {
            let mut layers = vec![BTreeMap::new(); LAYERS as usize];
            layers[0].insert(0x15, parse_action("volumeup", 0).unwrap());
            let profile = Profile { layout: layout.clone(), layers, leds: vec![None; LAYERS as usize] };
            let json = serde_json::to_string(&ProfileFile::from_profile(&profile)).unwrap();
            let back = serde_json::from_str::<ProfileFile>(&json).unwrap().into_profile().unwrap();
            assert_eq!(back.layout, layout);
            assert_eq!(back.layers, profile.layers);
        }
    }

    /// Errors carry a stable code and their arguments, never text: the UI translates them.
    #[test]
    fn error_codes() {
        let code = |t: &str| parse_action(t, 0).unwrap_err();
        assert_eq!(code("foo"), Error::new("unknown_key").arg("foo"));
        assert_eq!(code("ctrl-a-b"), Error::new("one_key_per_chord").arg("ctrl-a-b"));
        assert_eq!(code("a,,b").code, "empty_chord");
        assert_eq!(code("ctrl-volumeup").code, "media_with_modifiers");
        assert_eq!(code("volumeup,a").code, "media_combined");
        assert_eq!(code("click,click").code, "mouse_in_sequence");
        assert_eq!(code("wheelup-wheeldown").code, "wheel_twice");
        assert_eq!(code("click-a"), Error::new("mouse_token_invalid").arg("a"));
        assert_eq!(code("media<x>").code, "media_code_invalid");
        assert_eq!(code("media<70000>"), Error::new("media_code_range").arg("media<70000>"));
        assert_eq!(code("<300>"), Error::new("key_code_range").arg("<300>"));
        assert_eq!(code(&vec!["a"; 18].join(",")), Error::new("too_many_chords").arg(MAX_CHORDS));
        assert_eq!(parse_action("a", MAX_DELAY_MS + 1).unwrap_err(), Error::new("delay_too_long").arg(MAX_DELAY_MS));

        let bad = Action { kind: Kind::Mouse, wheel: 7, ..Default::default() };
        let err = write_plan(&[KeyChange { layer: 1, key_id: 5, action: bad }], &[]).unwrap_err();
        assert_eq!(err, Error::new("wheel_invalid").in_key(5));
        let json = |e: &Error| serde_json::to_value(e).unwrap();
        assert_eq!(json(&err), serde_json::json!({"code": "wheel_invalid", "args": [], "key": 5}));
        assert_eq!(json(&Error::new("unknown_key").arg("foo")), serde_json::json!({"code": "unknown_key", "args": ["foo"]}));
        assert_eq!(write_plan(&[KeyChange { layer: 1, key_id: 25, action: Action::none() }], &[]).unwrap_err(),
                   Error::new("key_id_invalid").arg(25));
        assert_eq!(write_plan(&[], &[LedChange { layer: 1, led: Led { mode: 1, color: 0 } }]).unwrap_err().code, "led_invalid");

        let load = |json: &str| serde_json::from_str::<ProfileFile>(json).unwrap().into_profile().unwrap_err();
        assert_eq!(load(r#"{"format": "x", "version": 1, "keys": 1, "knobs": 0, "layers": []}"#).code, "not_a_profile");
        let head = r#""format": "minikeyboard-profile", "version": 1, "keys": 6, "knobs": 1"#;
        assert_eq!(load(&format!("{{{head}, \"knob_ids\": [], \"layers\": []}}")).code, "profile_knob_count");
        assert_eq!(load(&format!("{{{head}, \"layers\": [{{\"keys\": {{\"x\": {{\"action\": \"a\"}}}}}}]}}")),
                   Error::new("key_id_invalid").arg("x"));
        assert_eq!(load(&format!("{{{head}, \"layers\": [{{\"keys\": {{\"2\": {{\"action\": \"foo\"}}}}}}]}}")),
                   Error::new("unknown_key").arg("foo").in_key(2));
    }
}
