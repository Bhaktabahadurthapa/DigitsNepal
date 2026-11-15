"""Build full-bleed iPhone/PWA icons from the app icon's white glyph.

iOS masks Home Screen icons itself and fills transparency with black, so the
icons here are opaque squares: flag blue, a crimson inner rule, white glyph.
"""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src-tauri/icons/source.png"
OUT = ROOT / "pwa/public/icons"
BLUE, CRIMSON = (11, 61, 145, 255), (193, 39, 45, 255)

src = Image.open(SRC).convert("RGBA")
w, h = src.size
# White glyph only: bright, low-saturation pixels in the central area.
inner = src.crop((int(w * .2), int(h * .2), int(w * .8), int(h * .8)))
mask = Image.new("L", inner.size, 0)
px, mp = inner.load(), mask.load()
for y in range(inner.height):
    for x in range(inner.width):
        r, g, b, a = px[x, y]
        lo = min(r, g, b)
        mp[x, y] = max(0, min(255, (lo - 120) * 2)) if a > 0 else 0
glyph = Image.new("RGBA", inner.size, (255, 255, 255, 255))
glyph.putalpha(mask)
glyph = glyph.crop(mask.getbbox())

def icon(size: int, glyph_frac: float, ring: bool) -> Image.Image:
    s = size * 4
    im = Image.new("RGBA", (s, s), BLUE)
    if ring:
        d = ImageDraw.Draw(im)
        m, t = int(s * .085), max(4, int(s * .022))
        d.rounded_rectangle((m, m, s - m, s - m), radius=int(s * .16), outline=CRIMSON, width=t)
    g = glyph.copy()
    scale = (s * glyph_frac) / max(g.size)
    g = g.resize((int(g.width * scale), int(g.height * scale)), Image.LANCZOS)
    im.alpha_composite(g, ((s - g.width) // 2, (s - g.height) // 2 + int(s * .01)))
    return im.resize((size, size), Image.LANCZOS).convert("RGB")

OUT.mkdir(parents=True, exist_ok=True)
icon(180, .5, True).save(OUT / "apple-touch-icon.png", optimize=True)
icon(192, .5, True).save(OUT / "icon-192.png", optimize=True)
icon(512, .5, True).save(OUT / "icon-512.png", optimize=True)
icon(512, .4, False).save(OUT / "icon-maskable-512.png", optimize=True)
icon(64, .6, False).save(OUT / "favicon-64.png", optimize=True)
print("icons written to", OUT)
