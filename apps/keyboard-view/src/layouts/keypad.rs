use super::{blocks, Def};
use crate::layout::{Builder, KeyRect};

pub const DEF: Def = Def { id: "keypad", label: "Keypad only", description: "A standalone numpad.", keys };

fn keys() -> Vec<KeyRect> {
    let mut b = Builder::new();
    blocks::numpad(&mut b);
    b.finish()
}
