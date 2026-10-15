#!/usr/bin/env python3
"""Verify source Python components against CC0 synthetic pixels, never native acceptance.

Run explicitly with the existing compute-worker venv. No network, credentials,
user databases, application, installer, or packaged Worker executable is opened.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

REPO = Path(__file__).resolve().parents[3]
FIXTURES = REPO / 'fixtures' / 'microstructure'
NAMES = ('gray16-le.tif','gray16-be.tif','gray16.png','rgb8.tif','raw-orientation6.png',
         'unsupported-pages.tif','unsupported-orientation6.tif','unsupported-float.tif')


class EvidenceError(Exception):
    pass


def read_regular(path: Path, limit: int = 1024 * 1024) -> bytes:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise EvidenceError('scientific_fixture_not_regular_or_bounded')
    with path.open('rb') as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise EvidenceError('scientific_fixture_not_regular_or_bounded')
    return data


def input_contract(format: str, window: bool = False, depth: int = 16, width: int = 4, orientation: int = 1, color: str = 'gray') -> dict:
    return {'version':'microstructure-input.v1','format':format,'width':width,'height':2,
        'decodedBitDepth':depth,'colorModel':color,'sourceExifOrientation':orientation,
        'orientationPolicy':'ignore-exif','coordinateSystem':'raw-raster',
        'alphaPolicy':'white-composite','iccPolicy':'unmanaged','grayscaleMapping':None if depth==8 else {
            'version':'linear-u16.v1','mode':'window' if window else 'full_range',
            'lower':1000 if window else 0,'upper':5000 if window else 65535,'rounding':'floor'}}


def component_evidence(fixtures: Path = FIXTURES) -> dict:
    import suna_worker.microstructure as analysis
    import suna_worker.raster_input as raster
    from suna_worker.microstructure import analyze_bytes
    from suna_worker.raster_input import decode_gray
    expected=(REPO/'compute-worker'/'suna_worker').resolve()
    if any(Path(module.__file__).resolve().parent!=expected for module in (analysis,raster)):
        raise EvidenceError('scientific_component_source_mismatch')
    source_hashes={name:hashlib.sha256(read_regular(expected/name)).hexdigest() for name in ('microstructure.py','raster_input.py')}
    golden = json.loads(read_regular(fixtures/'golden.json'))
    data = {name:read_regular(fixtures/name) for name in NAMES}
    hashes = {name:hashlib.sha256(content).hexdigest() for name,content in data.items()}
    if hashes != golden.get('sourceSha256') or golden.get('license') != 'CC0-1.0':
        raise EvidenceError('scientific_fixture_hash_mismatch')
    checks = []
    for name in NAMES[:3]:
        for window in [False, True]:
            meta = input_contract('PNG' if name.endswith('.png') else 'TIFF', window)
            pixels,w,h = decode_gray(data[name],meta)
            expected = golden['window1000to5000Gray8' if window else 'fullRangeGray8']
            result = analyze_bytes(data[name],{'threshold':128},meta)
            if (w,h)!=(4,2) or pixels.tolist()!=expected or result['sourceSha256']!=hashes[name] or result['metrics']['thresholdFraction']['value']!=(50 if window else 12.5) or result['rasterInput']!=meta or 'roiPhysicalArea' in result['metrics']:
                raise EvidenceError('scientific_component_golden_mismatch')
            checks.append({'fixture':name,'mapping':meta['grayscaleMapping'],'sourceSha256':hashes[name],
                'thresholdFraction':result['metrics']['thresholdFraction']['value'],'state':'passed'})
    roi = {'threshold':128,'rois':[{'kind':'rectangle','points':[[0,0],[2,2]]}],
           'scaleMicronsPerPixel':2.,'maskConfirmed':True}
    stats = analyze_bytes(data['gray16-le.tif'],roi,input_contract('TIFF',True))
    if (stats['metrics']['roiArea']['value'],stats['metrics']['roiPhysicalArea']['value'],stats['metrics']['meanGray']['value'],stats['regions'][0]['areaSquareMicrons']) != (4,16,111.5,8):
        raise EvidenceError('scientific_component_calibration_mismatch')
    for name,meta,key in [('rgb8.tif',input_contract('TIFF',depth=8,color='rgb'),'rgb8Gray'),
                          ('raw-orientation6.png',input_contract('PNG',depth=8,width=3,orientation=6),'rawOrientation6Pixels')]:
        pixels,_,_ = decode_gray(data[name],meta)
        if pixels.tolist()!=golden[key]:
            raise EvidenceError('scientific_component_golden_mismatch')
    for name in NAMES[5:]:
        try:
            decode_gray(data[name],input_contract('TIFF'))
        except ValueError:
            continue
        raise EvidenceError('scientific_unsupported_input_not_rejected')
    if any(read_regular(fixtures/name)!=content for name,content in data.items()):
        raise EvidenceError('scientific_source_changed')
    return {'schemaVersion':'scientific-input-components.v1','scope':'source_python_and_synthetic_pixels_only',
        'componentState':'passed','componentSha256':source_hashes,'fixtures':hashes,'mappingChecks':checks,
        'calibration':{'roiPixels':4,'roiSquareMicrons':16,'meanGray':111.5,'regionSquareMicrons':8},
        'nativeAcceptance':'NOT_RUN','installedWorkerAcceptance':'NOT_RUN','stableReleaseAccepted':False,
        'serviceChecks':{'crossref':'pending-network','europe_pmc':'pending-network','modelApi':'pending-network','sqlServer':'pending-network'},
        'nativeChecks':{'installation':'NOT_RUN','credentialManager':'NOT_RUN','dialogs':'NOT_RUN','dpi':'NOT_RUN','backupRelocation':'NOT_RUN'},
        'ciConclusion':'not-verified-by-this-tool','containsCredentials':False,'containsOriginalData':False,
        'platform':platform.system(),'pythonVersion':platform.python_version()}


def checkout_identity(expected: str | None) -> dict:
    try:
        revision = subprocess.check_output(['git','-C',str(REPO),'rev-parse','HEAD'],text=True,stderr=subprocess.DEVNULL,timeout=5).strip()
        dirty = bool(subprocess.check_output(['git','-C',str(REPO),'status','--porcelain'],text=True,stderr=subprocess.DEVNULL,timeout=5).strip())
    except (OSError,subprocess.SubprocessError):
        revision,dirty = None,True
    if expected and (not re.fullmatch('[a-f0-9]{40}',expected) or expected!=revision):
        raise EvidenceError('scientific_checkout_commit_mismatch')
    return {'gitHead':revision,'checkoutDirty':dirty,'expectedCommitMatches':bool(expected and expected==revision),
            'cleanCommitVerified':bool(expected and expected==revision and not dirty)}


def write_new_report(path: Path, report: dict) -> None:
    raw = str(path)
    if not path.is_absolute() or raw.startswith(('\\\\','//')) or '..' in path.parts:
        raise EvidenceError('scientific_output_not_local_absolute')
    if os.name=='nt' and (not re.match(r'^[A-Za-z]:[\\/]',raw) or ':' in raw[2:]):
        raise EvidenceError('scientific_output_not_local_absolute')
    for name in path.parts[1:]:
        if re.search(r'[<>:"|?*\x00-\x1f]',name) or name.endswith(('.', ' ')) or re.match(r'^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)',name,re.I):
            raise EvidenceError('scientific_output_unsafe_name')
    for parent in path.parents:
        if not parent.is_dir() or parent.is_symlink() or getattr(parent,'is_junction',lambda:False)():
            raise EvidenceError('scientific_output_parent_not_regular')
    try:
        with path.open('x',encoding='utf-8',newline='\n') as output:
            json.dump(report,output,ensure_ascii=False,indent=2,allow_nan=False)
            output.write('\n')
    except FileExistsError as error:
        raise EvidenceError('scientific_output_already_exists') from error


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',required=True,type=Path)
    parser.add_argument('--expected-commit')
    args = parser.parse_args(argv)
    try:
        identity = checkout_identity(args.expected_commit)
        report = component_evidence()
        report['checkout'] = identity
        report['toolSha256'] = hashlib.sha256(read_regular(Path(__file__))).hexdigest()
        write_new_report(args.output,report)
    except EvidenceError as error:
        print(json.dumps({'state':'failed','code':str(error)}));return 1
    except Exception:
        print(json.dumps({'state':'failed','code':'scientific_component_verification_failed'}));return 1
    print(json.dumps({'state':'component_checks_passed','nativeAcceptance':'NOT_RUN','ciConclusion':'not-verified-by-this-tool'}))
    return 0


if __name__=='__main__':
    sys.exit(main())
