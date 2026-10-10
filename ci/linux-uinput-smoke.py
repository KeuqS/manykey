#!/usr/bin/env python3
"""Smoke test for the Linux (evdev) backend using virtual keyboards.

Creates keyboards through /dev/uinput (needs root), runs `manykey watch`
against them, and checks key events, hot-plug, and that unplugging a keyboard
while a key is held produces a synthesized release followed by its removal.

Usage: sudo python3 ci/linux-uinput-smoke.py target/debug/manykey
"""
import fcntl
import os
import re
import struct
import subprocess
import sys
import threading
import time

UI_SET_EVBIT = 0x40045564
UI_SET_KEYBIT = 0x40045565
UI_DEV_CREATE = 0x5501
UI_DEV_DESTROY = 0x5502
EV_SYN, EV_KEY, EV_REP = 0, 1, 0x14
KEY_A, KEY_B = 30, 48


class VirtualKeyboard:
    def __init__(self, name, vendor, product):
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
        fcntl.ioctl(self.fd, UI_SET_EVBIT, EV_KEY)
        # manykey treats devices that support key repeat as keyboards.
        fcntl.ioctl(self.fd, UI_SET_EVBIT, EV_REP)
        for key in range(1, 120):
            fcntl.ioctl(self.fd, UI_SET_KEYBIT, key)
        # struct uinput_user_dev: name[80], input_id, ff_effects_max, abs arrays
        dev = (name.encode().ljust(80, b"\0")
               + struct.pack("HHHH", 3, vendor, product, 1)   # BUS_USB
               + struct.pack("I", 0)
               + b"\0" * (4 * 64 * 4))
        os.write(self.fd, dev)
        fcntl.ioctl(self.fd, UI_DEV_CREATE)

    def _emit(self, kind, code, value):
        now = time.time()
        sec = int(now)
        os.write(self.fd, struct.pack("llHHi", sec, int((now - sec) * 1e6), kind, code, value))

    def key(self, code, down):
        self._emit(EV_KEY, code, 1 if down else 0)
        self._emit(EV_SYN, 0, 0)

    def destroy(self):
        fcntl.ioctl(self.fd, UI_DEV_DESTROY)
        os.close(self.fd)


def main(binary):
    kb = VirtualKeyboard("manykey-ci-kbd", 0x1234, 0x5678)
    time.sleep(1.0)  # let udev/the kernel publish the device

    proc = subprocess.Popen([binary, "watch"], stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, text=True)
    lines = []
    reader = threading.Thread(target=lambda: [lines.append(l.rstrip("\n")) for l in proc.stdout],
                              daemon=True)
    reader.start()
    time.sleep(2.0)  # startup enumeration

    kb.key(KEY_A, True)
    kb.key(KEY_A, False)
    kb.key(KEY_B, True)                      # hold B ...
    time.sleep(0.5)
    kb2 = VirtualKeyboard("manykey-ci-kbd-two", 0x1234, 0x9999)   # hot-plug a second one
    time.sleep(1.5)
    kb.destroy()                             # ... and unplug the first while B is held
    time.sleep(1.5)
    kb2.destroy()
    time.sleep(1.0)
    proc.terminate()
    reader.join(timeout=3)

    out = "\n".join(lines)
    print("== manykey watch output\n" + out)
    failures = []

    m = re.search(r"^\+ keyboard (\d+) 1234:5678 manykey-ci-kbd$", out, re.M)
    if not m:
        failures.append("virtual keyboard 1 was not announced with its name and USB ids")
        kid = None
    else:
        kid = m.group(1)
    if not re.search(r"^\+ keyboard \d+ 1234:9999 manykey-ci-kbd-two$", out, re.M):
        failures.append("hot-plugged keyboard 2 was not announced")
    if kid:
        if not re.search(rf"^keyboard {kid}\s+A\s+down", out, re.M):
            failures.append("A press not reported for keyboard 1")
        if not re.search(rf"^keyboard {kid}\s+A\s+up", out, re.M):
            failures.append("A release not reported for keyboard 1")
        synth = re.search(rf"^keyboard {kid}\s+B\s+up .*\[synthesized\]$", out, re.M)
        removed = re.search(rf"^- keyboard {kid}$", out, re.M)
        if not synth:
            failures.append("no synthesized release for B when keyboard 1 was unplugged while held")
        if not removed:
            failures.append("keyboard 1 removal was not reported")
        if synth and removed and synth.start() > removed.start():
            failures.append("synthesized release came after the removal, not before")

    if failures:
        print("\nFAILED:\n  - " + "\n  - ".join(failures))
        print("\n== /proc/bus/input/devices")
        print(open("/proc/bus/input/devices").read())
        return 1
    print("\nOK: events, hot-plug and release-on-unplug all behaved as expected")
    return 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1]))
