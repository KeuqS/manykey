//! Reusable parts of keyboards. Coordinates are in the full-size 104-key
//! system (key units), so blocks line up when a layout combines them.

use crate::layout::{Builder, Dir, Shape};
use manykey::Key;

/// Esc and F1-F12 with the usual gaps.
pub fn function_row(b: &mut Builder) {
    b.key(Key::Escape, "Esc", 0.0, 0.0, 1.0, 1.0);
    #[rustfmt::skip]
    let fkeys = [
        (Key::F1, "F1", 2.0), (Key::F2, "F2", 3.0), (Key::F3, "F3", 4.0), (Key::F4, "F4", 5.0),
        (Key::F5, "F5", 6.5), (Key::F6, "F6", 7.5), (Key::F7, "F7", 8.5), (Key::F8, "F8", 9.5),
        (Key::F9, "F9", 11.0), (Key::F10, "F10", 12.0), (Key::F11, "F11", 13.0), (Key::F12, "F12", 14.0),
    ];
    for (key, label, x) in fkeys {
        b.key(key, label, x, 0.0, 1.0, 1.0);
    }
}

pub fn print_scroll_pause(b: &mut Builder) {
    b.key(Key::PrintScreen, "PrtSc", 15.25, 0.0, 1.0, 1.0);
    b.key(Key::ScrollLock, "ScrLk", 16.25, 0.0, 1.0, 1.0);
    b.key(Key::Pause, "Pause", 17.25, 0.0, 1.0, 1.0);
}

/// Number row through the Enter row; identical on every layout we draw.
pub fn number_through_enter(b: &mut Builder) {
    #[rustfmt::skip]
    let row1 = [
        (Key::Grave, "`"), (Key::Digit1, "1"), (Key::Digit2, "2"), (Key::Digit3, "3"),
        (Key::Digit4, "4"), (Key::Digit5, "5"), (Key::Digit6, "6"), (Key::Digit7, "7"),
        (Key::Digit8, "8"), (Key::Digit9, "9"), (Key::Digit0, "0"), (Key::Minus, "-"),
        (Key::Equal, "="),
    ];
    for (i, (key, label)) in row1.into_iter().enumerate() {
        b.key(key, label, i as f32, 1.5, 1.0, 1.0);
    }
    b.key(Key::Backspace, "Bksp", 13.0, 1.5, 2.0, 1.0);

    b.key(Key::Tab, "Tab", 0.0, 2.5, 1.5, 1.0);
    #[rustfmt::skip]
    let row2 = [
        (Key::Q, "Q"), (Key::W, "W"), (Key::E, "E"), (Key::R, "R"), (Key::T, "T"),
        (Key::Y, "Y"), (Key::U, "U"), (Key::I, "I"), (Key::O, "O"), (Key::P, "P"),
        (Key::LeftBracket, "["), (Key::RightBracket, "]"),
    ];
    for (i, (key, label)) in row2.into_iter().enumerate() {
        b.key(key, label, 1.5 + i as f32, 2.5, 1.0, 1.0);
    }
    b.key(Key::Backslash, "\\", 13.5, 2.5, 1.5, 1.0);

    b.key(Key::CapsLock, "Caps", 0.0, 3.5, 1.75, 1.0);
    #[rustfmt::skip]
    let row3 = [
        (Key::A, "A"), (Key::S, "S"), (Key::D, "D"), (Key::F, "F"), (Key::G, "G"),
        (Key::H, "H"), (Key::J, "J"), (Key::K, "K"), (Key::L, "L"),
        (Key::Semicolon, ";"), (Key::Quote, "'"),
    ];
    for (i, (key, label)) in row3.into_iter().enumerate() {
        b.key(key, label, 1.75 + i as f32, 3.5, 1.0, 1.0);
    }
    b.key(Key::Enter, "Enter", 12.75, 3.5, 2.25, 1.0);
}

/// The Z row with full-width right Shift.
pub fn shift_row(b: &mut Builder) {
    b.key(Key::LeftShift, "Shift", 0.0, 4.5, 2.25, 1.0);
    letters_zxcv(b);
    b.key(Key::RightShift, "Shift", 12.75, 4.5, 2.25, 1.0);
}

/// Z through slash, shared by every shift row.
pub fn letters_zxcv(b: &mut Builder) {
    #[rustfmt::skip]
    let row4 = [
        (Key::Z, "Z"), (Key::X, "X"), (Key::C, "C"), (Key::V, "V"), (Key::B, "B"),
        (Key::N, "N"), (Key::M, "M"), (Key::Comma, ","), (Key::Period, "."), (Key::Slash, "/"),
    ];
    for (i, (key, label)) in row4.into_iter().enumerate() {
        b.key(key, label, 2.25 + i as f32, 4.5, 1.0, 1.0);
    }
}

/// Ctrl / Win / Alt / Space / Alt / Win / Menu / Ctrl.
pub fn bottom_row(b: &mut Builder) {
    b.key(Key::LeftCtrl, "Ctrl", 0.0, 5.5, 1.25, 1.0);
    b.key(Key::LeftGui, "Win", 1.25, 5.5, 1.25, 1.0);
    b.key(Key::LeftAlt, "Alt", 2.5, 5.5, 1.25, 1.0);
    b.key(Key::Space, "", 3.75, 5.5, 6.25, 1.0);
    b.key(Key::RightAlt, "Alt", 10.0, 5.5, 1.25, 1.0);
    b.key(Key::RightGui, "Win", 11.25, 5.5, 1.25, 1.0);
    b.key(Key::Menu, "Menu", 12.5, 5.5, 1.25, 1.0);
    b.key(Key::RightCtrl, "Ctrl", 13.75, 5.5, 1.25, 1.0);
}

/// Insert/Home/PageUp over Delete/End/PageDown.
pub fn editing_cluster(b: &mut Builder) {
    b.key(Key::Insert, "Ins", 15.25, 1.5, 1.0, 1.0);
    b.key(Key::Home, "Home", 16.25, 1.5, 1.0, 1.0);
    b.key(Key::PageUp, "PgUp", 17.25, 1.5, 1.0, 1.0);
    b.key(Key::Delete, "Del", 15.25, 2.5, 1.0, 1.0);
    b.key(Key::End, "End", 16.25, 2.5, 1.0, 1.0);
    b.key(Key::PageDown, "PgDn", 17.25, 2.5, 1.0, 1.0);
}

/// Inverted-T arrow cluster in its full-size position.
pub fn arrows(b: &mut Builder) {
    b.arrow(Key::Up, Dir::Up, 16.25, 4.5);
    b.arrow(Key::Left, Dir::Left, 15.25, 5.5);
    b.arrow(Key::Down, Dir::Down, 16.25, 5.5);
    b.arrow(Key::Right, Dir::Right, 17.25, 5.5);
}

pub fn numpad(b: &mut Builder) {
    b.key(Key::NumLock, "Num", 18.5, 1.5, 1.0, 1.0);
    b.key(Key::KpDivide, "/", 19.5, 1.5, 1.0, 1.0);
    b.key(Key::KpMultiply, "*", 20.5, 1.5, 1.0, 1.0);
    b.key(Key::KpMinus, "-", 21.5, 1.5, 1.0, 1.0);
    b.key(Key::Kp7, "7", 18.5, 2.5, 1.0, 1.0);
    b.key(Key::Kp8, "8", 19.5, 2.5, 1.0, 1.0);
    b.key(Key::Kp9, "9", 20.5, 2.5, 1.0, 1.0);
    b.key(Key::KpPlus, "+", 21.5, 2.5, 1.0, 2.0);
    b.key(Key::Kp4, "4", 18.5, 3.5, 1.0, 1.0);
    b.key(Key::Kp5, "5", 19.5, 3.5, 1.0, 1.0);
    b.key(Key::Kp6, "6", 20.5, 3.5, 1.0, 1.0);
    b.key(Key::Kp1, "1", 18.5, 4.5, 1.0, 1.0);
    b.key(Key::Kp2, "2", 19.5, 4.5, 1.0, 1.0);
    b.key(Key::Kp3, "3", 20.5, 4.5, 1.0, 1.0);
    b.key(Key::KpEnter, "Ent", 21.5, 4.5, 1.0, 2.0);
    b.key(Key::Kp0, "0", 18.5, 5.5, 2.0, 1.0);
    b.key(Key::KpPeriod, ".", 20.5, 5.5, 1.0, 1.0);
}

/// A full-size 104-key keyboard, with or without the numpad.
pub fn full_size(b: &mut Builder, with_numpad: bool) {
    function_row(b);
    print_scroll_pause(b);
    number_through_enter(b);
    shift_row(b);
    bottom_row(b);
    editing_cluster(b);
    arrows(b);
    if with_numpad {
        numpad(b);
    }
}

/// F13-F24 on their own row, lined up under the right-hand clusters.
pub fn f13_to_f24(b: &mut Builder) {
    #[rustfmt::skip]
    let fx = [
        (Key::F13, "F13"), (Key::F14, "F14"), (Key::F15, "F15"), (Key::F16, "F16"),
        (Key::F17, "F17"), (Key::F18, "F18"), (Key::F19, "F19"), (Key::F20, "F20"),
        (Key::F21, "F21"), (Key::F22, "F22"), (Key::F23, "F23"), (Key::F24, "F24"),
    ];
    for (i, (key, label)) in fx.into_iter().enumerate() {
        b.key(key, label, 10.5 + i as f32, 7.0, 1.0, 1.0);
    }
}

/// Media/brightness keys: round, spread out, grouped by purpose, on their own row.
pub fn media(b: &mut Builder) {
    #[rustfmt::skip]
    let media: [(Key, &'static str, f32); 9] = [
        (Key::MediaPrev, "Prev", 0.0), (Key::MediaPlayPause, "Play", 1.6),
        (Key::MediaNext, "Next", 3.2), (Key::MediaStop, "Stop", 4.8),
        (Key::Mute, "Mute", 7.2), (Key::VolumeDown, "Vol-", 8.8), (Key::VolumeUp, "Vol+", 10.4),
        (Key::BrightnessDown, "Br-", 12.8), (Key::BrightnessUp, "Br+", 14.4),
    ];
    for (key, label, x) in media {
        b.push(key, label, x, 8.5, 1.2, 1.2, Shape::Round);
    }
}
