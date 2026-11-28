"""Checks every frame of a recording of examples/native-view-apple (docs/native-view-apple.md#checks): each frame
must show the native view's colour, the GPUI div's colour over it, and no gap pixel (the gap colour anywhere, or
black bordering the view inside the app's content: a hole with nothing behind it), so a frame where hole and view
part, or where either is missing, fails. Black that reaches the frame's edge or the top band is not the app's: the
window around a turning screen, or the iPhone's Dynamic Island.

The recording is anything ffmpeg reads: `xcrun simctl io <device> recordVideo` (iOS simulator), `screencapture -v`
(macOS: the example's colours are the same there), or a numbered frame series (`frames/f%04d.png`). Frames are
halved, as compression's blur at full size is no gap.

Usage: python3 -I scripts/apple-native-view-frames.py <recording>
Prints one summary line, and each failing frame with what it missed; exits 1 if any frame fails or none decodes.
Needs ffmpeg and ffprobe on PATH.
"""

import json
import subprocess
import sys

# Colour classes: a pixel is in a class when every channel is in that class's range. To add one, add a row and give
# it the next bit. The view turns through hues 90-210 at full saturation and value (apple.rs HUES): red at most
# half, green or blue full; the div is DIV_COLOUR (e07020); the gap is GAP_COLOUR (ff00ff). Ranges allow for
# H.264's colour shifts.
# (name, bit, (red, green, blue) ranges, inclusive)
CLASSES = [
    ("view (green side)", 1, ((0, 150), (190, 255), (0, 255))),
    ("view (blue side)", 2, ((0, 150), (0, 255), (190, 255))),
    ("div", 4, ((180, 255), (80, 150), (0, 70))),
    ("gap colour", 8, ((180, 255), (0, 90), (180, 255))),
    ("black", 16, ((0, 12), (0, 12), (0, 12))),
]
VIEW, DIV, GAP, BLACK = 1 | 2, 4, 8, 16
# What each frame must show, as shares of the frame's pixels: the view's smallest slot less the div is 9% of an
# iPhone frame and 3% of an iPad one; the div 3% and 1%.
MIN_VIEW = 0.01
MIN_DIV = 0.003
BORDER = 3  # pixels from the view or div within which black counts as a gap
HOUSING = 0.06  # share of the frame's height at its top holding the Dynamic Island (recordings stay portrait)


def channel_table(channel):
    """byte value -> the bits of the classes whose range on this channel holds it"""
    return bytes(
        sum(bit for _, bit, ranges in CLASSES if ranges[channel][0] <= value <= ranges[channel][1])
        for value in range(256)
    )


TABLES = [channel_table(channel) for channel in range(3)]
# mask byte -> 1 if it has any of the bits
HAS = {bits: bytes(int(value & bits != 0) for value in range(256)) for bits in (VIEW, DIV, GAP, BLACK, VIEW | DIV)}


def classify(frame, pixels):
    """One mask byte per pixel: the bits of every class the pixel is in."""
    mask = -1
    for channel in range(3):
        mask &= int.from_bytes(frame[channel::3].translate(TABLES[channel]), "little")
    return mask.to_bytes(pixels, "little")


def enclosed(black, starts, width, height):
    """How many of the `starts` lie in black regions that reach neither the frame's edge nor the top band."""
    pixels, top = width * height, int(HOUSING * height) * width
    region = bytearray(pixels)  # 0 unseen, 1 the app's, 2 outside it
    for start in starts:
        if region[start]:
            continue
        members, stack, outside = [start], [start], False
        region[start] = 1
        while stack:
            at = stack.pop()
            x = at % width
            outside |= at < top or at >= pixels - width or x in (0, width - 1)
            for step, inside in ((-1, x > 0), (1, x < width - 1), (-width, at >= width), (width, at < pixels - width)):
                if inside and black[at + step] and not region[at + step]:
                    region[at + step] = 1
                    members.append(at + step)
                    stack.append(at + step)
        if outside:
            for at in members:
                region[at] = 2
    return sum(region[start] == 1 for start in starts)


def check(mask, width, height):
    """What the frame misses, as a list of reasons; empty when it passes."""
    pixels = width * height
    view = mask.translate(HAS[VIEW]).count(1)
    div = mask.translate(HAS[DIV]).count(1)
    gap = mask.translate(HAS[GAP]).count(1)
    # The view and div grown by BORDER pixels each way (0/1 bytes as one integer: shifting it by whole bytes moves
    # every pixel at once), then the black pixels in it that are the app's.
    shown = int.from_bytes(mask.translate(HAS[VIEW | DIV]), "little")
    near = shown
    for step in [*range(1, BORDER + 1), *range(width, (BORDER + 1) * width, width)]:
        near |= shown << 8 * step | shown >> 8 * step
    is_black = mask.translate(HAS[BLACK])
    bordering = (near & int.from_bytes(is_black, "little")).to_bytes(pixels, "little")
    starts, at = [], bordering.find(1)
    while at >= 0:
        starts.append(at)
        at = bordering.find(1, at + 1)
    black = enclosed(is_black, starts, width, height)
    # (reason, failed)
    rules = [
        (f"view {view} px < {MIN_VIEW:.0%}", view < MIN_VIEW * pixels),
        (f"div {div} px < {MIN_DIV:.1%}", div < MIN_DIV * pixels),
        (f"gap colour {gap} px", gap > 0),
        (f"black {black} px bordering the view", black > 0),
    ]
    return [reason for reason, failed in rules if failed], gap + black


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    recording = sys.argv[1]
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "json", recording],
        capture_output=True,
        check=True,
    )
    stream = json.loads(probe.stdout)["streams"][0]
    width, height = stream["width"] // 2, stream["height"] // 2
    pixels = width * height
    decode = subprocess.Popen(
        ["ffmpeg", "-loglevel", "fatal", "-i", recording, "-fps_mode", "passthrough",
         "-vf", f"scale={width}:{height}", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        stdout=subprocess.PIPE,
    )
    frames, failed, worst = 0, 0, 0
    while frame := decode.stdout.read(pixels * 3):
        if len(frame) < pixels * 3:
            break
        frames += 1
        reasons, gap = check(classify(frame, pixels), width, height)
        worst = max(worst, gap)
        if reasons:
            failed += 1
            print(f"  frame {frames}: {', '.join(reasons)}")
    if decode.wait() != 0:
        sys.exit(f"apple-native-view-frames: ffmpeg could not decode {recording}")
    print(f"apple-native-view-frames: {recording}: {frames} frames, {failed} failing, most gap pixels in one {worst}")
    sys.exit(1 if failed or not frames else 0)


main()
