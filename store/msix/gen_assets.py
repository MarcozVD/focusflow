"""Genera los assets Square44x44/Square150x150/StoreLogo/etc para el MSIX de FocusFlow.

Origen: spike/src-tauri/icons/icon.png (512x512 RGBA, esquinas transparentes).
Patron del skill de Microsoft Store listing: fondo muestreado del propio icono,
icono centrado. Targetsize/plano -> icono grande; tiles 150+ -> 78% centrado.
"""
from collections import Counter
from pathlib import Path

from PIL import Image

SRC = Path(r"C:\Users\mvale\focusflow\spike\src-tauri\icons\icon.png")
OUT = Path(r"C:\Users\mvale\focusflow\store\msix\assets")
OUT.mkdir(parents=True, exist_ok=True)

icon = Image.open(SRC).convert("RGBA")

# Fondo: color opaco mas comun del icono (borde interior, sin AA del borde)
opaque = [p for p in icon.getdata() if p[3] > 250]
bg = Counter(opaque).most_common(1)[0][0][:3]
bg_hex = "#{:02X}{:02X}{:02X}".format(*bg)
print("bg:", bg_hex)


def card(size: int, fill_pct: float) -> Image.Image:
    im = Image.new("RGBA", (size, size), bg + (255,))
    inner = max(1, round(size * fill_pct))
    ic = icon.resize((inner, inner), Image.LANCZOS)
    off = (size - inner) // 2
    im.paste(ic, (off, off), ic)
    return im


# (nombre, lado_px, fill del icono)
targets = []
# Square44x44: scale (llenos) + targetsize (para barra de tareas/explorador)
for s in (100, 125, 150, 200):
    targets.append((f"Square44x44Logo.scale-{s}.png", round(44 * s / 100), 0.92))
for ts in (16, 24, 32, 48, 256):
    targets.append((f"Square44x44Logo.targetsize-{ts}.png", ts, 0.92))
for s in (100, 125, 150, 200):
    targets.append((f"Square150x150Logo.scale-{s}.png", round(150 * s / 100), 0.78))
targets.append(("Square310x310Logo.scale-100.png", 310, 0.78))
targets.append(("Square107x107Logo.scale-100.png", 107, 0.78))
for s in (100, 125, 150, 200):
    targets.append((f"StoreLogo.scale-{s}.png", round(50 * s / 100), 0.88))

for name, size, pct in targets:
    card(size, pct).save(OUT / name)
print(f"OK {len(targets)} assets -> {OUT}")
