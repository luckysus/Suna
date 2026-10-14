"""Versioned, lossless-source raster input shared with the native import contract.

No auto contrast, ICC conversion, orientation transform or magnification inference.
The only 16-bit derivative is explicit clamped integer linear mapping to gray8.
"""
from __future__ import annotations
import io
import numpy as np
from PIL import Image

INPUT_VERSION = "microstructure-input.v1"


def decode_gray(data: bytes, contract: dict | None) -> tuple[np.ndarray, int, int]:
    with Image.open(io.BytesIO(data)) as image:
        w, h = image.size
        if w <= 0 or h <= 0 or w*h > 16_000_000 or max(w, h) > 8192:
            raise ValueError("image_pixel_limit")
        if getattr(image, "n_frames", 1) != 1:
            raise ValueError("unsupported_format_or_depth")
        depth16 = image.mode in {"I;16", "I;16L", "I;16B", "I;16N"}
        if contract is None:
            if image.format not in {"PNG", "JPEG"} or image.mode not in {"L", "LA", "RGB", "RGBA", "P"}:
                raise ValueError("unsupported_format_or_depth")
        else:
            if not isinstance(contract, dict) or any(contract.get(k) != v for k, v in {
                "version": INPUT_VERSION, "format": image.format, "width": w, "height": h,
                "decodedBitDepth": 16 if depth16 else 8,
                "colorModel": "gray" if depth16 or image.mode in {"L", "LA"} else "rgb",
                "coordinateSystem": "raw-raster", "orientationPolicy": "ignore-exif",
                "alphaPolicy": "white-composite", "iccPolicy": "unmanaged",
                "sourceExifOrientation": image.getexif().get(274, 1),
            }.items()):
                raise ValueError("image_input_contract_mismatch")
            if image.format not in {"PNG", "JPEG", "TIFF"} or image.mode not in {"L", "LA", "RGB", "RGBA", "P", "I;16", "I;16L", "I;16B", "I;16N"}:
                raise ValueError("unsupported_format_or_depth")
            if image.format == "TIFF":
                # Pillow applies TIFF Orientation on load. Native rejects it rather
                # than risking a silent change to existing raw-raster ROI semantics.
                if image.getexif().get(274, 1) != 1:
                    raise ValueError("unsupported_tiff_orientation")
                if image.mode not in {"L", "RGB", "I;16", "I;16L", "I;16B", "I;16N"} or image.tag_v2.get(262) not in {1, 2} or image.tag_v2.get(284, 1) != 1 or 330 in image.tag_v2:
                    raise ValueError("unsupported_tiff_layout")
        if depth16:
            mapping = contract.get("grayscaleMapping") if contract else None
            if not isinstance(mapping, dict) or mapping.get("version") != "linear-u16.v1" or mapping.get("rounding") != "floor":
                raise ValueError("gray16_mapping_required")
            lower, upper = mapping.get("lower"), mapping.get("upper")
            if type(lower) is not int or type(upper) is not int or not 0 <= lower < upper <= 65535 or mapping.get("mode") not in {"full_range", "window"} or (mapping["mode"] == "full_range" and (lower, upper) != (0, 65535)):
                raise ValueError("invalid_gray16_window")
            values = np.asarray(image).astype(np.uint32)
            gray = ((np.clip(values, lower, upper) - lower) * 255 // (upper - lower)).astype(np.uint8)
        else:
            if contract is not None and contract.get("grayscaleMapping") is not None:
                raise ValueError("image_input_contract_mismatch")
            rgba = image.convert("RGBA")
            rgb = Image.new("RGBA", rgba.size, "white")
            rgb.alpha_composite(rgba)
            gray = np.asarray(rgb.convert("L"))
    return gray, w, h
