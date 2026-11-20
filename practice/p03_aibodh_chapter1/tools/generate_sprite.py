"""Generate an original pixel adventurer using only Python's standard library.

No LPC art is copied. Grid matches the tutorial source image: 13x54 64px tiles;
walking rows 8..11 contain 9 frames. Run this script from any working directory.
"""
import math
from pathlib import Path
import struct
import zlib

TILE = 64
WIDTH, HEIGHT = 13 * TILE, 54 * TILE
pixels = bytearray(WIDTH * HEIGHT * 4)


def rect(x, y, w, h, color):
    for py in range(y, y + h):
        for px in range(x, x + w):
            offset = (py * WIDTH + px) * 4
            pixels[offset:offset + 4] = bytes(color)


outline = (35, 35, 50, 255)
skin = (242, 184, 134, 255)
hair = (89, 54, 39, 255)
shirt = (30, 139, 173, 255)
pants = (58, 68, 110, 255)
boots = (66, 44, 40, 255)

for row in range(8, 12):
    for frame in range(9):
        x, y = frame * TILE, row * TILE
        stride = 0 if frame == 0 else round(3 * math.sin((frame - 1) * math.tau / 8))
        bob = 0 if frame == 0 else frame % 2
        # The character has a visible silhouette in every direction.
        rect(x + 25, y + 39, 6, 12 + stride, pants)
        rect(x + 33, y + 39, 6, 12 - stride, pants)
        rect(x + 23, y + 49 + stride, 8, 4, boots)
        rect(x + 33, y + 49 - stride, 8, 4, boots)
        rect(x + 21, y + 24 + bob, 22, 18, outline)
        rect(x + 23, y + 26 + bob, 18, 14, shirt)
        rect(x + 17, y + 28 + stride, 5, 11, skin)
        rect(x + 42, y + 28 - stride, 5, 11, skin)
        rect(x + 23, y + 9 + bob, 18, 17, outline)
        rect(x + 25, y + 11 + bob, 14, 13, skin)
        rect(x + 24, y + 9 + bob, 16, 5, hair)
        if row == 8:  # Back of head and shirt seam.
            rect(x + 25, y + 13 + bob, 14, 9, hair)
            rect(x + 31, y + 27 + bob, 2, 12, outline)
        elif row == 10:  # Front eyes and belt.
            rect(x + 27, y + 17 + bob, 2, 2, outline)
            rect(x + 35, y + 17 + bob, 2, 2, outline)
            rect(x + 23, y + 38, 18, 3, outline)
        else:  # Profile nose and one eye.
            left = row == 9
            rect(x + (22 if left else 39), y + 18 + bob, 3, 4, skin)
            rect(x + (26 if left else 36), y + 16 + bob, 2, 2, outline)


def chunk(kind, data):
    return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))


raw = b''.join(b'\0' + pixels[y * WIDTH * 4:(y + 1) * WIDTH * 4] for y in range(HEIGHT))
png = b'\x89PNG\r\n\x1a\n'
png += chunk(b'IHDR', struct.pack('>IIBBBBB', WIDTH, HEIGHT, 8, 6, 0, 0, 0))
png += chunk(b'IDAT', zlib.compress(raw, 9))
png += chunk(b'IEND', b'')
path = Path(__file__).resolve().parents[1] / 'src/assets/player_spritesheet.png'
path.write_bytes(png)
print(f'Generated {path.name}: {WIDTH}x{HEIGHT}, {len(png)} bytes')
