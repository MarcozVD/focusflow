"""Descarga las bases generadas por IA y compone los assets de Store con tamaños exactos."""
import os
import urllib.request
from PIL import Image

TMP = r"C:\Users\mvale\focusflow\store\tmp"
OUT = r"C:\Users\mvale\focusflow\store"
os.makedirs(TMP, exist_ok=True)

URLS = {
    os.path.join(TMP, "hero_base.png"): "https://v3b.fal.media/files/b/0aa92443/v2UWVLBZtkeOSWQNqtykc_gBJkuAuD.png",
    os.path.join(TMP, "keyart_base.png"): "https://v3b.fal.media/files/b/0aa92441/PH-Nb-jh_dlvfbRLgmJjF_DZ9cnoAJ.png",
}
for path, url in URLS.items():
    if not os.path.exists(path):
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=90) as r, open(path, "wb") as f:
            f.write(r.read())
    print("base:", path, os.path.getsize(path))

icon = Image.open(r"C:\Users\mvale\focusflow\spike\src-tauri\icons\icon.png").convert("RGBA")

def cover(img, w, h):
    """Escala y recorta (cover) para llenar exactamente w x h."""
    sw, sh = img.size
    scale = max(w / sw, h / sh)
    img = img.resize((round(sw * scale), round(sh * scale)), Image.LANCZOS)
    left = (img.width - w) // 2
    top = (img.height - h) // 2
    return img.crop((left, top, left + w, top + h))

def with_icon(base, w, h, icon_frac, name):
    """Fondo cover + icono de la app centrado."""
    canvas = cover(base, w, h).convert("RGB")
    inner = int(min(w, h) * icon_frac)
    ic = icon.resize((inner, inner), Image.LANCZOS)
    canvas.paste(ic, ((w - inner) // 2, (h - inner) // 2), ic)
    p = os.path.join(OUT, name)
    canvas.save(p, "PNG")
    print("->", name, canvas.size, f"{os.path.getsize(p)/1024:.0f} KB")

def with_icon_and_title(base, w, h, name):
    """Fondo cover + banda inferior oscura degradada + icono + 'FocusFlow'."""
    canvas = cover(base, w, h).convert("RGB")
    band_h = int(h * 0.22)
    grad = Image.new("L", (1, band_h))
    for y in range(band_h):
        grad.putpixel((0, y), int(255 * (y / band_h) ** 1.2))
    dark = Image.new("RGB", (w, band_h), (7, 13, 26))
    canvas.paste(dark, (0, h - band_h), grad.resize((w, band_h)))
    inner = int(h * 0.14)
    ic = icon.resize((inner, inner), Image.LANCZOS)
    ix = int(w * 0.08)
    iy = h - band_h + (band_h - inner) // 2
    canvas.paste(ic, (ix, iy), ic)
    text = Image.new("RGBA", (w, band_h), (0, 0, 0, 0))
    from PIL import ImageDraw, ImageFont
    d = ImageDraw.Draw(text)
    fsize = int(h * 0.075)
    try:
        font = ImageFont.truetype("segoeuib.ttf", fsize)
    except OSError:
        font = ImageFont.load_default()
    tx = ix + inner + int(w * 0.03)
    ty = h - band_h + (band_h - fsize) // 2
    d.text((tx, ty), "FocusFlow", font=font, fill=(255, 255, 255, 255))
    canvas.paste(text, (0, h - band_h), text)
    p = os.path.join(OUT, name)
    canvas.save(p, "PNG")
    print("->", name, canvas.size, f"{os.path.getsize(p)/1024:.0f} KB")

hero = Image.open(os.path.join(TMP, "hero_base.png"))
keyart = Image.open(os.path.join(TMP, "keyart_base.png"))
print("hero base:", hero.size, "| keyart base:", keyart.size)

# Arte de superhéroe 16:9 (sin título del producto)
with_icon(hero, 1920, 1080, 0.30, "superhero-art-1920x1080.png")
with_icon(hero, 3840, 2160, 0.30, "superhero-art-3840x2160.png")
# Arte de héroe titulado 1920x1080 (SÍ lleva el título)
with_icon_and_title(hero, 1920, 1080, "titled-hero-art-1920x1080.png")
# Arte clave de marca 584x800
with_icon(keyart, 584, 800, 0.34, "brand-key-art-584x800.png")
