#!/usr/bin/env python3
"""Dessine l'icône de Taiseux (1024 px) sans dépendance : un visage qui se tait (deux yeux, une bouche fermée)
sur un carré arrondi vert. Antialiasing par champ de distance signé. Puis : cargo tauri icon design/icone.png"""
import math, struct, sys, zlib

N = 1024
FOND = (31, 95, 74)        # --accent
TRAIT = (247, 246, 242)    # --bg clair

def sd_rrect(x, y, cx, cy, hw, hh, r):
    qx, qy = abs(x - cx) - hw + r, abs(y - cy) - hh + r
    return math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - r

def couverture(d):  # d en pixels, négatif = dedans
    return max(0.0, min(1.0, 0.5 - d))

lignes = []
for j in range(N):
    y = j + 0.5
    row = bytearray([0])
    for i in range(N):
        x = i + 0.5
        a_fond = couverture(sd_rrect(x, y, 512, 512, 412, 412, 180))  # marge macOS : 100 px
        visage = min(
            math.hypot(x - 392, y - 430) - 54,                    # œil gauche
            math.hypot(x - 632, y - 430) - 54,                    # œil droit
            sd_rrect(x, y, 512, 640, 170, 30, 30),                # bouche fermée
        )
        a_trait = couverture(visage) * a_fond
        r = FOND[0] * (1 - a_trait) + TRAIT[0] * a_trait
        g = FOND[1] * (1 - a_trait) + TRAIT[1] * a_trait
        b = FOND[2] * (1 - a_trait) + TRAIT[2] * a_trait
        row += bytes((round(r), round(g), round(b), round(255 * a_fond)))
    lignes.append(bytes(row))

def bloc(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
png = b"\x89PNG\r\n\x1a\n" + bloc(b"IHDR", struct.pack(">IIBBBBB", N, N, 8, 6, 0, 0, 0)) + bloc(b"IDAT", zlib.compress(b"".join(lignes), 9)) + bloc(b"IEND", b"")
open(sys.argv[1] if len(sys.argv) > 1 else "icone.png", "wb").write(png)
