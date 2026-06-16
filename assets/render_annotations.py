#!/usr/bin/env python3
"""Render annotations from a ui_annotator JSON file onto an image.

Usage:
    python render_annotations.py image.jpg annotations.json output.png

This is the batch counterpart to assets/ui_annotator.html:
- ui_annotator.html is for humans to draw boxes.
- render_annotations.py is for AI to visualise the resulting JSON.
"""

import json
import sys
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

COLORS = {
    "red": (255, 60, 60),
    "green": (0, 255, 0),
    "blue": (0, 160, 255),
    "yellow": (255, 220, 0),
}

ROLE_COLORS = {
    "chrome": "red",
    "content": "green",
    "rail": "blue",
    "floating": "yellow",
}


def main():
    if len(sys.argv) < 4:
        print("Usage: render_annotations.py <image> <annotations.json> <output>")
        sys.exit(1)

    image_path = Path(sys.argv[1])
    json_path = Path(sys.argv[2])
    output_path = Path(sys.argv[3])

    img = Image.open(image_path)
    draw = ImageDraw.Draw(img)

    # Prefer a CJK-capable system font so Chinese labels render correctly.
    font_candidates = [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/simhei.ttf",
        "C:/Windows/Fonts/simsun.ttc",
        "arial.ttf",
    ]
    chosen = None
    for candidate in font_candidates:
        try:
            ImageFont.truetype(candidate, 18)
            chosen = candidate
            break
        except Exception:
            continue
    if chosen:
        font = ImageFont.truetype(chosen, 18)
        small = ImageFont.truetype(chosen, 14)
    else:
        font = ImageFont.load_default()
        small = font

    data = json.loads(json_path.read_text(encoding="utf-8"))
    for ann in data.get("annotations", []):
        color_key = ann.get("color") or ROLE_COLORS.get(ann.get("role", "content"), "green")
        color = COLORS.get(color_key, (255, 255, 255))
        x, y, w, h = ann["x"], ann["y"], ann["w"], ann["h"]

        # border
        draw.rectangle([x, y, x + w, y + h], outline=color, width=3)

        # label background
        label = ann.get("label", "")
        if label:
            text = f"{label} ({ann.get('role','')})"
            bbox = draw.textbbox((0, 0), text, font=font)
            tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
            draw.rectangle([x, y, x + tw + 8, y + th + 6], fill=(0, 0, 0, 180))
            draw.text((x + 4, y + 3), text, fill=color, font=font)

        # dimensions
        dim = f"{int(w)}×{int(h)}"
        draw.text((x + 4, y + h - 18), dim, fill=color, font=small)

        # note
        note = ann.get("note", "")
        if note:
            draw.text((x + 4, y + h + 4), note, fill=color, font=small)

    img.save(output_path)
    print(f"Saved: {output_path}")


if __name__ == "__main__":
    main()
