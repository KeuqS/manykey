use super::{blocks, Def};
use crate::layout::{Builder, KeyRect};

pub const DEF: Def = Def {
    id: "all",
    label: "All keys",
    description: "Everything manykey can report, including F13-F24 and media keys.",
    keys,
};

fn keys() -> Vec<KeyRect> {
    let mut b = Builder::new();
    blocks::full_size(&mut b, true);
    blocks::f13_to_f24(&mut b);
    blocks::media(&mut b);
    b.finish()
}
