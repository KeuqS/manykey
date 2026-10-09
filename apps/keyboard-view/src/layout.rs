//! Layout types: the shapes keys are drawn in, and a builder the per-form-factor
//! files in `layouts/` use to describe themselves. Geometry is in key units
//! (1.0 = one standard key).

use crate::layouts::FormFactor;
use manykey::Key;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Rect,
    Round,
    /// A rectangle with a drawn triangle (arrow glyphs are missing from egui's fonts).
    Arrow(Dir),
}

pub struct KeyRect {
    pub key: Key,
    pub label: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub shape: Shape,
}

/// The keys one form factor shows, repacked so the top-left key is at (0, 0).
pub struct Layout {
    pub keys: Vec<KeyRect>,
    pub width: f32,
    pub height: f32,
}

impl Layout {
    pub fn new(ff: FormFactor) -> Layout {
        let mut keys = (ff.def().keys)();
        let min_x = keys.iter().map(|k| k.x).fold(f32::MAX, f32::min);
        let min_y = keys.iter().map(|k| k.y).fold(f32::MAX, f32::min);
        let max_x = keys.iter().map(|k| k.x + k.w).fold(0.0, f32::max);
        let max_y = keys.iter().map(|k| k.y + k.h).fold(0.0, f32::max);
        for k in &mut keys {
            k.x -= min_x;
            k.y -= min_y;
        }
        Layout { keys, width: max_x - min_x, height: max_y - min_y }
    }

    pub fn has(&self, key: Key) -> bool {
        self.keys.iter().any(|k| k.key == key)
    }
}

/// Collects keys for a layout definition.
pub struct Builder {
    keys: Vec<KeyRect>,
}

impl Builder {
    pub fn new() -> Self {
        Builder { keys: Vec::new() }
    }

    pub fn key(&mut self, key: Key, label: &'static str, x: f32, y: f32, w: f32, h: f32) {
        self.push(key, label, x, y, w, h, Shape::Rect);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn push(&mut self, key: Key, label: &'static str, x: f32, y: f32, w: f32, h: f32, shape: Shape) {
        self.keys.push(KeyRect { key, label, x, y, w, h, shape });
    }

    pub fn arrow(&mut self, key: Key, dir: Dir, x: f32, y: f32) {
        self.push(key, "", x, y, 1.0, 1.0, Shape::Arrow(dir));
    }

    pub fn finish(self) -> Vec<KeyRect> {
        self.keys
    }
}
