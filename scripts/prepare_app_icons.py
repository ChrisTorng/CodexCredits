#!/usr/bin/env python3
"""Build transparent, high-resolution Android/Windows icons from shared brand.svg geometry."""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path("src-tauri/icons")
SIZE = 1024
SCALE = SIZE / 256
GOLD = (230, 180, 51, 255)
CYAN = (44, 183, 198, 255)
img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
draw = ImageDraw.Draw(img)

def pt(x, y):
    return (round(x * SCALE), round(y * SCALE))

def arc(cx, cy, radius, start, end, color):
    box = (cx-radius, cy-radius, cx+radius, cy+radius)
    draw.arc((*pt(box[0],box[1]), *pt(box[2],box[3])),
             start=start, end=end, fill=color, width=round(20*SCALE))

# Angles are clockwise in image coordinates. No white tile/background.
arc(148,104,75,225,428,GOLD)
arc(109,157,75,23,245,CYAN)
for x,y,color in ((148,104,GOLD),(109,157,CYAN)):
    r=16
    draw.ellipse((*pt(x-r,y-r),*pt(x+r,y+r)),fill=color)

ROOT.mkdir(parents=True,exist_ok=True)
for n, size in (("icon.png",512),("128x128.png",128),("32x32.png",32)):
    img.resize((size,size),Image.Resampling.LANCZOS).save(ROOT/n)
img.resize((256,256),Image.Resampling.LANCZOS).save(
    ROOT/"icon.ico",format="ICO",sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
with Image.open(ROOT/"icon.png") as icon:
    assert icon.getpixel((0,0))[3] == 0, "App icon background must be transparent"
    assert icon.getpixel((296,208))[3] > 0, "Gold center dot missing"
    assert icon.getpixel((218,314))[3] > 0, "Cyan center dot missing"
print("Transparent twin-loop icon built for Android, Windows and app header")
