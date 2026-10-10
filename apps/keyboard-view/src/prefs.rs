//! Remembers which form factor each keyboard was set to, in a small text file.

use crate::layouts::FormFactor;
use manykey::KeyboardInfo;
use std::collections::HashMap;
use std::path::PathBuf;

/// An identity for a keyboard that survives restarts, as far as the OS lets us
/// tell. Combines name, USB ids and where it's attached (Linux: physical port;
/// Windows: the device path, which includes a per-port instance id). Moving a
/// keyboard to a different port can make it look like a new one.
pub fn stable_key(info: &KeyboardInfo) -> String {
    let ids = match (info.vendor_id, info.product_id) {
        (Some(v), Some(p)) => format!("{v:04x}:{p:04x}"),
        _ => "----:----".to_string(),
    };
    let place = info.location.as_deref().unwrap_or(&info.path);
    format!("{}|{}|{}", info.name, ids, place).replace(['\t', '\n', '\r'], " ")
}

pub struct Prefs {
    path: Option<PathBuf>,
    factors: HashMap<String, FormFactor>,
}

impl Prefs {
    pub fn load() -> Prefs {
        let path = config_path();
        let factors =
            path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse(&t)).unwrap_or_default();
        Prefs { path, factors }
    }

    pub fn factor_for(&self, info: &KeyboardInfo) -> Option<FormFactor> {
        self.factors.get(&stable_key(info)).copied()
    }

    pub fn set_factor(&mut self, info: &KeyboardInfo, ff: FormFactor) {
        self.factors.insert(stable_key(info), ff);
        self.save();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(path, serialize(&self.factors)) {
            eprintln!("manykey-view: could not save {}: {e}", path.display());
        }
    }
}

fn config_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }?;
    Some(base.join("manykey").join("viewer.conf"))
}

fn parse(text: &str) -> HashMap<String, FormFactor> {
    text.lines()
        .filter_map(|line| {
            let (key, ff) = line.rsplit_once('\t')?;
            Some((key.to_string(), FormFactor::from_id(ff)?))
        })
        .collect()
}

fn serialize(map: &HashMap<String, FormFactor>) -> String {
    let mut lines: Vec<String> = map.iter().map(|(k, ff)| format!("{k}\t{}", ff.id())).collect();
    lines.sort();
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use manykey::KeyboardId;

    fn info(name: &str, vid: u16, pid: u16, loc: Option<&str>) -> KeyboardInfo {
        KeyboardInfo {
            id: KeyboardId(7),
            name: name.into(),
            path: format!("\\\\?\\HID#{name}"),
            vendor_id: Some(vid),
            product_id: Some(pid),
            location: loc.map(String::from),
        }
    }

    #[test]
    fn round_trips_through_the_file_format() {
        let mut m = HashMap::new();
        m.insert(stable_key(&info("Board A", 0x046d, 0xc31c, Some("usb-1"))), FormFactor::from_id("keypad").unwrap());
        m.insert(stable_key(&info("Board B", 1, 2, None)), FormFactor::from_id("no-numpad").unwrap());
        assert_eq!(parse(&serialize(&m)), m);
    }

    #[test]
    fn ignores_garbage_and_unknown_factors() {
        let m = parse("no tab here\nkey\tnot-a-factor\nok|1|2\tcompact\n");
        assert_eq!(m.len(), 1);
        assert_eq!(m["ok|1|2"], FormFactor::from_id("compact").unwrap());
    }

    #[test]
    fn identical_keyboards_on_different_ports_differ() {
        let a = stable_key(&info("Same", 1, 2, Some("usb-port-1")));
        let b = stable_key(&info("Same", 1, 2, Some("usb-port-2")));
        assert_ne!(a, b);
    }

    #[test]
    fn same_keyboard_gets_same_key_across_runs() {
        let mut one = info("Same", 1, 2, Some("p"));
        let mut two = info("Same", 1, 2, Some("p"));
        one.id = KeyboardId(1);
        two.id = KeyboardId(9); // process-local id must not matter
        assert_eq!(stable_key(&one), stable_key(&two));
    }
}
