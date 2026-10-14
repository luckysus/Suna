"""Regenerate tiny original synthetic pixel fixtures; no third-party images.

Copyright-free pixel patterns dedicated to the public domain (CC0-1.0).
Expected values below are explicit goldens, not calculated by Suna's decoder.
Run with compute-worker/.venv/bin/python fixtures/microstructure/generate.py.
"""
import hashlib
import json
from pathlib import Path
import numpy as np
from PIL import Image, TiffImagePlugin

ROOT = Path(__file__).parent
PIXELS = [[0, 1000, 2000, 3000], [4000, 5000, 6000, 65535]]


def save(name, image, **kwargs):
    image.save(ROOT / name, **kwargs)


def generate():
    gray = np.array(PIXELS, dtype=np.uint16)
    save('gray16-le.tif', Image.fromarray(gray), format='TIFF', compression='raw')
    save('gray16-be.tif', Image.fromarray(gray.astype('>u2')), format='TIFF', compression='raw')
    save('gray16.png', Image.fromarray(gray), format='PNG')
    rgb = np.array([[[0,0,0],[255,0,0],[0,255,0],[0,0,255]],[[255,255,255],[255,255,0],[255,0,255],[0,255,255]]], dtype=np.uint8)
    save('rgb8.tif', Image.fromarray(rgb), format='TIFF', compression='raw')
    rotated = Image.fromarray(np.array([[0,64,128],[192,224,255]], dtype=np.uint8))
    exif = Image.Exif(); exif[274] = 6
    save('raw-orientation6.png', rotated, format='PNG', exif=exif)
    tags = TiffImagePlugin.ImageFileDirectory_v2(); tags[274] = 6
    save('unsupported-orientation6.tif', rotated, format='TIFF', tiffinfo=tags)
    save('unsupported-pages.tif', Image.fromarray(gray), format='TIFF', save_all=True, append_images=[Image.fromarray(gray)])
    save('unsupported-float.tif', Image.fromarray(gray.astype(np.float32)), format='TIFF')
    names = sorted(path.name for path in ROOT.iterdir() if path.suffix in {'.png','.tif'})
    golden = {'license':'CC0-1.0','sourcePixels16':PIXELS,
        'fullRangeGray8':[[0,3,7,11],[15,19,23,255]],
        'window1000to5000Gray8':[[0,0,63,127],[191,255,255,255]],
        'rgb8Gray':[[0,76,150,29],[255,226,105,179]],
        'rawOrientation6Pixels':[[0,64,128],[192,224,255]],
        'sourceSha256':{name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in names}}
    (ROOT/'golden.json').write_text(json.dumps(golden,indent=2)+'\n',encoding='utf-8')


if __name__ == '__main__':
    generate()
