//! Cross-platform physical key identity.
//!
//! A [`Key`] is a USB HID usage ID on the Keyboard/Keypad page (0x07): it names
//! a *physical key position*, independent of layout (`Key::A` is the key that
//! is "A" on a US layout, wherever it sits on an AZERTY board). `Key::UNKNOWN`
//! (0) means the backend had no mapping; the raw platform code is always
//! available in `KeyEvent::native_code`.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(pub u16);

macro_rules! keys {
    ($($name:ident = $code:literal),* $(,)?) => {
        #[allow(non_upper_case_globals)]
        impl Key {
            $(pub const $name: Key = Key($code);)*
        }

        fn known_name(code: u16) -> Option<&'static str> {
            match code {
                $($code => Some(stringify!($name)),)*
                _ => None,
            }
        }
    };
}

keys! {
    A = 0x04, B = 0x05, C = 0x06, D = 0x07, E = 0x08, F = 0x09, G = 0x0A,
    H = 0x0B, I = 0x0C, J = 0x0D, K = 0x0E, L = 0x0F, M = 0x10, N = 0x11,
    O = 0x12, P = 0x13, Q = 0x14, R = 0x15, S = 0x16, T = 0x17, U = 0x18,
    V = 0x19, W = 0x1A, X = 0x1B, Y = 0x1C, Z = 0x1D,
    Digit1 = 0x1E, Digit2 = 0x1F, Digit3 = 0x20, Digit4 = 0x21, Digit5 = 0x22,
    Digit6 = 0x23, Digit7 = 0x24, Digit8 = 0x25, Digit9 = 0x26, Digit0 = 0x27,
    Enter = 0x28, Escape = 0x29, Backspace = 0x2A, Tab = 0x2B, Space = 0x2C,
    Minus = 0x2D, Equal = 0x2E, LeftBracket = 0x2F, RightBracket = 0x30,
    Backslash = 0x31, Semicolon = 0x33, Quote = 0x34, Grave = 0x35,
    Comma = 0x36, Period = 0x37, Slash = 0x38, CapsLock = 0x39,
    F1 = 0x3A, F2 = 0x3B, F3 = 0x3C, F4 = 0x3D, F5 = 0x3E, F6 = 0x3F,
    F7 = 0x40, F8 = 0x41, F9 = 0x42, F10 = 0x43, F11 = 0x44, F12 = 0x45,
    PrintScreen = 0x46, ScrollLock = 0x47, Pause = 0x48, Insert = 0x49,
    Home = 0x4A, PageUp = 0x4B, Delete = 0x4C, End = 0x4D, PageDown = 0x4E,
    Right = 0x4F, Left = 0x50, Down = 0x51, Up = 0x52,
    NumLock = 0x53, KpDivide = 0x54, KpMultiply = 0x55, KpMinus = 0x56,
    KpPlus = 0x57, KpEnter = 0x58,
    Kp1 = 0x59, Kp2 = 0x5A, Kp3 = 0x5B, Kp4 = 0x5C, Kp5 = 0x5D, Kp6 = 0x5E,
    Kp7 = 0x5F, Kp8 = 0x60, Kp9 = 0x61, Kp0 = 0x62, KpPeriod = 0x63,
    NonUsBackslash = 0x64, Menu = 0x65,
    F13 = 0x68, F14 = 0x69, F15 = 0x6A, F16 = 0x6B, F17 = 0x6C, F18 = 0x6D,
    F19 = 0x6E, F20 = 0x6F, F21 = 0x70, F22 = 0x71, F23 = 0x72, F24 = 0x73,
    Mute = 0x7F, VolumeUp = 0x80, VolumeDown = 0x81,
    LeftCtrl = 0xE0, LeftShift = 0xE1, LeftAlt = 0xE2, LeftGui = 0xE3,
    RightCtrl = 0xE4, RightShift = 0xE5, RightAlt = 0xE6, RightGui = 0xE7,
}

impl Key {
    pub const UNKNOWN: Key = Key(0);

    pub fn is_unknown(self) -> bool {
        self.0 == 0
    }

    /// A short stable name like `"A"` or `"LeftShift"`, or `"Hid0x.."` for keys
    /// without a name in this table.
    pub fn name(self) -> String {
        match known_name(self.0) {
            Some(n) => n.to_string(),
            None if self.0 == 0 => "Unknown".to_string(),
            None => format!("Hid{:#04x}", self.0),
        }
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Key::{}", self.name())
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

/// Linux evdev `KEY_*` code → HID usage. Unmapped codes give `Key::UNKNOWN`.
pub fn from_evdev(code: u32) -> Key {
    let hid: u16 = match code {
        1 => 0x29,
        2..=10 => 0x1E + (code as u16 - 2), // 1..9
        11 => 0x27,                         // 0
        12 => 0x2D,
        13 => 0x2E,
        14 => 0x2A,
        15 => 0x2B,
        16 => 0x14, // Q
        17 => 0x1A, // W
        18 => 0x08, // E
        19 => 0x15, // R
        20 => 0x17, // T
        21 => 0x1C, // Y
        22 => 0x18, // U
        23 => 0x0C, // I
        24 => 0x12, // O
        25 => 0x13, // P
        26 => 0x2F,
        27 => 0x30,
        28 => 0x28,
        29 => 0xE0,
        30 => 0x04, // A
        31 => 0x16, // S
        32 => 0x07, // D
        33 => 0x09, // F
        34 => 0x0A, // G
        35 => 0x0B, // H
        36 => 0x0D, // J
        37 => 0x0E, // K
        38 => 0x0F, // L
        39 => 0x33,
        40 => 0x34,
        41 => 0x35,
        42 => 0xE1,
        43 => 0x31,
        44 => 0x1D, // Z
        45 => 0x1B, // X
        46 => 0x06, // C
        47 => 0x19, // V
        48 => 0x05, // B
        49 => 0x11, // N
        50 => 0x10, // M
        51 => 0x36,
        52 => 0x37,
        53 => 0x38,
        54 => 0xE5,
        55 => 0x55, // KP *
        56 => 0xE2,
        57 => 0x2C,
        58 => 0x39,
        59..=68 => 0x3A + (code as u16 - 59), // F1..F10
        69 => 0x53,
        70 => 0x47,
        71 => 0x5F, // KP7
        72 => 0x60,
        73 => 0x61,
        74 => 0x56, // KP -
        75 => 0x5C, // KP4
        76 => 0x5D,
        77 => 0x5E,
        78 => 0x57, // KP +
        79 => 0x59, // KP1
        80 => 0x5A,
        81 => 0x5B,
        82 => 0x62, // KP0
        83 => 0x63,
        86 => 0x64,
        87 => 0x44, // F11
        88 => 0x45, // F12
        96 => 0x58, // KP Enter
        97 => 0xE4,
        98 => 0x54, // KP /
        99 => 0x46, // SysRq / PrintScreen
        100 => 0xE6,
        102 => 0x4A,
        103 => 0x52,
        104 => 0x4B,
        105 => 0x50,
        106 => 0x4F,
        107 => 0x4D,
        108 => 0x51,
        109 => 0x4E,
        110 => 0x49,
        111 => 0x4C,
        113 => 0x7F,
        114 => 0x81,
        115 => 0x80,
        119 => 0x48,
        125 => 0xE3,
        126 => 0xE7,
        127 => 0x65,
        183..=194 => 0x68 + (code as u16 - 183), // F13..F24
        _ => 0,
    };
    Key(hid)
}

/// Windows Raw Input make code (set-1 scancode, `0xE000`/`0xE100` OR'd in for
/// extended keys) → HID usage. Set-1 codes below 0x59 coincide with evdev's.
pub fn from_windows_scancode(code: u32) -> Key {
    let evdev = match code {
        0x01..=0x58 => code,
        0xE01C => 96,
        0xE01D => 97,
        0xE035 => 98,
        0xE037 => 99,
        0xE038 => 100,
        0xE047 => 102,
        0xE048 => 103,
        0xE049 => 104,
        0xE04B => 105,
        0xE04D => 106,
        0xE04F => 107,
        0xE050 => 108,
        0xE051 => 109,
        0xE052 => 110,
        0xE053 => 111,
        0xE05B => 125,
        0xE05C => 126,
        0xE05D => 127,
        0xE020 => 113,
        0xE02E => 114,
        0xE030 => 115,
        0xE145 => 119, // Pause
        0xE045 => 69,  // NumLock, as some keyboards report it
        0x64..=0x6F => 183 + (code - 0x64), // F13..F24
        _ => return Key::UNKNOWN,
    };
    from_evdev(evdev)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_and_linux_agree_on_the_same_physical_key() {
        // (evdev, windows) pairs for the same physical key.
        let pairs = [
            (30, 0x1E, Key::A),
            (57, 0x39, Key::Space),
            (42, 0x2A, Key::LeftShift),
            (97, 0xE01D, Key::RightCtrl),
            (103, 0xE048, Key::Up),
            (71, 0x47, Key::Kp7), // keypad 7, not Home
            (102, 0xE047, Key::Home),
            (119, 0xE145, Key::Pause),
            (88, 0x58, Key::F12),
        ];
        for (ev, win, key) in pairs {
            assert_eq!(from_evdev(ev), key, "evdev {ev}");
            assert_eq!(from_windows_scancode(win), key, "windows {win:#x}");
        }
    }

    #[test]
    fn every_main_block_scancode_maps() {
        for c in 1..=83u32 {
            if c == 84 || c == 85 {
                continue;
            }
            assert!(!from_evdev(c).is_unknown(), "evdev {c}");
            assert!(!from_windows_scancode(c).is_unknown(), "windows {c:#x}");
        }
    }

    #[test]
    fn names() {
        assert_eq!(Key::A.name(), "A");
        assert_eq!(Key::LeftShift.name(), "LeftShift");
        assert_eq!(Key(0xAB).name(), "Hid0xab");
        assert_eq!(Key::UNKNOWN.name(), "Unknown");
    }
}
