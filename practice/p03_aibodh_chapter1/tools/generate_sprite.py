"""Generate Chapter 3's original spritesheets with only the Python standard library."""
import math
from pathlib import Path
import struct
import zlib

TILE = 64
COLS, ROWS = 9, 12
WIDTH, HEIGHT = COLS * TILE, ROWS * TILE
OUT = Path(__file__).resolve().parents[1] / 'src/assets/characters'

# name, skin, hair, outfit, headwear (hood, headband, hat, helmet, or none)
CHARACTERS = [
    ('adventurer', (242, 184, 134), (89, 54, 39), (30, 139, 173), None),
    ('scout', (170, 109, 74), (59, 36, 34), (85, 164, 76), 'headband'),
    ('crimson_count', (238, 210, 209), (62, 35, 52), (173, 37, 65), None),
    ('graveyard_reaper', (195, 203, 161), (54, 37, 66), (127, 78, 158), 'hood'),
    ('lantern_warden', (215, 151, 96), (110, 72, 48), (213, 134, 42), 'helmet'),
    ('starlit_oracle', (192, 164, 221), (222, 213, 248), (76, 81, 193), 'hat'),
]


def generate(name, skin, hair, outfit, headwear):
    pixels = bytearray(WIDTH * HEIGHT * 4)
    outline = (35, 35, 50)

    def rect(x, y, w, h, color):
        line = bytes((*color, 255)) * w
        for py in range(y, y + h):
            offset = (py * WIDTH + x) * 4
            pixels[offset:offset + len(line)] = line

    for row in range(ROWS):
        action, facing = divmod(row, 4)  # Walk, Run, Jump; Up, Left, Down, Right.
        frames = (9, 8, 6)[action]
        for frame in range(frames):
            x, y = frame * TILE, row * TILE
            phase = frame / (frames - 1) if action == 2 else frame / frames
            stride = round((3 if action == 0 else 6) * math.sin(phase * math.tau))
            tuck = round(5 * math.sin(phase * math.pi)) if action == 2 else 0
            if action == 2:
                stride = tuck // 2
            bob = frame % 2 if action < 2 else -(tuck // 2)
            arm = -tuck * 2 if action == 2 else stride
            rect(x + 25, y + 39, 6, 12 + stride - tuck, (58, 68, 110))
            rect(x + 33, y + 39, 6, 12 - stride - tuck, (58, 68, 110))
            rect(x + 23, y + 49 + stride - tuck, 8, 4, (66, 44, 40))
            rect(x + 33, y + 49 - stride - tuck, 8, 4, (66, 44, 40))
            rect(x + 21, y + 24 + bob, 22, 18, outline)
            rect(x + 23, y + 26 + bob, 18, 14, outfit)
            rect(x + 17, y + 28 + arm, 5, 11, skin)
            rect(x + 42, y + 28 + (arm if action == 2 else -arm), 5, 11, skin)
            rect(x + 23, y + 9 + bob, 18, 17, outline)
            rect(x + 25, y + 11 + bob, 14, 13, skin)
            rect(x + 24, y + 9 + bob, 16, 5, hair)
            if facing == 0:
                rect(x + 25, y + 13 + bob, 14, 9, hair)
                rect(x + 31, y + 27 + bob, 2, 12, outline)
            elif facing == 2:
                rect(x + 27, y + 17 + bob, 2, 2, outline)
                rect(x + 35, y + 17 + bob, 2, 2, outline)
                rect(x + 23, y + 38, 18, 3, outline)
            else:
                left = facing == 1
                rect(x + (22 if left else 39), y + 18 + bob, 3, 4, skin)
                rect(x + (26 if left else 36), y + 16 + bob, 2, 2, outline)
            if headwear == 'hood':
                rect(x + 22, y + 7 + bob, 20, 7, outfit)
                rect(x + 22, y + 13 + bob, 3, 12, outfit)
                rect(x + 39, y + 13 + bob, 3, 12, outfit)
            elif headwear == 'headband':
                rect(x + 23, y + 13 + bob, 18, 3, outfit)
            elif headwear == 'helmet':
                rect(x + 22, y + 7 + bob, 20, 8, (155, 151, 146))
                rect(x + 30, y + 12 + bob, 4, 5, (249, 183, 59))
            elif headwear == 'hat':
                rect(x + 25, y + 6 + bob, 14, 8, outfit)
                rect(x + 20, y + 13 + bob, 24, 3, outfit)

    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))

    raw = b''.join(b'\0' + pixels[y * WIDTH * 4:(y + 1) * WIDTH * 4] for y in range(HEIGHT))
    png = b'\x89PNG\r\n\x1a\n'
    png += chunk(b'IHDR', struct.pack('>IIBBBBB', WIDTH, HEIGHT, 8, 6, 0, 0, 0))
    png += chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b'')
    (OUT / f'{name}.png').write_bytes(png)


if __name__ == '__main__':
    OUT.mkdir(parents=True, exist_ok=True)
    for character in CHARACTERS:
        generate(*character)
