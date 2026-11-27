#!/usr/bin/env python3
"""Touches a Weston compositor through its test plugin's `weston_test.send_touch`, as Weston's own test suite does
(tests/weston-test-client-helper.c), for record-traces.sh's wayland and x11 harnesses (docs/traces-desktop.md).

Usage: weston-touch.py FRAME...   with WAYLAND_DISPLAY (and XDG_RUNTIME_DIR) naming a Weston that loaded test-plugin.so
A frame is "ms finger..." as record-traces.sh's touch_frames takes it: each finger "slot:x:y" (compositor logical px;
down when new, else a move) or "slot:up", then a wait of `ms`. Every request carries the CLOCK_MONOTONIC time it is
sent at, as libinput stamps real touches. Speaks the Wayland wire protocol itself: python3 only, no libraries.
"""

import os
import socket
import struct
import sys
import time

# weston_test's request opcodes and wl_touch's event codes, from Weston 14's protocol/weston-test.xml and wayland.xml.
SEND_TOUCH = 8
TOUCH_TYPE = {"down": 0, "up": 1, "motion": 2}


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
