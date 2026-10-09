//! Turning physical keys into text, assuming a US ANSI layout (the layout the
//! viewer draws). manykey reports *positions*; real text input is the OS's job,
//! so this is only an approximation for display.

use manykey::Key;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnterMode {
    /// Insert a visible "[Enter]" marker and keep going on the same line.
    Marker,
    NewLine,
    /// Clear the box, as a chat input would on send.
    Clear,
}

impl EnterMode {
    pub const ALL: [EnterMode; 3] = [EnterMode::Marker, EnterMode::NewLine, EnterMode::Clear];

    pub fn label(self) -> &'static str {
        match self {
            EnterMode::Marker => "show [Enter]",
            EnterMode::NewLine => "new line",
            EnterMode::Clear => "clear box",
        }
    }
}

/// How many characters a tile's text box keeps.
const MAX_CHARS: usize = 400;

/// The character a key types on a US layout, if it types one.
pub fn char_for(key: Key, shift: bool, caps: bool) -> Option<char> {
    let c = key.0;
    let ch = match c {
        0x04..=0x1D => {
            let lower = (b'a' + (c - 0x04) as u8) as char;
            return Some(if shift ^ caps { lower.to_ascii_uppercase() } else { lower });
        }
        0x1E..=0x27 => {
            // 1..9 then 0
            let idx = (c - 0x1E) as usize;
            ("1234567890".as_bytes()[idx] as char, "!@#$%^&*()".as_bytes()[idx] as char)
        }
        0x2C => (' ', ' '),
        0x2D => ('-', '_'),
        0x2E => ('=', '+'),
        0x2F => ('[', '{'),
        0x30 => (']', '}'),
        0x31 => ('\\', '|'),
        0x33 => (';', ':'),
        0x34 => ('\'', '"'),
        0x35 => ('`', '~'),
        0x36 => (',', '<'),
        0x37 => ('.', '>'),
        0x38 => ('/', '?'),
        // Keypad, assuming NumLock is on.
        0x54 => return Some('/'),
        0x55 => return Some('*'),
        0x56 => return Some('-'),
        0x57 => return Some('+'),
        0x59..=0x61 => return Some((b'1' + (c - 0x59) as u8) as char),
        0x62 => return Some('0'),
        0x63 => return Some('.'),
        _ => return None,
    };
    Some(if shift { ch.1 } else { ch.0 })
}

/// Update `text` for one key press (or repeat).
pub fn type_key(text: &mut String, key: Key, shift: bool, caps: bool, command: bool, enter: EnterMode) {
    if command {
        return; // Ctrl/Alt/Win chords aren't text.
    }
    match key {
        Key::Backspace => {
            text.pop();
        }
        Key::Enter | Key::KpEnter => match enter {
            EnterMode::Marker => text.push_str("[Enter]"),
            EnterMode::NewLine => text.push('\n'),
            EnterMode::Clear => text.clear(),
        },
        Key::Tab => text.push_str("[Tab]"),
        _ => {
            if let Some(c) = char_for(key, shift, caps) {
                text.push(c);
            }
        }
    }
    let n = text.chars().count();
    if n > MAX_CHARS {
        *text = text.chars().skip(n - MAX_CHARS).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(keys: &[(Key, bool)], enter: EnterMode) -> String {
        let mut s = String::new();
        for &(k, shift) in keys {
            type_key(&mut s, k, shift, false, false, enter);
        }
        s
    }

    #[test]
    fn letters_digits_and_shift() {
        assert_eq!(char_for(Key::A, false, false), Some('a'));
        assert_eq!(char_for(Key::A, true, false), Some('A'));
        assert_eq!(char_for(Key::Z, false, false), Some('z'));
        assert_eq!(char_for(Key::Digit1, false, false), Some('1'));
        assert_eq!(char_for(Key::Digit1, true, false), Some('!'));
        assert_eq!(char_for(Key::Digit0, true, false), Some(')'));
        assert_eq!(char_for(Key::Semicolon, true, false), Some(':'));
        assert_eq!(char_for(Key::Quote, true, false), Some('"'));
    }

    #[test]
    fn caps_lock_flips_letters_only() {
        assert_eq!(char_for(Key::A, false, true), Some('A'));
        assert_eq!(char_for(Key::A, true, true), Some('a'));
        assert_eq!(char_for(Key::Digit1, false, true), Some('1'));
        assert_eq!(char_for(Key::Digit1, true, true), Some('!'));
    }

    #[test]
    fn non_printing_keys_type_nothing() {
        assert_eq!(char_for(Key::F5, false, false), None);
        assert_eq!(char_for(Key::LeftShift, false, false), None);
        assert_eq!(char_for(Key::Up, false, false), None);
    }

    #[test]
    fn backspace_and_enter_modes() {
        let keys = [(Key::H, true), (Key::I, false), (Key::Enter, false), (Key::Digit2, false)];
        assert_eq!(typed(&keys, EnterMode::NewLine), "Hi\n2");
        assert_eq!(typed(&keys, EnterMode::Marker), "Hi[Enter]2");
        assert_eq!(typed(&keys, EnterMode::Clear), "2");
        assert_eq!(typed(&[(Key::A, false), (Key::B, false), (Key::Backspace, false)], EnterMode::NewLine), "a");
        assert_eq!(typed(&[(Key::Backspace, false)], EnterMode::NewLine), "");
    }

    #[test]
    fn chords_do_not_type() {
        let mut s = String::new();
        type_key(&mut s, Key::C, false, false, true, EnterMode::NewLine);
        assert_eq!(s, "");
    }

    #[test]
    fn text_is_capped() {
        let mut s = String::new();
        for _ in 0..(MAX_CHARS + 50) {
            type_key(&mut s, Key::X, false, false, false, EnterMode::NewLine);
        }
        assert_eq!(s.chars().count(), MAX_CHARS);
    }
}
