#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod device;
mod keycodes;
mod protocol;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;

use device::{DevError, FakeDevice, Hidraw, Keyboard, Node};
use protocol::{Action, ActionView, DeviceInfo, Error, KeyChange, Layout, Led, LedChange, Profile, ProfileFile};

/// An open device and what it said when it was identified.
struct Conn {
    kb: Keyboard,
    info: Option<DeviceInfo>,
    /// time of the last identify request
    identified_at: Instant,
}

/// How often a device that did not tell its model is asked again.
const IDENTIFY_RETRY: Duration = Duration::from_secs(3);

impl Conn {
    /// Identify again if the device stayed silent, at most every IDENTIFY_RETRY; true if asked.
    fn retry_identify(&mut self, now: Instant) -> Result<bool, DevError> {
        if self.info.is_some() || now.duration_since(self.identified_at) < IDENTIFY_RETRY {
            return Ok(false);
        }
        self.identified_at = now;
        self.info = self.kb.identify()?;
        Ok(true)
    }
}

struct Shared {
    conn: Mutex<Option<Conn>>,
    demo: Option<(u8, u8)>,
    /// consecutive EACCES on open: the uaccess ACL lands shortly after the node appears
    denied: AtomicU8,
}

impl Shared {
    fn new(demo: Option<(u8, u8)>) -> Self {
        Self { conn: Mutex::new(None), demo, denied: AtomicU8::new(0) }
    }

    /// The connection, or None while a read/write holds it.
    fn try_conn(&self) -> Option<MutexGuard<'_, Option<Conn>>> {
        match self.conn.try_lock() {
            Ok(g) => Some(g),
            Err(TryLockError::WouldBlock) => None,
            Err(TryLockError::Poisoned(p)) => Some(p.into_inner()),
        }
    }
}

type AppState<'a> = State<'a, Arc<Shared>>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, Error> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| Error::new("internal").arg(e))
}

// ---------------------------------------------------------------------------
// Catalog and actions
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    keys: Vec<keycodes::Key>,
    media: Vec<keycodes::Media>,
    modifiers: Vec<(u8, &'static str)>,
    mouse_buttons: Vec<(u8, &'static str)>,
    layouts: Vec<Layout>,
    max_chords: usize,
    max_delay: u16,
    version: &'static str,
}

#[tauri::command]
fn catalog() -> Catalog {
    Catalog {
        keys: keycodes::keys(),
        media: keycodes::media(),
        modifiers: keycodes::MODIFIERS.to_vec(),
        mouse_buttons: keycodes::MOUSE_BUTTONS.to_vec(),
        layouts: protocol::known_layouts(),
        max_chords: protocol::MAX_CHORDS,
        max_delay: protocol::MAX_DELAY_MS,
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri::command]
fn describe(action: Action) -> ActionView {
    action.normalized().view()
}

#[tauri::command]
fn parse_action(text: String, delay: u16) -> Result<ActionView, Error> {
    protocol::parse_action(&text, delay).map(|a| a.normalized().view())
}

// ---------------------------------------------------------------------------
// Device
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    /// "connected" | "disconnected" | "permission" | "busy" | "error"
    state: &'static str,
    node: Option<Node>,
    info: Option<DeviceInfo>,
    layout: Option<Layout>,
    /// what went wrong when state is "error"
    error: Option<Error>,
    demo: bool,
}

impl Status {
    fn new(state: &'static str) -> Self {
        Self { state, node: None, info: None, layout: None, error: None, demo: false }
    }

    /// A failed open/identify: gone devices are just "disconnected".
    fn failed(e: DevError) -> Self {
        match e {
            DevError::Disconnected => Self::new("disconnected"),
            e => Self { error: Some(e.into()), ..Self::new("error") },
        }
    }
}

/// Connects to a macropad when one appears and notices when it goes away.
fn poll_blocking(shared: &Shared) -> Status {
    let Some(mut guard) = shared.try_conn() else {
        return Status::new("busy");
    };
    if let Some(conn) = guard.as_ref() {
        // same hidrawN after a fast replug is a new device: compare the HID sysfs dir too
        let node = conn.kb.dev.node();
        let present = shared.demo.is_some()
            || device::find_keyboards().iter().any(|n| n.path == node.path && n.hid_dir == node.hid_dir);
        if !present || !conn.kb.dev.alive() {
            *guard = None;
        }
    }
    if let Some(conn) = guard.as_mut() {
        if let Err(e) = conn.retry_identify(Instant::now()) {
            *guard = None;
            return Status::failed(e);
        }
    }
    if guard.is_none() {
        let transport: Box<dyn device::Transport> = if let Some((keys, knobs)) = shared.demo {
            Box::new(FakeDevice::new(keys, knobs))
        } else {
            let Some(node) = device::find_keyboards().into_iter().next() else {
                shared.denied.store(0, Ordering::Relaxed);
                return Status::new("disconnected");
            };
            match Hidraw::open(node.clone()) {
                Ok(h) => {
                    shared.denied.store(0, Ordering::Relaxed);
                    Box::new(h)
                }
                Err(DevError::PermissionDenied(_)) => {
                    let n = shared.denied.load(Ordering::Relaxed).saturating_add(1);
                    shared.denied.store(n, Ordering::Relaxed);
                    if n < 2 {
                        return Status::new("disconnected");
                    }
                    return Status { node: Some(node), ..Status::new("permission") };
                }
                Err(e) => return Status::failed(e),
            }
        };
        let mut kb = Keyboard::new(transport);
        if shared.demo.is_some() {
            kb.store_pause = std::time::Duration::from_millis(150);
        }
        // identify once per connection, like Widget::timeoutSlot
        let info = match kb.identify() {
            Ok(info) => info,
            Err(e) => return Status::failed(e),
        };
        *guard = Some(Conn { kb, info, identified_at: Instant::now() });
    }
    let conn = guard.as_ref().expect("keyboard just opened");
    let (node, info) = (conn.kb.dev.node().clone(), conn.info);
    Status {
        state: "connected",
        layout: info.map(|i| protocol::layout_for_device(i, node.pid)),
        info,
        node: Some(node),
        error: None,
        demo: shared.demo.is_some(),
    }
}

#[tauri::command]
async fn poll(state: AppState<'_>) -> Result<Status, Error> {
    let shared = state.inner().clone();
    blocking(move || poll_blocking(&shared)).await
}

/// Forget the open device; the next poll opens and identifies it again.
#[tauri::command]
fn reconnect(state: AppState<'_>) -> Result<(), Error> {
    let mut guard = state.try_conn().ok_or(Error::new("busy"))?;
    state.denied.store(0, Ordering::Relaxed);
    if state.demo.is_none() {
        *guard = None; // the simulated device keeps what was stored
    }
    Ok(())
}

fn with_keyboard<T>(shared: &Shared, f: impl FnOnce(&mut Keyboard) -> Result<T, DevError>) -> Result<T, Error> {
    let mut guard = shared.conn.lock().unwrap_or_else(|p| p.into_inner());
    let conn = guard.as_mut().ok_or(Error::new("no_keyboard"))?;
    let result = f(&mut conn.kb);
    if result.is_err() {
        *guard = None; // the next poll reopens, like the Python version
    }
    result.map_err(Error::from)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadKey {
    layer: u8,
    key_id: u8,
    action: ActionView,
}

#[tauri::command]
async fn read_config(state: AppState<'_>, app: AppHandle, layout: Layout) -> Result<Vec<ReadKey>, Error> {
    let shared = state.inner().clone();
    blocking(move || {
        with_keyboard(&shared, |kb| {
            let mut out = Vec::new();
            for layer in 1..=protocol::LAYERS {
                for (key_id, action) in kb.read_layer(&layout, layer)? {
                    out.push(ReadKey { layer, key_id, action: action.view() });
                }
                let _ = app.emit("progress", (layer as usize, protocol::LAYERS as usize));
            }
            Ok(out)
        })
    })
    .await?
}

#[tauri::command]
async fn write_changes(
    state: AppState<'_>,
    app: AppHandle,
    keys: Vec<KeyChange>,
    leds: Vec<LedChange>,
) -> Result<usize, Error> {
    let plan = protocol::write_plan(&keys, &leds)?;
    let shared = state.inner().clone();
    blocking(move || {
        with_keyboard(&shared, |kb| {
            kb.write_plan(&plan, |done, total| {
                let _ = app.emit("progress", (done, total));
            })?;
            Ok(plan.len())
        })
    })
    .await?
}

// ---------------------------------------------------------------------------
// Profiles
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ProfileIn {
    layout: Layout,
    layers: Vec<BTreeMap<u8, Action>>,
    leds: Vec<Option<Led>>,
}

#[derive(Serialize)]
struct ProfileOut {
    layout: Layout,
    layers: Vec<BTreeMap<u8, ActionView>>,
    leds: Vec<Option<Led>>,
    path: String,
}

/// Dialog texts come from the UI, already translated.
#[tauri::command]
async fn save_profile(
    app: AppHandle,
    profile: ProfileIn,
    title: String,
    filter: String,
    file_name: String,
) -> Result<Option<String>, Error> {
    blocking(move || {
        let Some(path) = app
            .dialog()
            .file()
            .set_title(title)
            .add_filter(filter, &["json"])
            .set_file_name(file_name)
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = path.into_path().map_err(|e| Error::new("internal").arg(e))?;
        let file = ProfileFile::from_profile(&Profile {
            layout: profile.layout,
            layers: profile.layers,
            leds: profile.leds,
        });
        let json = serde_json::to_string_pretty(&file).map_err(|e| Error::new("internal").arg(e))?;
        std::fs::write(&path, json).map_err(|e| Error::new("file_write").arg(e))?;
        Ok(Some(path.display().to_string()))
    })
    .await?
}

#[tauri::command]
async fn open_profile(app: AppHandle, title: String, filter: String) -> Result<Option<ProfileOut>, Error> {
    blocking(move || {
        let Some(path) = app.dialog().file().set_title(title).add_filter(filter, &["json"]).blocking_pick_file() else {
            return Ok(None);
        };
        let path = path.into_path().map_err(|e| Error::new("internal").arg(e))?;
        let text = std::fs::read_to_string(&path).map_err(|e| Error::new("file_read").arg(e))?;
        let file: ProfileFile = serde_json::from_str(&text).map_err(|e| Error::new("profile_parse").arg(e))?;
        let p = file.into_profile()?;
        Ok(Some(ProfileOut {
            layout: p.layout,
            layers: p
                .layers
                .into_iter()
                .map(|m| m.into_iter().map(|(k, a)| (k, a.view())).collect())
                .collect(),
            leds: p.leds,
            path: path.display().to_string(),
        }))
    })
    .await?
}

fn parse_demo(args: &[String]) -> Option<(u8, u8)> {
    let pos = args.iter().position(|a| a == "--demo")?;
    let spec = args.get(pos + 1).filter(|s| !s.starts_with("--")).map_or("12x2", |s| s.as_str());
    let (k, n) = spec.to_lowercase().split_once('x').and_then(|(k, n)| Some((k.parse().ok()?, n.parse().ok()?)))?;
    Some((k, n))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Usage: minikeyboard [--demo KEYSxKNOBS] [--debug]");
        println!("  --demo 12x2   use a simulated device");
        println!("  --debug       log the USB packets exchanged");
        return;
    }
    if args.iter().any(|a| a == "--demo") && parse_demo(&args).is_none() {
        eprintln!("--demo expects KEYSxKNOBS, e.g. --demo 12x2");
        std::process::exit(2);
    }
    let level = if args.iter().any(|a| a == "--debug") { "debug" } else { "warn" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level)).init();

    let shared = Arc::new(Shared::new(parse_demo(&args)));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            catalog,
            describe,
            parse_action,
            poll,
            reconnect,
            read_config,
            write_changes,
            save_profile,
            open_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while starting the application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_argument() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(parse_demo(&a(&["x", "--demo"])), Some((12, 2)));
        assert_eq!(parse_demo(&a(&["x", "--demo", "6x1"])), Some((6, 1)));
        assert_eq!(parse_demo(&a(&["x", "--demo", "--debug"])), Some((12, 2)));
        assert_eq!(parse_demo(&a(&["x", "--demo", "zz"])), None);
        assert_eq!(parse_demo(&a(&["x"])), None);
    }

    /// `CATALOG_OUT=/path/catalog.json cargo test dump_catalog -- --ignored`
    /// writes what the UI receives, for the browser test harness.
    #[test]
    #[ignore]
    fn dump_catalog() {
        let path = std::env::var("CATALOG_OUT").expect("set CATALOG_OUT");
        std::fs::write(path, serde_json::to_string_pretty(&catalog()).unwrap()).unwrap();
    }

    /// The UI builds every label: the catalog carries names, codes and layout kinds only.
    #[test]
    fn catalog_has_no_labels() {
        let cat = serde_json::to_value(catalog()).unwrap();
        assert_eq!(cat["keys"][0], serde_json::json!({"code": 4, "name": "a", "dom": "KeyA"}));
        assert_eq!(cat["media"][0], serde_json::json!({"code": 0xCD, "name": "play"}));
        assert_eq!(cat["modifiers"][0], serde_json::json!([1, "ctrl"]));
        assert_eq!(cat["mouseButtons"][0], serde_json::json!([1, "click"]));
        let layouts = cat["layouts"].as_array().unwrap();
        assert_eq!(layouts[0]["kind"], "knob_wheel");
        assert!(layouts[1..].iter().all(|l| l["kind"] == "standard"));
        assert!(layouts.iter().all(|l| l.get("label").is_none()));
        let view = serde_json::to_value(describe(protocol::parse_action("ctrl-c", 0).unwrap())).unwrap();
        assert_eq!(view["text"], "ctrl-c");
        assert!(view.get("label").is_none());
    }

    #[test]
    fn errors_are_codes() {
        let shared = Shared::new(None);
        assert_eq!(with_keyboard(&shared, |_| Ok(())).unwrap_err(), Error::new("no_keyboard"));
        let status = Status::failed(DevError::Io("read: x".into()));
        assert_eq!(serde_json::to_value(&status).unwrap()["error"], serde_json::json!({"code": "io", "args": ["read: x"]}));
        assert!(Status::failed(DevError::Disconnected).error.is_none());
    }

    #[test]
    fn poll_demo_connects_and_identifies() {
        let shared = Shared::new(Some((9, 3)));
        let s = poll_blocking(&shared);
        assert_eq!(s.state, "connected");
        assert_eq!(s.layout, protocol::find_layout(9, 3));
        assert_eq!(poll_blocking(&shared).layout, s.layout); // cached, no new identify
        let guard = shared.conn.lock().unwrap();
        assert_eq!(poll_blocking(&shared).state, "busy");
        drop(guard);
    }

    #[test]
    fn silent_device_is_asked_again_later() {
        let mut fake = FakeDevice::new(6, 1);
        fake.identifies = false;
        let t0 = Instant::now();
        let mut conn = Conn { kb: Keyboard::new(Box::new(fake)), info: None, identified_at: t0 };
        assert!(!conn.retry_identify(t0 + Duration::from_secs(1)).unwrap());
        assert!(conn.retry_identify(t0 + IDENTIFY_RETRY).unwrap());
        assert!(conn.info.is_none());
        assert!(!conn.retry_identify(t0 + IDENTIFY_RETRY + Duration::from_secs(1)).unwrap());

        let mut conn = Conn { kb: Keyboard::new(Box::new(FakeDevice::new(6, 1))), info: None, identified_at: t0 };
        assert!(conn.retry_identify(t0 + IDENTIFY_RETRY).unwrap());
        assert_eq!(conn.info.map(|i| (i.keys, i.knobs)), Some((6, 1)));
        assert!(!conn.retry_identify(t0 + 2 * IDENTIFY_RETRY).unwrap());
    }
}
