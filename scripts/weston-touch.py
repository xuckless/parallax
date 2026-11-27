#!/usr/bin/env python3
"""Touches a Weston compositor through its test plugin's `weston_test.send_touch`, as Weston's own test suite does
(tests/weston-test-client-helper.c), for record-traces.sh's wayland and x11 harnesses (docs/traces-desktop.md), and
moves, clicks and types with `move_pointer`, `send_button` and `send_key` for weston-selection-checks.sh.

Usage: weston-touch.py FRAME...   with WAYLAND_DISPLAY (and XDG_RUNTIME_DIR) naming a Weston that loaded test-plugin.so
A frame is "ms input..." as record-traces.sh's touch_frames takes it, then a wait of `ms`. An input is a finger
"slot:x:y" (compositor logical px; down when new, else a move) or "slot:up", or the seat's mouse and keyboard:
"pointer:x:y" (move), "button:down" / "button:up" (the left button), "key:CODE:down" / "key:CODE:up" (a Linux evdev
key code, e.g. 42 left shift, 30 a). Every request carries the CLOCK_MONOTONIC time it is sent at, as libinput stamps
real input. Speaks the Wayland wire protocol itself: python3 only, no libraries.
"""

import os
import socket
import struct
import sys
import time

# weston_test's request opcodes and wl_touch's event codes, from Weston 14's protocol/weston-test.xml and wayland.xml.
MOVE_POINTER = 1
SEND_BUTTON = 2
SEND_KEY = 5
SEND_TOUCH = 8
TOUCH_TYPE = {"down": 0, "up": 1, "motion": 2}
# wl_pointer.button_state and wl_keyboard.key_state; BTN_LEFT from linux/input-event-codes.h.
PRESS_STATE = {"up": 0, "down": 1}
BTN_LEFT = 0x110
# The mouse and keyboard inputs: an input's first word → its request and how its arguments pack. To add one, add a row.
SEAT_INPUTS = {
    "pointer": (MOVE_POINTER, lambda x, y: struct.pack("<ii", round(float(x)), round(float(y)))),
    "button": (SEND_BUTTON, lambda state: struct.pack("<iI", BTN_LEFT, PRESS_STATE[state])),
    "key": (SEND_KEY, lambda code, state: struct.pack("<II", int(code), PRESS_STATE[state])),
}


class Wire:
    """Just enough of a Wayland client: the registry, sync and a bind."""

    def __init__(self):
        path = os.path.join(os.environ["XDG_RUNTIME_DIR"], os.environ.get("WAYLAND_DISPLAY", "wayland-0"))
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(path)
        self.next_id = 2
        self.buffer = b""

    def new_id(self):
        self.next_id += 1
        return self.next_id - 1

    def send(self, obj, opcode, args):
        self.sock.sendall(struct.pack("<II", obj, (8 + len(args)) << 16 | opcode) + args)

    def roundtrip(self):
        """Every event until the server answers a wl_display.sync, as (object, opcode, body)."""
        callback = self.new_id()
        self.send(1, 0, struct.pack("<I", callback))
        events = []
        while True:
            while len(self.buffer) < 8 or len(self.buffer) < struct.unpack_from("<I", self.buffer, 4)[0] >> 16:
                chunk = self.sock.recv(65536)
                if not chunk:
                    sys.exit("weston-touch: the compositor closed the connection")
                self.buffer += chunk
            obj, header = struct.unpack_from("<II", self.buffer)
            size = header >> 16
            body, self.buffer = self.buffer[8:size], self.buffer[size:]
            if obj == 1 and header & 0xFFFF == 0:
                error_obj, code, length = struct.unpack_from("<III", body)
                sys.exit(f"weston-touch: protocol error {code} on object {error_obj}: {body[12:11 + length].decode()}")
            if obj == callback:
                return events
            events.append((obj, header & 0xFFFF, body))


def string(text):
    data = text.encode() + b"\0"
    return struct.pack("<I", len(data)) + data + b"\0" * (-len(data) % 4)


def fixed(value):
    return struct.pack("<i", round(float(value) * 256))


def bind_weston_test(wire):
    registry = wire.new_id()
    wire.send(1, 1, struct.pack("<I", registry))
    for obj, opcode, body in wire.roundtrip():
        if obj != registry or opcode != 0:
            continue
        name, length = struct.unpack_from("<II", body)
        if body[8:7 + length].decode() == "weston_test":
            test = wire.new_id()
            wire.send(registry, 0, struct.pack("<I", name) + string("weston_test") + struct.pack("<II", 1, test))
            return test
    sys.exit("weston-touch: no weston_test global: start Weston with --modules=…/test-plugin.so")


def main(frames):
    wire = Wire()
    test = bind_weston_test(wire)
    down = set()
    due = time.monotonic()
    for frame in frames:
        ms, *fingers = frame.split()
        now = time.clock_gettime_ns(time.CLOCK_MONOTONIC)
        stamp = struct.pack("<III", now // 10**9 >> 32, now // 10**9 & 0xFFFFFFFF, now % 10**9)
        for finger in fingers:
            slot, *at = finger.split(":")
            if slot in SEAT_INPUTS:
                opcode, pack = SEAT_INPUTS[slot]
                wire.send(test, opcode, stamp + pack(*at))
                continue
            if at == ["up"]:
                down.discard(slot)
                # An up must carry no coordinates (WESTON_TEST_ERROR_TOUCH_UP_WITH_COORDINATE).
                kind, at = "up", [0, 0]
            else:
                kind = "motion" if slot in down else "down"
                down.add(slot)
            args = stamp + struct.pack("<i", int(slot)) + fixed(at[0]) + fixed(at[1]) + struct.pack("<I", TOUCH_TYPE[kind])
            wire.send(test, SEND_TOUCH, args)
        wire.roundtrip()
        # Scheduled from the start, so the round trips never stretch the gesture.
        due += int(ms) / 1000
        time.sleep(max(0, due - time.monotonic()))


if __name__ == "__main__":
    main(sys.argv[1:])
