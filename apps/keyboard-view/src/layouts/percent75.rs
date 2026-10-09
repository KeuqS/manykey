//! 75%: contiguous function row, arrow cluster in the corner, and a right-hand
//! column of PageUp/PageDown/Home/End. No numpad.

use super::{blocks, Def};
use crate::layout::{Builder, Dir, KeyRect};
use manykey::Key;

pub const DEF: Def = Def {
    id: "75",
    label: "75%",
    description: "Compact: function row, arrow keys, and a PgUp/PgDn/Home/End column. No numpad.",
    keys,
};

fn keys() -> Vec<KeyRect> {
    let mut b = Builder::new();

    b.key(Key::Escape, "Esc", 0.0, 0.0, 1.0, 1.0);
    let fkeys = [
        (Key::F1, "F1"), (Key::F2, "F2"), (Key::F3, "F3"), (Key::F4, "F4"), (Key::F5, "F5"), (Key::F6, "F6"),
        (Key::F7, "F7"), (Key::F8, "F8"), (Key::F9, "F9"), (Key::F10, "F10"), (Key::F11, "F11"), (Key::F12, "F12"),
    ];
    for (i, (key, label)) in fkeys.into_iter().enumerate() {
        b.key(key, label, 1.0 + i as f32, 0.0, 1.0, 1.0);
    }
    b.key(Key::PrintScreen, "PrtSc", 13.0, 0.0, 1.0, 1.0);
    b.key(Key::Insert, "Ins", 14.0, 0.0, 1.0, 1.0);
    b.key(Key::Delete, "Del", 15.0, 0.0, 1.0, 1.0);

    blocks::number_through_enter(&mut b);

    // Shorter right Shift and bottom row, to make room for the arrows.
    b.key(Key::LeftShift, "Shift", 0.0, 4.5, 2.25, 1.0);
    blocks::letters_zxcv(&mut b);
    b.key(Key::RightShift, "Shift", 12.25, 4.5, 1.75, 1.0);

    b.key(Key::LeftCtrl, "Ctrl", 0.0, 5.5, 1.25, 1.0);
    b.key(Key::LeftGui, "Win", 1.25, 5.5, 1.25, 1.0);
    b.key(Key::LeftAlt, "Alt", 2.5, 5.5, 1.25, 1.0);
    b.key(Key::Space, "", 3.75, 5.5, 5.5, 1.0);
    b.key(Key::RightAlt, "Alt", 9.25, 5.5, 1.25, 1.0);
    b.key(Key::RightCtrl, "Ctrl", 10.5, 5.5, 1.25, 1.0);

    b.arrow(Key::Up, Dir::Up, 14.0, 4.5);
    b.arrow(Key::Left, Dir::Left, 13.0, 5.5);
    b.arrow(Key::Down, Dir::Down, 14.0, 5.5);
    b.arrow(Key::Right, Dir::Right, 15.0, 5.5);
    b.key(Key::PageUp, "PgUp", 15.0, 1.5, 1.0, 1.0);
    b.key(Key::PageDown, "PgDn", 15.0, 2.5, 1.0, 1.0);
    b.key(Key::Home, "Home", 15.0, 3.5, 1.0, 1.0);
    b.key(Key::End, "End", 15.0, 4.5, 1.0, 1.0);

    b.finish()
}
