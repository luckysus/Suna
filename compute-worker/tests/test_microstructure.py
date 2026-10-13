import hashlib
import io
import math
import numpy as np
import pytest
from PIL import Image
from suna_worker.microstructure import analyze_bytes, analyze_batch

def png():
    array=np.zeros((10,20),dtype=np.uint8);array[:,:10]=255
    stream=io.BytesIO();Image.fromarray(array).save(stream,format='PNG');return stream.getvalue()

def test_exact_threshold_area_scale_regions_and_uncalibrated_units():
    result=analyze_bytes(png(),{'threshold':128,'maskConfirmed':True,'scaleMicronsPerPixel':2.})
    assert result['metrics']['thresholdFraction']['value']==50
    assert sum(result['histogram'])==200
    assert result['regions'][0]['areaSquareMicrons']==400
    assert result['regions'][0]['equivalentDiameterMicrons']==pytest.approx(math.sqrt(400/math.pi)*2)
    uncalibrated=analyze_bytes(png(),{'threshold':128,'maskConfirmed':True})
    assert 'Microns' not in str(uncalibrated['regions'])
    assert all('μm' not in metric['unit'] for metric in uncalibrated['metrics'].values())
    assert not analyze_bytes(png(),{'threshold':128})['regions']

@pytest.mark.parametrize('kind,points',[('rectangle',[[0,0],[10,10]]),('polygon',[[0,0],[10,0],[10,10],[0,10]])])
def test_roi_pixel_centers_union_and_outside_exclusion(kind,points):
    roi={'kind':kind,'points':points}
    result=analyze_bytes(png(),{'rois':[roi,roi],'threshold':128})
    assert result['metrics']['roiArea']['value']==100
    assert result['metrics']['thresholdFraction']['value']==100

@pytest.mark.parametrize('params',[{'threshold':256},{'scaleMicronsPerPixel':0},{'scaleMicronsPerPixel':float('nan')},{'rois':[{'kind':'rectangle','points':[[-1,0],[10,10]]}]},{'rois':[{'kind':'polygon','points':[[0,0],[1,1]]}]}])
def test_reject_invalid_analysis(params):
    with pytest.raises(ValueError):analyze_bytes(png(),params)

def test_batch_preserves_individual_failures_and_source_hash(tmp_path):
    path=tmp_path/'合成图.png';path.write_bytes(png());sha=hashlib.sha256(png()).hexdigest()
    rows=analyze_batch({'images':[{'imageId':'one','annotationId':'a','path':str(path),'sha256':sha,'annotation':{}},{'imageId':'two','path':str(path),'sha256':'bad','annotation':{}}]})['images']
    assert rows[0]['result']['metrics']['thresholdFraction']['value']==50
    assert rows[1]['error']=='source_hash_changed'

def test_reject_corruption_and_16bit():
    with pytest.raises(OSError):analyze_bytes(b'broken',{})
    stream=io.BytesIO();Image.fromarray(np.zeros((2,2),dtype=np.uint16)).save(stream,format='PNG')
    with pytest.raises(ValueError):analyze_bytes(stream.getvalue(),{})
