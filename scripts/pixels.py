"""Reads pixels of a screenshot: touch-inject's raw one (a 16-byte header of width and height, then RGBA rows) or an
8-bit PNG (GNOME Shell's Screenshot D-Bus call).

scripts/windows-native-view-checks.sh and scripts/wayland-native-view-checks.sh ask it what the compositor composed:
the view under GPUI's hole, the div over it, GPUI's background around it, and any gap colour
(docs/native-view-windows.md, docs/native-view-wayland.md).

A colour is rrggbb in hex; `**` for a channel matches any value (the Wayland page's hue drifts in red).

Usage: python3 -I scripts/pixels.py <shot> at <x> <y>           prints "r g b" of that pixel
       python3 -I scripts/pixels.py <shot> count <colour>       prints how many pixels are that colour
       python3 -I scripts/pixels.py <shot> size                 prints "width height"
       python3 -I scripts/pixels.py <shot> bbox <colour>...     prints "left top right bottom" (right and bottom
                                                                exclusive) of the rows and columns with 20 or more
                                                                pixels of any of the colours
       python3 -I scripts/pixels.py <shot> inside <gap> <ring> <colour>...
                                                                prints how many gap-colour pixels lie more than
                                                                <ring> pixels inside the bbox of the gap and the
                                                                colours: a gap that is not just an edge seam
"""

import struct
import sys
import zlib

TOLERANCE = 24  # per channel: DWM's composition, WARP, lavapipe and scaling may round
MIN_RUN = 20  # pixels a row or column needs to count towards a bbox


def read_png(data):
    width = height = None
    idat = b""
    at = 8
    while at < len(data):
        (length,) = struct.unpack_from(">I", data, at)
        kind = data[at + 4 : at + 8]
        body = data[at + 8 : at + 8 + length]
        if kind == b"IHDR":
            width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or colour not in (2, 6) or interlace:
                sys.exit(f"pixels: only 8-bit RGB or RGBA PNGs without interlacing (got {depth}, {colour})")
            channels = 3 if colour == 2 else 4
        elif kind == b"IDAT":
            idat += body
        at += 12 + length
    raw = zlib.decompress(idat)
    stride = width * channels
    rows = []
    previous = bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind, row = raw[start], bytearray(raw[start + 1 : start + 1 + stride])
        # PNG's five filters: none, sub, up, average, Paeth.
        if kind == 1:
            for i in range(channels, stride):
                row[i] = (row[i] + row[i - channels]) & 0xFF
        elif kind == 2:
            row = bytearray((a + b) & 0xFF for a, b in zip(row, previous))
        elif kind == 3:
            for i in range(stride):
                left = row[i - channels] if i >= channels else 0
                row[i] = (row[i] + ((left + previous[i]) >> 1)) & 0xFF
        elif kind == 4:
            for i in range(stride):
                a = row[i - channels] if i >= channels else 0
                b = previous[i]
                c = previous[i - channels] if i >= channels else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                row[i] = (row[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 0xFF
        rows.append(row)
        previous = row
    pixels = bytearray()
    for row in rows:
        if channels == 4:
            pixels += row
        else:
            for i in range(0, stride, 3):
                pixels += row[i : i + 3] + b"\xff"
    return width, height, bytes(pixels)


def read(path):
    data = open(path, "rb").read()
    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        return read_png(data)
    width, height = struct.unpack_from("<II", data)
    return width, height, data[16:]


def colour(spec):
    return [None if spec[i : i + 2] == "**" else int(spec[i : i + 2], 16) for i in (0, 2, 4)]


def matches(pixels, at, want):
    return all(w is None or abs(pixels[at + c] - w) <= TOLERANCE for c, w in enumerate(want))


def bbox(width, pixels, wants):
    """The bounds of the rows and columns with at least MIN_RUN pixels of the colours, so stray antialiased text
    pixels that happen to match don't count."""
    rows, columns = {}, {}
    for at in range(0, len(pixels), 4):
        if any(matches(pixels, at, want) for want in wants):
            x, y = (at // 4) % width, (at // 4) // width
            rows[y] = rows.get(y, 0) + 1
            columns[x] = columns.get(x, 0) + 1
    ys = [y for y, n in rows.items() if n >= MIN_RUN]
    xs = [x for x, n in columns.items() if n >= MIN_RUN]
    if not xs or not ys:
        return None, None, None, None
    return min(xs), min(ys), max(xs) + 1, max(ys) + 1


def main():
    path, command, *args = sys.argv[1:]
    width, height, pixels = read(path)
    if command == "at":
        x, y = int(args[0]), int(args[1])
        if not (0 <= x < width and 0 <= y < height):
            sys.exit(f"pixels: {x} {y} is outside the {width}x{height} screenshot")
        at = (y * width + x) * 4
        print(*pixels[at : at + 3])
    elif command == "count":
        want = colour(args[0])
        print(sum(matches(pixels, at, want) for at in range(0, len(pixels), 4)))
    elif command == "size":
        print(width, height)
    elif command == "bbox":
        box = bbox(width, pixels, [colour(spec) for spec in args])
        if box[0] is None:
            sys.exit("pixels: no pixel of those colours")
        print(*box)
    elif command == "inside":
        gap, ring = colour(args[0]), int(args[1])
        left, top, right, bottom = bbox(width, pixels, [gap] + [colour(spec) for spec in args[2:]])
        if left is None:
            print(0)
            return
        inner = 0
        for y in range(top + ring, bottom - ring):
            for x in range(left + ring, right - ring):
                inner += matches(pixels, (y * width + x) * 4, gap)
        print(inner)
    else:
        sys.exit(f"pixels: unknown command {command}")


main()
