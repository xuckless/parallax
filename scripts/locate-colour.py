"""Prints "x y", in image pixels, of the centre of the topmost horizontal band of a screenshot painted in one colour.

GPUI draws everything itself, so there is no view tree to query: the smoke and trace scripts find what to tap by its
colour (docs/android-smoke.md, docs/traces.md). Reads the image on stdin: a raw Android `screencap` (RGBA_8888 after
a 12- or 16-byte header) or a PNG (8-bit RGB or RGBA, not interlaced: what Chromium and `simctl io screenshot` write).

Usage: python3 -I scripts/locate-colour.py <rrggbb> [ink] < screenshot
With `ink`, prints "x0 x1 y" instead: the horizontal extent of what is drawn on that band (weston-selection-checks.sh
finds a text field's glyphs so). Exits 1 when no band of that colour is on screen.
"""

import struct
import sys
import zlib

STEP = 8  # sample every 8th row and column
TOLERANCE = 8  # per channel: an emulator's GL draws to RGB565, which rounds red and blue to steps of 8
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    return a if pa <= pb and pa <= pc else b if pb <= pc else c


def png_rows(data):
    """(width, height, channels, rows): each row the unfiltered bytes of one scanline."""
    chunks, at = {}, len(PNG_SIGNATURE)
    while at < len(data):
        (length,) = struct.unpack_from(">I", data, at)
        kind = data[at + 4 : at + 8]
        chunks[kind] = chunks.get(kind, b"") + data[at + 8 : at + 8 + length]
        at += 12 + length
    width, height, depth, colour_type, _, _, interlace = struct.unpack(">IIBBBBB", chunks[b"IHDR"])
    channels = {2: 3, 6: 4}.get(colour_type)
    if depth != 8 or channels is None or interlace:
        sys.exit("locate-colour: only 8-bit RGB or RGBA PNGs, not interlaced")
    raw = zlib.decompress(chunks[b"IDAT"])
    stride, rows, previous = width * channels, [], bytes(width * channels)
    for y in range(height):
        kind, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1 : (y + 1) * (stride + 1)])
        for i in range(stride):
            left = line[i - channels] if i >= channels else 0
            up = previous[i]
            corner = previous[i - channels] if i >= channels else 0
            line[i] = (line[i] + (0, left, up, (left + up) // 2, paeth(left, up, corner))[kind]) & 0xFF
        rows.append(bytes(line))
        previous = line
    return width, height, channels, rows


def raw_rows(data):
    width, height = struct.unpack_from("<II", data)
    pixels = data[len(data) - width * height * 4 :]
    return width, height, 4, [pixels[y * width * 4 : (y + 1) * width * 4] for y in range(height)]


data = sys.stdin.buffer.read()
width, height, channels, rows = (png_rows if data.startswith(PNG_SIGNATURE) else raw_rows)(data)
want = bytes.fromhex(sys.argv[1])


def matches(row, x):
    return all(abs(p - w) <= TOLERANCE for p, w in zip(row[x * channels : x * channels + 3], want))


band = []  # (y, sampled matching xs) of matching rows in the first band
for y in range(0, height, STEP):
    xs = [x for x in range(0, width, STEP) if matches(rows[y], x)]
    # A band row: at least a quarter of the sampled pixels match, so status bar icons and glyphs never count.
    if len(xs) * STEP * 4 >= width:
        band.append((y, xs))
    elif band:
        break
if not band:
    sys.exit("no band of colour " + sys.argv[1] + " on screen")
y, xs = band[len(band) // 2]
if sys.argv[2:] != ["ink"]:
    print(sum(xs) // len(xs), y)
    sys.exit()
# "ink": what is drawn on the band (a text field's glyphs) as "x0 x1 y", its leftmost and rightmost pixel not of the
# band's colour, every pixel of its sampled rows looked at between that row's first and last pixel of the colour (so
# the band's anti-aliased edges and rounded corners are not ink).
ink = []
for row in rows[band[0][0] : band[-1][0] + 1]:
    painted = [x for x in range(max(xs[0] - STEP, 0), min(xs[-1] + STEP, width)) if matches(row, x)]
    ink += [x for x in range(painted[0], painted[-1]) if not matches(row, x)]
if not ink:
    sys.exit("nothing drawn on the band of colour " + sys.argv[1])
print(min(ink), max(ink), y)
