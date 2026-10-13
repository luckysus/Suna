"""Deterministic raster statistics. Threshold regions are not identified phases/grains."""
from __future__ import annotations
import hashlib
import io
import math
from pathlib import Path
from typing import Any, Callable
import numpy as np
from PIL import Image, __version__ as pillow_version
from scipy import ndimage, __version__ as scipy_version

VERSION = "microstructure.v1"
MAX_BYTES = 32 * 1024 * 1024
MAX_PIXELS = 16_000_000
Image.MAX_IMAGE_PIXELS = MAX_PIXELS


def roi_mask(shape: tuple[int, int], rois: list[dict]) -> np.ndarray:
    h, w = shape
    mask = np.zeros(shape, dtype=bool) if rois else np.ones(shape, dtype=bool)
    if len(rois) > 20:
        raise ValueError("roi_limit")
    for roi in rois:
        pts = np.asarray(roi["points"], dtype=float)
        if pts.ndim != 2 or pts.shape[1] != 2 or not np.isfinite(pts).all() or not (2 <= len(pts) <= 256):
            raise ValueError("invalid_roi_points")
        if (pts < 0).any() or (pts[:, 0] > w).any() or (pts[:, 1] > h).any():
            raise ValueError("roi_outside_image")
        if roi["kind"] == "rectangle" and len(pts) == 2:
            x0, y0 = pts.min(axis=0)
            x1, y1 = pts.max(axis=0)
            xs = (np.arange(w) + .5 >= x0) & (np.arange(w) + .5 < x1)
            ys = (np.arange(h) + .5 >= y0) & (np.arange(h) + .5 < y1)
            mask |= ys[:, None] & xs[None, :]
        elif roi["kind"] == "polygon" and len(pts) >= 3:
            # Pixel centers and even-odd filling; union prevents double counting.
            xs = np.arange(w)[None, :] + .5
            for row in range(0, h, 128):
                ys = np.arange(row, min(h, row + 128))[:, None] + .5
                inside = np.zeros((len(ys), w), dtype=bool)
                for a, b in zip(pts, np.roll(pts, -1, axis=0)):
                    if a[1] == b[1]:
                        continue
                    inside ^= ((a[1] > ys) != (b[1] > ys)) & (xs < (b[0]-a[0])*(ys-a[1])/(b[1]-a[1])+a[0])
                mask[row:row+len(ys)] |= inside
        else:
            raise ValueError("invalid_roi_kind")
    if not mask.any():
        raise ValueError("empty_roi")
    return mask


def analyze_bytes(data: bytes, annotation: dict[str, Any]) -> dict[str, Any]:
    if len(data) > MAX_BYTES:
        raise ValueError("image_byte_limit")
    with Image.open(io.BytesIO(data)) as image:
        w, h = image.size
        if image.format not in {"PNG", "JPEG"} or image.mode not in {"L", "LA", "RGB", "RGBA", "P"} or getattr(image, "n_frames", 1) != 1:
            raise ValueError("unsupported_format_or_depth")
        if w*h > MAX_PIXELS or max(w, h) > 8192:
            raise ValueError("image_pixel_limit")
        # Composite transparent pixels on white explicitly, matching native preview.
        rgba = image.convert("RGBA")
        rgb = Image.new("RGBA", rgba.size, "white")
        rgb.alpha_composite(rgba)
        gray = np.asarray(rgb.convert("L"))
    roi = roi_mask(gray.shape, annotation.get("rois", []))
    threshold = annotation.get("threshold", 128)
    if isinstance(threshold, bool) or not isinstance(threshold, int) or not 0 <= threshold <= 255:
        raise ValueError("invalid_threshold")
    scale = annotation.get("scaleMicronsPerPixel")
    if scale is not None and (isinstance(scale, bool) or not isinstance(scale, (int, float)) or not math.isfinite(scale) or scale <= 0):
        raise ValueError("invalid_scale")
    values = gray[roi]
    selected = (gray >= threshold) & roi
    metrics = {
        "roiArea": {"value": int(roi.sum()), "unit": "px²", "qualifiedName": "ROI 像素面积"},
        "thresholdFraction": {"value": float(selected.sum()/roi.sum()*100), "unit": "%", "qualifiedName": "阈值区域占比（灰度≥阈值）"},
        "meanGray": {"value": float(values.mean()), "unit": "gray/255", "qualifiedName": "平均灰度"},
        "contrastStd": {"value": float(values.std()), "unit": "gray/255", "qualifiedName": "灰度标准差"},
    }
    regions: list[dict] = []
    if annotation.get("maskConfirmed") is True:
        labels, count = ndimage.label(selected)  # fixed 4-connected pixels
        if count > 10_000:
            raise ValueError("region_count_limit")
        areas = np.bincount(labels.ravel())[1:]
        for area in areas:
            region = {"areaPixels": int(area), "equivalentDiameterPixels": float(math.sqrt(4*area/math.pi))}
            if scale is not None:
                region.update(areaSquareMicrons=float(area*scale*scale), equivalentDiameterMicrons=float(region["equivalentDiameterPixels"]*scale))
            regions.append(region)
    if scale is not None:
        metrics["roiPhysicalArea"] = {"value": float(roi.sum()*scale*scale), "unit": "μm²", "qualifiedName": "已标定 ROI 面积"}
    return {"algorithm": VERSION, "algorithmVersion": VERSION, "sourceSha256": hashlib.sha256(data).hexdigest(), "width": w, "height": h, "scaleMicronsPerPixel": scale, "roi": annotation.get("rois", []), "threshold": threshold, "histogram": np.bincount(values, minlength=256).tolist(), "metrics": metrics, "regions": regions, "reviewed": False, "limitations": ["阈值分割受腐蚀、照明和噪声影响", "连通区域不等同于晶粒或材料相", "透明像素合成白底；忽略 EXIF 旋转，坐标基于原始栅格"], "environment": {"numpy": np.__version__, "pillow": pillow_version, "scipy": scipy_version}}


def analyze_batch(payload: dict, report: Callable | None = None) -> dict:
    images = payload.get("images")
    if not isinstance(images, list) or not 1 <= len(images) <= 20:
        raise ValueError("image_batch_limit")
    results = []
    for n, item in enumerate(images):
        try:
            with Path(item["path"]).open("rb") as source:
                data = source.read(MAX_BYTES + 1)
            if hashlib.sha256(data).hexdigest() != item["sha256"]:
                raise ValueError("source_hash_changed")
            result = analyze_bytes(data, item["annotation"])
            results.append({"imageId": item["imageId"], "annotationId": item["annotationId"], "result": result})
        except (ValueError, OSError, KeyError, Image.DecompressionBombError) as error:
            # Stable error codes; do not expose internal paths or decoder messages.
            code = str(error) if isinstance(error, ValueError) and str(error) in {"source_hash_changed", "image_pixel_limit", "image_byte_limit", "invalid_roi_points", "roi_outside_image", "empty_roi", "invalid_threshold", "invalid_scale", "unsupported_format_or_depth", "region_count_limit", "invalid_roi_kind", "roi_limit"} else "image_decode_failed"
            results.append({"imageId": item.get("imageId"), "annotationId": item.get("annotationId"), "error": code})
        if report:
            report("image_analysis", int((n+1)/len(images)*95))
    return {"images": results, "algorithmVersion": VERSION}
