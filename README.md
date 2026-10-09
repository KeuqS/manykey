# manykey

Keyboard input that knows *which* keyboard it came from: instead of a keycode you get `(keyboard, key)`.

- Physical keys are USB HID usages (`Key::A`, `Key::LeftShift`, ...), identical on every OS.
- Keyboards have a process-stable `KeyboardId` plus info (name, vendor/product id, location) to recognise them across runs.
- Backends: Windows (Raw Input, no driver needed, works unfocused), Linux (evdev; needs read access to `/dev/input`).
- Rust API (`Listener`), C API (`include/manykey.h`), and a CLI: `manykey list`, `manykey watch`.
- `Listener::fake()` gives a hardware-free backend for tests.

Regenerate the header after changing `src/capi.rs`:
`cbindgen --config cbindgen.toml --crate manykey --output include/manykey.h`

MIT licensed.
