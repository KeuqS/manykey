use super::{blocks, Def};
use crate::layout::{Builder, KeyRect};

pub const DEF: Def = Def {
    id: "no-numpad",
    label: "Standard, no numpad",
    description: "Full-size without the numpad (tenkeyless).",
    keys,
};

fn keys() -> Vec<KeyRect> {
    let mut b = Builder::new();
    blocks::full_size(&mut b, false);
    b.finish()
}
