"""Generate the DigitsNepal source icon and menu-bar template icon.
Run: python3 scripts/make-icons.py && npx tauri icon src-tauri/icons/source.png
"""
from PIL import Image, ImageDraw, ImageFont
import os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "src-tauri", "icons")
FONT = "/System/Library/Fonts/Kohinoor.ttc"
os.makedirs(OUT, exist_ok=True)

def glyph_font(size):
    # Index 2 of Kohinoor.ttc is the semibold face on recent macOS.
    for idx in (2, 1, 0):
        try:
            return ImageFont.truetype(FONT, size, index=idx)
        except Exception:
            continue
    return ImageFont.load_default()

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
pad = 92
d.rounded_rectangle([pad, pad, S - pad, S - pad], radius=190, fill=(0, 56, 147, 255))  # Nepal-flag blue
inner = 26
d.rounded_rectangle([pad + inner, pad + inner, S - pad - inner, S - pad - inner], radius=166, outline=(220, 20, 60, 255), width=22)  # crimson rule
f = glyph_font(520)
text = "अ"
bbox = d.textbbox((0, 0), text, font=f)
w, h = bbox[2] - bbox[0], bbox[3] - bbox[1]
d.text(((S - w) / 2 - bbox[0], (S - h) / 2 - bbox[1] - 10), text, font=f, fill=(255, 255, 255, 255))
img.save(os.path.join(OUT, "source.png"))

# Menu-bar template icon: black glyph on transparent, 44px (@2x of 22pt).
T = 44
tray = Image.new("RGBA", (T, T), (0, 0, 0, 0))
td = ImageDraw.Draw(tray)
tf = glyph_font(34)
bb = td.textbbox((0, 0), text, font=tf)
tw, th = bb[2] - bb[0], bb[3] - bb[1]
td.text(((T - tw) / 2 - bb[0], (T - th) / 2 - bb[1]), text, font=tf, fill=(0, 0, 0, 255))
tray.save(os.path.join(OUT, "tray.png"))
print("ok")
