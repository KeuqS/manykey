use super::{blocks, Def};
use crate::layout::{Builder, KeyRect};

pub const DEF: Def = Def {
    id: "compact",
    label: "Compact (main block only)",
    description: "Letters, numbers and modifiers only; no function row, arrows or numpad.",
    keys,
};

fn keys() -> Vec<KeyRect> {
    let mut b = Builder::new();
    blocks::number_through_enter(&mut b);
    blocks::shift_row(&mut b);
    blocks::bottom_row(&mut b);
    b.finish()
}
