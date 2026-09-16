#!/usr/bin/env python3
"""Export runtime icons from the selected, cropped source image (requires Pillow)."""

from pathlib import Path

from PIL import Image


ASSETS = Path(__file__).resolve().parent
WINDOWS_SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256)


def resize_icon(source: Image.Image, size: int) -> Image.Image:
    image = source.resize((size, size), Image.Resampling.LANCZOS)
    # Suppress almost-transparent Lanczos ringing outside the rounded tile.
    image.putalpha(image.getchannel("A").point(lambda alpha: 0 if alpha <= 4 else alpha))
    return image


def main() -> None:
    with Image.open(ASSETS / "app-icon-source.png") as image:
        source = image.convert("RGBA")

    if source.width != source.height:
        raise ValueError("The source icon must be square")

    resize_icon(source, 64).save(ASSETS / "tray-icon.png", optimize=True)

    frames = [resize_icon(source, size) for size in WINDOWS_SIZES]
    frames[-1].save(
        ASSETS / "app-icon.ico",
        format="ICO",
        sizes=[(size, size) for size in WINDOWS_SIZES],
        append_images=frames[:-1],
    )

    print("Exported tray-icon.png (64px) and app-icon.ico (16–256px)")


if __name__ == "__main__":
    main()
