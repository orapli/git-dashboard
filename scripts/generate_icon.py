#!/usr/bin/env python3
"""Generate assets/icon.png (256x256) and assets/icon.ico for the app.

Pure-stdlib (zlib/struct) so it runs anywhere. The artwork is the app motif:
a rounded square in the accent blue (#528bff) with a white commit-graph glyph
(two nodes on a trunk plus one branch node).
"""
import struct
import zlib
import math
import os

SIZE = 256
ACCENT = (82, 139, 255)   # #528bff
WHITE = (255, 255, 255)
CORNER_R = 56.0


def rounded_rect_alpha(x, y):
    """1.0 inside the rounded square, 0.0 outside (1px soft edge), via SDF."""
    pad = 8.0
    half = (SIZE - 2 * pad) / 2.0
    cx, cy = SIZE / 2.0, SIZE / 2.0
    qx = abs(x - cx) - (half - CORNER_R)
    qy = abs(y - cy) - (half - CORNER_R)
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    inside = min(max(qx, qy), 0.0)
    sdf = outside + inside - CORNER_R  # negative inside
    return max(0.0, min(1.0, 0.5 - sdf))


def seg_dist(px, py, ax, ay, bx, by):
    """Distance from point to segment."""
    vx, vy = bx - ax, by - ay
    t = ((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy)
    t = max(0.0, min(1.0, t))
    return math.hypot(px - (ax + vx * t), py - (ay + vy * t))


# Commit-graph glyph geometry (in 256px space)
TRUNK_X = 104
N1 = (TRUNK_X, 76)     # top node
N2 = (TRUNK_X, 180)    # bottom node
N3 = (172, 128)        # branch node
NODE_R = 17.0
RING_W = 11.0
LINE_W = 11.0


def glyph_alpha(x, y):
    # connector lines (trunk + branch)
    line = 0.0
    d = seg_dist(x, y, N1[0], N1[1], N2[0], N2[1])
    line = max(line, min(1.0, LINE_W / 2.0 - d + 0.5))
    d = seg_dist(x, y, TRUNK_X, 150, N3[0], N3[1])
    line = max(line, min(1.0, LINE_W / 2.0 - d + 0.5))
    # mask lines out inside the nodes so they don't poke into the ring holes
    for nx, ny in (N1, N2, N3):
        d = math.hypot(x - nx, y - ny)
        line *= max(0.0, min(1.0, d - NODE_R + 0.5))
    a = max(0.0, line)
    # nodes as rings (read better at small sizes than filled dots)
    for nx, ny in (N1, N2, N3):
        d = math.hypot(x - nx, y - ny)
        ring = min(1.0, NODE_R - d + 0.5) * min(1.0, d - (NODE_R - RING_W) + 0.5)
        a = max(a, max(0.0, ring))
    return max(0.0, min(1.0, a))


def build_pixels():
    rows = []
    for y in range(SIZE):
        row = bytearray()
        for x in range(SIZE):
            bg_a = rounded_rect_alpha(x + 0.5, y + 0.5)
            g_a = glyph_alpha(x + 0.5, y + 0.5) * bg_a
            r = ACCENT[0] * (1 - g_a) + WHITE[0] * g_a
            g = ACCENT[1] * (1 - g_a) + WHITE[1] * g_a
            b = ACCENT[2] * (1 - g_a) + WHITE[2] * g_a
            row += bytes((int(r), int(g), int(b), int(bg_a * 255)))
        rows.append(bytes(row))
    return rows


def write_png(path, rows):
    raw = b"".join(b"\x00" + r for r in rows)

    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)
    return png


def write_ico(path, png_bytes):
    # Modern ICO: a single 256x256 PNG-compressed entry
    header = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack(
        "<BBBBHHII",
        0, 0,            # width/height: 0 means 256
        0, 0,            # palette, reserved
        1, 32,           # planes, bpp
        len(png_bytes),  # data size
        6 + 16,          # data offset
    )
    with open(path, "wb") as f:
        f.write(header + entry + png_bytes)


def write_rgba_64(path, rows):
    """Box-downsample 256→64 and write raw RGBA bytes (embedded as the
    runtime window icon: no PNG decoder needed in the app)."""
    out = bytearray()
    for oy in range(64):
        for ox in range(64):
            acc = [0, 0, 0, 0]
            for sy in range(4):
                row = rows[oy * 4 + sy]
                for sx in range(4):
                    base = (ox * 4 + sx) * 4
                    for ch in range(4):
                        acc[ch] += row[base + ch]
            out += bytes(v // 16 for v in acc)
    with open(path, "wb") as f:
        f.write(out)


def main():
    out_dir = os.path.join(os.path.dirname(__file__), "..", "assets")
    os.makedirs(out_dir, exist_ok=True)
    rows = build_pixels()
    png = write_png(os.path.join(out_dir, "icon.png"), rows)
    write_ico(os.path.join(out_dir, "icon.ico"), png)
    write_rgba_64(os.path.join(out_dir, "icon-64.rgba"), rows)
    print("wrote assets/icon.png, assets/icon.ico, assets/icon-64.rgba")


if __name__ == "__main__":
    main()
