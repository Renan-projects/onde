"""Génère assets/onde.ico (ondes concentriques sur un carré arrondi), sans dépendance."""
import math
import struct
import sys
import zlib


def png(size):
    rows = []
    c = (size - 1) / 2
    for y in range(size):
        row = bytearray([0])
        for x in range(size):
            dx, dy = (x - c) / size, (y - c) / size
            corner = math.hypot(max(abs(dx) - 0.30, 0), max(abs(dy) - 0.30, 0))
            a = 0
            if corner < 0.2:
                a = 255
                if corner > 0.2 - 1.2 / size:
                    a = int(255 * (0.2 - corner) * size / 1.2)
            t = (x + y) / (2 * size)
            base = (int(40 + 90 * t), int(20 + 60 * (1 - t)), int(110 + 100 * t))
            d = math.hypot(dx + 0.12, dy - 0.12)
            wave = 0.5 + 0.5 * math.cos(d * 55)
            ring = max(0.0, wave - 0.55) / 0.45 * max(0.0, 1 - d * 1.6)
            col = tuple(min(255, int(b + (255 - b) * ring)) for b in base)
            row += bytes(col) + bytes([a])
        rows.append(bytes(row))
    raw = b"".join(rows)

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


sizes = [16, 20, 24, 32, 48, 64, 256]
images = [png(s) for s in sizes]
out = struct.pack("<HHH", 0, 1, len(sizes))
offset = 6 + 16 * len(sizes)
for s, img in zip(sizes, images):
    out += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(img), offset)
    offset += len(img)
out += b"".join(images)
with open(sys.argv[1] if len(sys.argv) > 1 else "assets/onde.ico", "wb") as f:
    f.write(out)
