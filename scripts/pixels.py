"""Reads pixels of a raw screenshot (touch-inject's: a 16-byte header of width and height, then RGBA rows).

scripts/windows-native-view-checks.sh asks it what DirectComposition composed: the view under GPUI's hole, the
div over it, GPUI's background around it, and any gap colour (docs/native-view-windows.md).

Usage: python3 -I scripts/pixels.py <screenshot> at <x> <y>      prints "r g b" of that pixel
       python3 -I scripts/pixels.py <screenshot> count <rrggbb>  prints how many pixels are that colour
"""

import struct
import sys

TOLERANCE = 24  # per channel, for `count`: DWM's composition and WARP may round


def main():
    path, command, *args = sys.argv[1:]
    data = open(path, "rb").read()
    width, height = struct.unpack_from("<II", data)
    pixels = data[16:]
    if command == "at":
        x, y = int(args[0]), int(args[1])
        if not (0 <= x < width and 0 <= y < height):
            sys.exit(f"pixels: {x} {y} is outside the {width}x{height} screenshot")
        at = (y * width + x) * 4
        print(*pixels[at : at + 3])
    elif command == "count":
        want = bytes.fromhex(args[0])
        count = sum(
            all(abs(pixels[at + c] - want[c]) <= TOLERANCE for c in range(3)) for at in range(0, len(pixels), 4)
        )
        print(count)
    else:
        sys.exit(f"pixels: unknown command {command}")


main()
