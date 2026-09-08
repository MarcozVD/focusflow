"""Genera assets para Microsoft Partner Center (FocusFlow)."""
from PIL import Image
import os

ROOT = r"C:\Users\mvale\focusflow"
ICON = os.path.join(ROOT, "spike", "src-tauri", "icons", "icon.png")
DEMO = os.path.join(ROOT, "image.png")
OUT = os.path.join(ROOT, "store")
os.makedirs(OUT, exist_ok=True)

icon = Image.open(ICON).convert("RGBA")
demo = Image.open(DEMO).convert("RGBA")
print("icon:", icon.size, "demo:", demo.size)

def store_logo(src, size, name):
    """1:1 con fondo = color dominante del icono, icono centrado al 78%."""
    # fondo: muestrea esquina del icono
    bg = src.getpixel((5, 5))[:3]
    canvas = Image.new("RGBA", (size, size), bg + (255,))
    inner = int(size * 0.78)
    ic = src.resize((inner, inner), Image.LANCZOS)
    off = (size - inner) // 2
    canvas.paste(ic, (off, off), ic)
    p = os.path.join(OUT, name)
    canvas.convert("RGB").save(p, "PNG")
    print("->", p, canvas.size)

def poster(src, w, h, name):
    """2:3 póster: fondo degradado del color del icono + icono centrado."""
    bg = src.getpixel((5, 5))[:3]
    top = tuple(min(255, c + 25) for c in bg)
    canvas = Image.new("RGBA", (w, h))
    for y in range(h):
        t = y / h
        row = tuple(int(top[i] * (1 - t) + bg[i] * t) for i in range(3))
        for x in range(w):
            canvas.putpixel((x, y), row + (255,))
    inner = int(min(w, h) * 0.62)
    ic = src.resize((inner, inner), Image.LANCZOS)
    canvas.paste(ic, ((w - inner) // 2, (h - inner) // 2), ic)
    p = os.path.join(OUT, name)
    canvas.convert("RGB").save(p, "PNG")
    print("->", p, canvas.size)

def screenshot(src, min_w, name):
    """Escala la demo al mínimo admitido (1366 de ancho)."""
    w, h = src.size
    scale = max(min_w / w, 768 / h)
    img = src.resize((round(w * scale), round(h * scale)), Image.LANCZOS)
    p = os.path.join(OUT, name)
    img.convert("RGB").save(p, "PNG")
    print("->", p, img.size)

# Logotipo principal 1:1 (requerido)
store_logo(icon, 1080, "store-logo-1080x1080.png")
store_logo(icon, 2160, "store-logo-2160x2160.png")
# Póster 2:3 (recomendado)
poster(icon, 720, 1080, "poster-720x1080.png")
poster(icon, 1440, 2160, "poster-1440x2160.png")
# Captura provisional desde la demo del README
screenshot(demo, 1366, "screenshot-01-provisional.png")
