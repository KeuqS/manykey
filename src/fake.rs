//! A scriptable backend for tests (yours and ours): no hardware required.

use crate::{Backend, Event, EventSink, Key, KeyEvent, KeyState, KeyboardId, KeyboardInfo, Listener};

struct FakeBackend;
impl Backend for FakeBackend {}

/// Handle for injecting fake keyboards and key events into a [`Listener`].
pub struct FakeKeyboards {
    sink: EventSink,
    next_id: u64,
}

impl Listener {
    /// A listener fed by the returned [`FakeKeyboards`] instead of real devices.
    pub fn fake(capacity: usize) -> (Listener, FakeKeyboards) {
        let mut sink = None;
        let listener = Listener::with_backend(capacity, |s| {
            sink = Some(s);
            Ok(Box::new(FakeBackend))
        })
        .expect("fake backend cannot fail");
        (listener, FakeKeyboards { sink: sink.unwrap(), next_id: 0 })
    }
}

impl FakeKeyboards {
    pub fn add(&mut self, name: &str) -> KeyboardId {
        self.next_id += 1;
        let id = KeyboardId(self.next_id);
        self.sink.send(Event::KeyboardAdded(KeyboardInfo {
            id,
            name: name.to_string(),
            path: format!("fake:{}", id.0),
            vendor_id: Some(0xFFFF),
            product_id: Some(id.0 as u16),
            location: None,
        }));
        id
    }

    pub fn remove(&self, id: KeyboardId) {
        self.sink.send(Event::KeyboardRemoved(id));
    }

    pub fn key(&self, keyboard: KeyboardId, key: Key, state: KeyState) {
        self.sink.send(Event::Key(KeyEvent {
            keyboard,
            key,
            native_code: key.0 as u32,
            state,
        }));
    }

    pub fn press(&self, keyboard: KeyboardId, key: Key) {
        self.key(keyboard, key, KeyState::Pressed);
    }

    pub fn release(&self, keyboard: KeyboardId, key: Key) {
        self.key(keyboard, key, KeyState::Released);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn events_are_tagged_with_their_keyboard() {
        let (l, mut fake) = Listener::fake(64);
        let a = fake.add("A");
        let b = fake.add("B");
        fake.press(a, Key::W);
        fake.press(b, Key::W);
        fake.release(a, Key::W);

        let mut keys = Vec::new();
        while let Some(e) = l.try_recv() {
            if let Event::Key(k) = e {
                keys.push((k.keyboard, k.key, k.state));
            }
        }
        assert_eq!(
            keys,
            vec![
                (a, Key::W, KeyState::Pressed),
                (b, Key::W, KeyState::Pressed),
                (a, Key::W, KeyState::Released),
            ]
        );
    }

    #[test]
    fn overflow_is_announced_and_drops_presses_before_releases() {
        let (l, mut fake) = Listener::fake(4);
        let a = fake.add("A"); // 1
        fake.press(a, Key::A); // 2
        fake.release(a, Key::A); // 3
        fake.press(a, Key::B); // 4
        fake.press(a, Key::C); // full: drops the oldest press (A), not a release
        assert_eq!(l.dropped_events(), 1);

        // Marker comes first, then what survived.
        assert_eq!(l.try_recv(), Some(Event::Overflow { dropped: 1 }));
        assert!(matches!(l.try_recv(), Some(Event::KeyboardAdded(_))));
        let rest: Vec<(Key, KeyState)> = std::iter::from_fn(|| l.try_recv())
            .filter_map(|e| if let Event::Key(k) = e { Some((k.key, k.state)) } else { None })
            .collect();
        assert_eq!(
            rest,
            vec![(Key::A, KeyState::Released), (Key::B, KeyState::Pressed), (Key::C, KeyState::Pressed)]
        );
    }

    #[test]
    fn dropping_a_release_still_signals_overflow() {
        let (l, mut fake) = Listener::fake(2);
        let a = fake.add("A");
        fake.release(a, Key::A);
        fake.release(a, Key::B); // only releases left to drop: oldest goes
        assert_eq!(l.try_recv(), Some(Event::Overflow { dropped: 1 }));
    }

    #[test]
    fn recv_timeout_returns_none_when_idle() {
        let (l, _fake) = Listener::fake(8);
        assert!(l.recv_timeout(Duration::from_millis(20)).is_none());
    }

    #[test]
    fn blocking_recv_wakes_on_event_from_another_thread() {
        let (l, mut fake) = Listener::fake(8);
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            fake.add("late");
        });
        assert!(matches!(l.recv(), Event::KeyboardAdded(_)));
        t.join().unwrap();
    }
}
