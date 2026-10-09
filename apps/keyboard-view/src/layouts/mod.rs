//! One file per form factor. To add a layout: create a file with a `DEF`,
//! declare the module below, and add it to `FormFactor::ALL`.

mod all;
mod blocks;
mod compact;
mod keypad;
mod no_numpad;
mod percent75;
mod standard;

use crate::layout::KeyRect;
use std::hash::{Hash, Hasher};

/// What a form-factor file provides.
pub struct Def {
    /// Stable name stored in the preferences file; never change one.
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub keys: fn() -> Vec<KeyRect>,
}

#[derive(Clone, Copy)]
pub struct FormFactor(&'static Def);

impl FormFactor {
    pub const ALL: [FormFactor; 6] = [
        FormFactor(&all::DEF),
        FormFactor(&standard::DEF),
        FormFactor(&no_numpad::DEF),
        FormFactor(&percent75::DEF),
        FormFactor(&compact::DEF),
        FormFactor(&keypad::DEF),
    ];

    /// What a keyboard we know nothing about starts as.
    pub const DEFAULT: FormFactor = FormFactor(&all::DEF);

    pub fn def(self) -> &'static Def {
        self.0
    }
    pub fn id(self) -> &'static str {
        self.0.id
    }
    pub fn label(self) -> &'static str {
        self.0.label
    }
    pub fn description(self) -> &'static str {
        self.0.description
    }

    pub fn from_id(id: &str) -> Option<FormFactor> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }
}

impl PartialEq for FormFactor {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl Eq for FormFactor {}
impl Hash for FormFactor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}
impl std::fmt::Debug for FormFactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormFactor({})", self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Layout;
    use manykey::Key;

    fn ff(id: &str) -> Layout {
        Layout::new(FormFactor::from_id(id).unwrap_or_else(|| panic!("no layout {id}")))
    }

    #[test]
    fn ids_are_unique_and_round_trip() {
        let mut seen = std::collections::HashSet::new();
        for f in FormFactor::ALL {
            assert!(seen.insert(f.id()), "duplicate id {}", f.id());
            assert_eq!(FormFactor::from_id(f.id()), Some(f));
        }
        assert_eq!(FormFactor::from_id("nonsense"), None);
        assert!(FormFactor::ALL.contains(&FormFactor::DEFAULT));
    }

    /// Ids are saved in users' preference files, so they must never change.
    #[test]
    fn saved_ids_are_stable() {
        let ids: Vec<_> = FormFactor::ALL.iter().map(|f| f.id()).collect();
        assert_eq!(ids, ["all", "standard", "no-numpad", "75", "compact", "keypad"]);
    }

    #[test]
    fn layouts_start_at_origin() {
        for f in FormFactor::ALL {
            let l = Layout::new(f);
            assert!(!l.keys.is_empty(), "{f:?}");
            assert!(l.keys.iter().all(|k| k.x >= 0.0 && k.y >= 0.0), "{f:?}");
            assert!(l.keys.iter().any(|k| k.x == 0.0), "{f:?}");
            assert!(l.keys.iter().any(|k| k.y == 0.0), "{f:?}");
            assert!(l.width > 0.0 && l.height > 0.0);
        }
    }

    #[test]
    fn no_key_appears_twice_in_any_layout() {
        for f in FormFactor::ALL {
            let mut seen = std::collections::HashSet::new();
            for k in &Layout::new(f).keys {
                assert!(seen.insert(k.key), "{f:?}: duplicate {:?}", k.key);
            }
        }
    }

    #[test]
    fn no_two_keys_overlap_in_any_layout() {
        for f in FormFactor::ALL {
            let keys = Layout::new(f).keys;
            for (i, a) in keys.iter().enumerate() {
                for b in &keys[i + 1..] {
                    let overlap = a.x < b.x + b.w - 1e-3
                        && b.x < a.x + a.w - 1e-3
                        && a.y < b.y + b.h - 1e-3
                        && b.y < a.y + a.h - 1e-3;
                    assert!(!overlap, "{f:?}: {:?} overlaps {:?}", a.key, b.key);
                }
            }
        }
    }

    #[test]
    fn each_form_factor_has_the_keys_it_claims() {
        let (all, std, tkl, l75, compact, pad) =
            (ff("all"), ff("standard"), ff("no-numpad"), ff("75"), ff("compact"), ff("keypad"));

        assert!(all.has(Key::F13) && all.has(Key::MediaPlayPause) && all.has(Key::Kp5));
        assert!(!std.has(Key::F13) && !std.has(Key::Mute));
        assert!(std.has(Key::Kp5) && std.has(Key::Up) && std.has(Key::F12));
        assert!(!tkl.has(Key::Kp5) && tkl.has(Key::Up));
        assert!(!compact.has(Key::Up) && !compact.has(Key::F1) && compact.has(Key::A));
        assert!(pad.has(Key::Kp5) && !pad.has(Key::A));
        assert!(pad.width < 5.0);
        for k in [Key::PageUp, Key::PageDown, Key::Home, Key::End, Key::Up, Key::Left, Key::F12, Key::Delete] {
            assert!(l75.has(k), "75%: {k:?}");
        }
        assert!(!l75.has(Key::Kp5) && !l75.has(Key::F13) && !l75.has(Key::Mute));
        assert!(l75.width <= 16.0 + f32::EPSILON, "width {}", l75.width);
    }
}
