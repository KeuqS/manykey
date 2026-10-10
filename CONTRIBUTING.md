# Contributing to manykey

Thanks for taking an interest. manykey is an early prototype, so the most valuable contributions right now are reports from real hardware.

## The most useful thing you can do: test on real hardware

So far the Windows backend has been run on one machine with a built-in keyboard, a USB keyboard and a ZSA Moonlander at once, and the Linux backend has only been run against *virtual* keyboards in CI, never on physical hardware. Please try any of these and tell us what happened (even "it worked" is useful, because the README marks these as untested):

- Plug in two or more USB keyboards and run `manykey watch`. Does each keystroke carry the right keyboard number?
- Plug a keyboard in and unplug it while `manykey watch` is running. Is it announced and removed correctly?
- Unplug a keyboard while holding a key. Do you see a `[synthesized]` release before the removal?
- If you have two *identical* keyboards, do they get different numbers? Does replugging keep or change a keyboard's number?
- **Linux:** does the evdev backend find your keyboards? What does it say if it lacks permission?
- Remote desktop, virtual machines, KVM switches and virtual keyboards: what shows up?
- Keyboards with media keys, Fn layers, or extra keys: what does `manykey watch` print?

When you report, include:

1. Your OS and version, and for Linux whether you use X11 or Wayland.
2. Which keyboards (make/model, USB or Bluetooth, built-in) and how they were connected.
3. The output of `manykey list` and the relevant part of `manykey watch`.
4. What you expected, and what happened instead.

## Reporting bugs and asking questions

Open an issue at <https://github.com/libmanykey/manykey/issues>. Please search first, and keep one problem per issue.

manykey reads keyboard input, so it has security implications. If you think you've found a vulnerability, please report it privately using GitHub's "Report a vulnerability" option on the repository's Security tab rather than a public issue.

## Development

You need a recent stable Rust toolchain.

```text
cargo build --release          # library, C libraries, and the manykey command line tool
cargo test --workspace         # tests (no keyboard hardware needed)
cargo run -p manykey-view      # the viewer app
```

Useful things to know:

- **Tests don't need hardware.** `Listener::fake` is a scriptable backend; use it for anything that consumes events.
- **The README's Rust snippets are tests.** They're compiled as doctests, so if you change the API, `cargo test` will tell you which examples broke.
- **The C header is generated.** After changing `src/capi.rs`, regenerate it and commit the result:
  `cbindgen --config cbindgen.toml --crate manykey --output include/manykey.h`
- **Backends live in `src/backend/`**, one file each. A new backend implements the `Backend` trait and is added to the selection in `src/backend/mod.rs` and to `BackendKind`.
- **Viewer layouts live in `apps/keyboard-view/src/layouts/`**, one file per keyboard form factor. Adding one is a short file plus a line in `mod.rs`. Never change an existing layout's `id`: it's saved in users' preference files.

## Pull requests

- Keep them small and focused. One change per pull request is easiest to review.
- Add or update tests for behaviour you change.
- **Keep the documentation honest.** The README labels things *Tested*, *Builds*, *Planned* and *Idea*. Don't move something to *Tested* unless you ran it on real hardware, and say what hardware in the pull request.
- Don't add behaviour that polls the OS for key state, blocks or alters input, or sends data anywhere. The README lists what manykey does and doesn't do; changes to that list need a clear reason and a discussion first.
- By contributing, you agree your contribution is licensed under the project's MIT license.

## AI-assisted contributions

AI assistance is welcome here, and the project itself is developed with a lot of it. What matters is accountability:

- **Unreviewed AI-generated pull requests will be closed.** Every contribution must come from a human who has personally read every line they're submitting.
- That person must be able to answer questions about the design, and about how the change was verified: what you ran, and on what.
- If a change depends on behaviour you haven't observed (for example, how an operating system reports a device), say so in the pull request rather than asserting it.

The aim is not to police tools but to keep the codebase something a person understands and stands behind.
