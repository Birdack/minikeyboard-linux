//! HID usage tables shared by the protocol and the UI catalog.
//! Only stable syntax names and codes live here: the UI builds every label.

use serde::Serialize;

/// (bit, syntax name)
pub const MODIFIERS: [(u8, &str); 8] = [
    (0x01, "ctrl"),
    (0x02, "shift"),
    (0x04, "alt"),
    (0x08, "win"),
    (0x10, "rctrl"),
    (0x20, "rshift"),
    (0x40, "ralt"),
    (0x80, "rwin"),
];

const MODIFIER_ALIASES: [(&str, u8); 13] = [
    ("lctrl", 0x01),
    ("control", 0x01),
    ("lshift", 0x02),
    ("lalt", 0x04),
    ("opt", 0x04),
    ("cmd", 0x08),
    ("super", 0x08),
    ("meta", 0x08),
    ("lwin", 0x08),
    ("altgr", 0x40),
    ("ropt", 0x40),
    ("rcmd", 0x80),
    ("rsuper", 0x80),
];

#[derive(Clone, Serialize)]
pub struct Key {
    pub code: u8,
    pub name: String,
    /// KeyboardEvent.code of the physical key, used to record from the PC keyboard
    pub dom: String,
}

#[derive(Clone, Serialize)]
pub struct Media {
    pub code: u16,
    pub name: &'static str,
}

pub const MEDIA: [(u16, &str); 19] = [
    (0x00CD, "play"),
    (0x00B7, "stop"),
    (0x00B6, "previous"),
    (0x00B5, "next"),
    (0x00E2, "mute"),
    (0x00EA, "volumedown"),
    (0x00E9, "volumeup"),
    (0x0070, "screenbrightnessdown"),
    (0x006F, "screenbrightnessup"),
    (0x0192, "calculator"),
    (0x018A, "mail"),
    (0x0194, "explorer"),
    (0x019E, "screenlock"),
    (0x0182, "favorites"),
    (0x0221, "search"),
    (0x0223, "webpagehome"),
    (0x0224, "webpageback"),
    (0x0225, "webpageforward"),
    (0x0227, "webpagerefresh"),
];

/// (bit, syntax name)
pub const MOUSE_BUTTONS: [(u8, &str); 3] = [(0x01, "click"), (0x02, "rclick"), (0x04, "mclick")];
pub const WHEEL_UP: u8 = 0x01;
pub const WHEEL_DOWN: u8 = 0xFF;

/// Keyboard page (0x07) usages with their syntax name and DOM code.
pub fn keys() -> Vec<Key> {
    let mut out = Vec::new();
    let mut push = |code: u8, name: &str, dom: &str| out.push(Key { code, name: name.into(), dom: dom.into() });
    for (i, c) in ('a'..='z').enumerate() {
        let up = c.to_ascii_uppercase();
        push(0x04 + i as u8, &c.to_string(), &format!("Key{up}"));
    }
    for (i, c) in "1234567890".chars().enumerate() {
        push(0x1E + i as u8, &c.to_string(), &format!("Digit{c}"));
    }
    for (code, name, dom) in [
        (0x28, "enter", "Enter"),
        (0x29, "escape", "Escape"),
        (0x2A, "backspace", "Backspace"),
        (0x2B, "tab", "Tab"),
        (0x2C, "space", "Space"),
        (0x2D, "minus", "Minus"),
        (0x2E, "equal", "Equal"),
        (0x2F, "leftbracket", "BracketLeft"),
        (0x30, "rightbracket", "BracketRight"),
        (0x31, "backslash", "Backslash"),
        (0x32, "nonushash", "IntlHash"),
        (0x33, "semicolon", "Semicolon"),
        (0x34, "quote", "Quote"),
        (0x35, "grave", "Backquote"),
        (0x36, "comma", "Comma"),
        (0x37, "dot", "Period"),
        (0x38, "slash", "Slash"),
        (0x39, "capslock", "CapsLock"),
    ] {
        push(code, name, dom);
    }
    for i in 0..12u8 {
        push(0x3A + i, &format!("f{}", i + 1), &format!("F{}", i + 1));
    }
    for (code, name, dom) in [
        (0x46, "printscreen", "PrintScreen"),
        (0x47, "scrolllock", "ScrollLock"),
        (0x48, "pause", "Pause"),
        (0x49, "insert", "Insert"),
        (0x4A, "home", "Home"),
        (0x4B, "pageup", "PageUp"),
        (0x4C, "delete", "Delete"),
        (0x4D, "end", "End"),
        (0x4E, "pagedown", "PageDown"),
        (0x4F, "right", "ArrowRight"),
        (0x50, "left", "ArrowLeft"),
        (0x51, "down", "ArrowDown"),
        (0x52, "up", "ArrowUp"),
        (0x53, "numlock", "NumLock"),
        (0x54, "numpadslash", "NumpadDivide"),
        (0x55, "numpadasterisk", "NumpadMultiply"),
        (0x56, "numpadminus", "NumpadSubtract"),
        (0x57, "numpadplus", "NumpadAdd"),
        (0x58, "numpadenter", "NumpadEnter"),
    ] {
        push(code, name, dom);
    }
    for i in 0..9u8 {
        push(0x59 + i, &format!("numpad{}", i + 1), &format!("Numpad{}", i + 1));
    }
    for (code, name, dom) in [
        (0x62, "numpad0", "Numpad0"),
        (0x63, "numpaddot", "NumpadDecimal"),
        (0x64, "nonusbackslash", "IntlBackslash"),
        (0x65, "application", "ContextMenu"),
        (0x66, "power", "Power"),
        (0x67, "numpadequal", "NumpadEqual"),
    ] {
        push(code, name, dom);
    }
    for i in 0..12u8 {
        push(0x68 + i, &format!("f{}", i + 13), &format!("F{}", i + 13));
    }
    out
}

pub fn media() -> Vec<Media> {
    MEDIA.iter().map(|&(code, name)| Media { code, name }).collect()
}

pub fn modifier_by_name(name: &str) -> Option<u8> {
    MODIFIERS
        .iter()
        .find(|m| m.1 == name)
        .map(|m| m.0)
        .or_else(|| MODIFIER_ALIASES.iter().find(|a| a.0 == name).map(|a| a.1))
}

pub fn key_by_name(name: &str) -> Option<u8> {
    let alias = match name {
        "esc" => Some(0x29),
        "return" => Some(0x28),
        "del" => Some(0x4C),
        "period" => Some(0x37),
        "pgup" => Some(0x4B),
        "pgdn" => Some(0x4E),
        "menu" => Some(0x65),
        "ins" => Some(0x49),
        _ => None,
    };
    alias.or_else(|| keys().into_iter().find(|k| k.name == name).map(|k| k.code))
}

pub fn key_name(code: u8) -> String {
    keys().into_iter().find(|k| k.code == code).map(|k| k.name).unwrap_or(format!("<{code}>"))
}

pub fn media_by_name(name: &str) -> Option<u16> {
    if name == "prev" {
        return Some(0x00B6);
    }
    MEDIA.iter().find(|m| m.1 == name).map(|m| m.0)
}

pub fn media_name(code: u16) -> String {
    MEDIA.iter().find(|m| m.0 == code).map(|m| m.1.to_string()).unwrap_or(format!("media<{code}>"))
}

pub fn modifier_names(mods: u8) -> Vec<&'static str> {
    MODIFIERS.iter().filter(|m| mods & m.0 != 0).map(|m| m.1).collect()
}
