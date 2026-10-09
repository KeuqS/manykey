use super::{blocks, Def};
use crate::layout::{Builder, KeyRect};

pub const DEF: Def = Def {
    id: "standard",
    label: "Standard (with numpad)",
    description: "Full-size 104-key: function row, navigation cluster, numpad.",
    keys,
};

fn keys() -> Vec<KeyRect> {
    let mut b = Builder::new();
    blocks::full_size(&mut b, true);
    b.finish()
}
