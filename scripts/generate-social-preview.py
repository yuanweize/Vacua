#!/usr/bin/env python3
"""Generate GitHub Social Preview image for Vacua."""
import os
from PIL import Image, ImageDraw, ImageFont

def generate():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(script_dir)
    out_dir = os.path.join(repo_root, "assets", "social")
    os.makedirs(out_dir, exist_ok=True)
    out_file = os.path.join(out_dir, "github-social-preview.png")

    width, height = 1280, 640
    img = Image.new("RGBA", (width, height), (13, 17, 23, 255))
    draw = ImageDraw.Draw(img)

    # Subtle ambient radial glow
    cx, cy = 640, 210
    for r in range(320, 0, -16):
        alpha = int(14 * (1.0 - r / 320.0))
        draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(56, 139, 253, alpha))

    # Draw Vacua Mark
    mx, my, mr = 640, 180, 75
    draw.ellipse([mx - mr, my - mr, mx + mr, my + mr], outline=(245, 245, 247, 245), width=10)

    # Chevron V
    draw.line([(mx - 38, my - 30), (mx, my + 40)], fill=(245, 245, 247, 245), width=10)
    draw.line([(mx, my + 40), (mx + 38, my - 30)], fill=(245, 245, 247, 245), width=10)
    draw.ellipse([mx - 8, my - 14, mx + 8, my + 2], fill=(245, 245, 247, 245))

    # Fonts
    font_paths = [
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/System/Library/Fonts/Helvetica.ttc",
    ]
    font_title = None
    for fp in font_paths:
        if os.path.exists(fp):
            font_title = ImageFont.truetype(fp, 64)
            font_sub = ImageFont.truetype(fp, 28)
            font_tag = ImageFont.truetype(fp, 19)
            break
    if not font_title:
        font_title = font_sub = font_tag = ImageFont.load_default()

    # Title
    title = "Vacua"
    bbox_t = draw.textbbox((0, 0), title, font=font_title)
    tw = bbox_t[2] - bbox_t[0]
    draw.text((640 - tw // 2, 305), title, fill=(245, 245, 247, 255), font=font_title)

    # Subtitle
    sub = "Storage intelligence for macOS."
    bbox_s = draw.textbbox((0, 0), sub, font=font_sub)
    sw = bbox_s[2] - bbox_s[0]
    draw.text((640 - sw // 2, 395), sub, fill=(139, 148, 158, 255), font=font_sub)

    # Pill tags
    tag_text = "Rust   ·   APFS Extents   ·   Local-First Safety"
    bbox_tag = draw.textbbox((0, 0), tag_text, font=font_tag)
    tag_w = bbox_tag[2] - bbox_tag[0]
    tag_h = bbox_tag[3] - bbox_tag[1]
    px, py = 640 - tag_w // 2, 475
    draw.rounded_rectangle(
        [px - 24, py - 10, px + tag_w + 24, py + tag_h + 10],
        radius=16,
        fill=(22, 27, 34, 255),
        outline=(48, 54, 61, 255),
        width=2,
    )
    draw.text((px, py), tag_text, fill=(88, 166, 255, 255), font=font_tag)

    rgb_img = img.convert("RGB")
    rgb_img.save(out_file, "PNG", optimize=True)
    print(f"Generated {out_file} (1280x640)")

if __name__ == "__main__":
    generate()
