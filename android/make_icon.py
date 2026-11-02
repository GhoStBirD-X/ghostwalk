"""Draws the GhostWalk launcher icon (orange ghost on white) as a PNG, using only the standard library."""
import struct, sys, zlib

def png(path, size):
    px = []
    s = size
    for y in range(s):
        row = bytearray([0])
        for x in range(s):
            fx, fy = (x + 0.5) / s, (y + 0.5) / s
            r, g, b, a = 255, 255, 255, 255
            # rounded square background
            cx, cy = min(max(fx, 0.18), 0.82), min(max(fy, 0.18), 0.82)
            if (fx - cx) ** 2 + (fy - cy) ** 2 > 0.18 ** 2:
                a = 0
            ghost = False
            if (fx - 0.5) ** 2 + (fy - 0.45) ** 2 < 0.25 ** 2 and fy < 0.45:
                ghost = True
            if 0.25 <= fx <= 0.75 and 0.45 <= fy <= 0.7:
                ghost = True
            for i in range(3):
                bx = 0.25 + 0.5 / 6 + i * 0.5 / 3
                if (fx - bx) ** 2 + (fy - 0.7) ** 2 < (0.5 / 6) ** 2:
                    ghost = True
            if ghost and a:
                r, g, b = 255, 92, 38
                for ex in (0.405, 0.595):
                    if (fx - ex) ** 2 + (fy - 0.44) ** 2 < 0.045 ** 2:
                        r, g, b = 255, 255, 255
            row += bytes([r, g, b, a])
        px.append(bytes(row))
    raw = b"".join(px)
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", s, s, 8, 6, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(data)

png(sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 192)
