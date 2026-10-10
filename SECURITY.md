# Security policy

manykey reads keyboard input, so security reports are taken seriously. This is a small volunteer project: reports are handled on a best-effort basis, with no guaranteed response time.

## Reporting a vulnerability

Please report suspected vulnerabilities **privately**, not in a public issue:

1. Go to the repository's **Security** tab.
2. Choose **Report a vulnerability** (or use <https://github.com/libmanykey/manykey/security/advisories/new>).

Include what you found, how to reproduce it, which version and platform, and what you think the impact is. We'll coordinate a fix and a disclosure with you before anything is made public.

## What counts

Examples of things worth reporting:

- Memory-safety problems in the library or the C API (out-of-bounds access, use after free, invalid pointer handling).
- manykey reporting input it shouldn't: for example keystrokes outside the scope the caller asked for, or from devices it wasn't asked to watch.
- Behaviour that contradicts what the README says manykey does and does not do (for example, anything that writes files or makes network connections from the library).
- Problems in the build, release or CI setup that could let someone tamper with released artifacts.

## What is by design

manykey's purpose is to observe keystrokes from every keyboard, including while your window isn't focused, and on Linux that needs read access to `/dev/input`. That capability is documented in the README under *What manykey does to your program* and *Linux permissions*; reports that amount to "it can read keystrokes" are describing the design, not a vulnerability. Misuse of the library by software that embeds it is the embedding software's responsibility.

## Supported versions

While the project is pre-1.0, only the latest release receives fixes.
