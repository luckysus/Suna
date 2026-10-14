import copy
import hashlib
import json
from pathlib import Path
import pytest
from suna_worker.microstructure import analyze_bytes, analyze_batch
from suna_worker.raster_input import decode_gray

ROOT = Path(__file__).resolve().parents[2] / 'fixtures' / 'microstructure'
GOLDEN = json.loads((ROOT/'golden.json').read_text())


def contract(format='TIFF', depth=16, *, window=False, width=4, height=2, orientation=1, color='gray'):
    return {'version':'microstructure-input.v1','format':format,'width':width,'height':height,
        'decodedBitDepth':depth,'colorModel':color,'sourceExifOrientation':orientation,
        'orientationPolicy':'ignore-exif','coordinateSystem':'raw-raster',
        'alphaPolicy':'white-composite','iccPolicy':'unmanaged',
        'grayscaleMapping':None if depth==8 else {'version':'linear-u16.v1','mode':'window' if window else 'full_range',
            'lower':1000 if window else 0,'upper':5000 if window else 65535,'rounding':'floor'}}


@pytest.mark.parametrize('name',['gray16-le.tif','gray16-be.tif','gray16.png'])
@pytest.mark.parametrize('window',[False,True])
def test_real_16bit_pixels_match_independent_goldens(name,window):
    data=(ROOT/name).read_bytes()
    assert hashlib.sha256(data).hexdigest()==GOLDEN['sourceSha256'][name]
    meta=contract('PNG' if name.endswith('.png') else 'TIFF',window=window)
    pixels,w,h=decode_gray(data,meta)
    assert (w,h)==(4,2)
    assert pixels.tolist()==GOLDEN['window1000to5000Gray8' if window else 'fullRangeGray8']
    result=analyze_bytes(data,{'threshold':128,'maskConfirmed':True},meta)
    assert result['algorithm']=='microstructure.v2'
    assert result['sourceSha256']==GOLDEN['sourceSha256'][name]
    assert result['rasterInput']==meta
    assert result['metrics']['thresholdFraction']['value']==(50 if window else 12.5)
    assert 'roiPhysicalArea' not in result['metrics']


def test_raw_coordinates_and_calibration_are_not_rotated_or_guessed():
    data=(ROOT/'gray16-le.tif').read_bytes()
    a={'threshold':128,'rois':[{'kind':'rectangle','points':[[0,0],[2,2]]}],
       'scaleMicronsPerPixel':2.,'maskConfirmed':True}
    result=analyze_bytes(data,a,contract(window=True))
    assert result['metrics']['meanGray']['value']==111.5
    assert result['metrics']['roiArea']['value']==4
    assert result['metrics']['roiPhysicalArea']['value']==16
    assert result['regions'][0]['areaSquareMicrons']==8
    raw=(ROOT/'raw-orientation6.png').read_bytes()
    pixels,w,h=decode_gray(raw,contract('PNG',8,width=3,orientation=6))
    assert (w,h)==(3,2)
    assert pixels.tolist()==GOLDEN['rawOrientation6Pixels']


def test_rgb_tiff_and_legacy_8bit_semantics():
    pixels,_,_=decode_gray((ROOT/'rgb8.tif').read_bytes(),contract(depth=8,color='rgb'))
    assert pixels.tolist()==GOLDEN['rgb8Gray']
    raw=(ROOT/'raw-orientation6.png').read_bytes()
    legacy=analyze_bytes(raw,{'threshold':128})
    current=analyze_bytes(raw,{'threshold':128},contract('PNG',8,width=3,orientation=6))
    assert legacy['algorithm']=='microstructure.v1'
    for key in ['histogram','metrics','regions','roi','width','height']:
        assert legacy[key]==current[key]


@pytest.mark.parametrize('mutate',[
    lambda c:c.update(version='unknown'),lambda c:c.update(width=3),
    lambda c:c.update(decodedBitDepth=8),lambda c:c.update(orientationPolicy='auto'),
    lambda c:c.update(sourceExifOrientation=6),lambda c:c.update(grayscaleMapping=None),
    lambda c:c['grayscaleMapping'].update(lower=True),lambda c:c['grayscaleMapping'].update(upper=0),
    lambda c:c['grayscaleMapping'].update(upper=65536),lambda c:c['grayscaleMapping'].update(lower=1),
    lambda c:c['grayscaleMapping'].update(rounding='nearest')])
def test_contract_cannot_silently_change_mapping_or_coordinates(mutate):
    meta=contract();mutate(meta)
    with pytest.raises(ValueError):decode_gray((ROOT/'gray16-le.tif').read_bytes(),meta)


@pytest.mark.parametrize('name',['unsupported-pages.tif','unsupported-orientation6.tif','unsupported-float.tif'])
def test_unsupported_tiff_is_not_silently_reinterpreted(name):
    with pytest.raises(ValueError):decode_gray((ROOT/name).read_bytes(),contract())


def test_batch_binds_original_source_and_keeps_per_image_errors(tmp_path):
    data=(ROOT/'gray16-le.tif').read_bytes();path=tmp_path/'原始图.tif';path.write_bytes(data)
    row={'imageId':'i','annotationId':'a','path':str(path),'sha256':hashlib.sha256(data).hexdigest(),
        'annotation':{},'rasterInput':contract()}
    invalid=copy.deepcopy(row);invalid['imageId']='bad';invalid['rasterInput']['grayscaleMapping']=None
    result=analyze_batch({'images':[row,invalid]})
    assert result['algorithmVersion']=='microstructure.v2'
    assert result['images'][0]['result']['sourceSha256']==row['sha256']
    assert result['images'][1]['error']=='gray16_mapping_required'
    assert path.read_bytes()==data
