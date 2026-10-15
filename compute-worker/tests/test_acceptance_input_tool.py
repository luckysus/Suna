import importlib.util
import json
from pathlib import Path
import shutil
import pytest

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('acceptance_input',ROOT/'tools/windows-acceptance/scripts/verify-scientific-input.py')
tool=importlib.util.module_from_spec(spec);spec.loader.exec_module(tool)


def test_real_component_evidence_preserves_native_network_and_ci_boundaries():
    report=tool.component_evidence()
    assert len(report['mappingChecks'])==6 and len(report['fixtures'])==8
    assert report['calibration']['meanGray']==111.5
    assert report['componentState']=='passed'
    assert report['nativeAcceptance']==report['installedWorkerAcceptance']=='NOT_RUN'
    assert set(report['serviceChecks'].values())=={'pending-network'}
    assert set(report['nativeChecks'].values())=={'NOT_RUN'}
    assert report['stableReleaseAccepted'] is False
    assert report['ciConclusion']=='not-verified-by-this-tool'
    assert str(ROOT) not in json.dumps(report)


def test_source_hash_tampering_fails_without_output(tmp_path,monkeypatch):
    fixtures=tmp_path/'fixtures';shutil.copytree(tool.FIXTURES,fixtures)
    (fixtures/'gray16-be.tif').write_bytes(b'changed')
    with pytest.raises(tool.EvidenceError,match='scientific_fixture_hash_mismatch'):
        tool.component_evidence(fixtures)


def test_chinese_report_path_is_created_once_without_overwriting(tmp_path):
    output=tmp_path/'中文 像素验收.json'
    tool.write_new_report(output,tool.component_evidence())
    original=output.read_bytes()
    assert json.loads(original)['nativeAcceptance']=='NOT_RUN'
    with pytest.raises(tool.EvidenceError,match='scientific_output_already_exists'):
        tool.write_new_report(output,{'nativeAcceptance':'PASS'})
    assert output.read_bytes()==original


def test_relative_missing_and_parent_file_output_is_rejected(tmp_path):
    for output in [Path('relative.json'),tmp_path/'missing'/'record.json']:
        with pytest.raises(tool.EvidenceError):tool.write_new_report(output,{})
    parent=tmp_path/'file';parent.write_text('private fixture')
    with pytest.raises(tool.EvidenceError):tool.write_new_report(parent/'record.json',{})
    for name in ['NUL.json','COM1.txt','report.','record:stream']:
        with pytest.raises(tool.EvidenceError):tool.write_new_report(tmp_path/name,{})


def test_stale_installed_module_cannot_claim_checkout_source_verification(tmp_path,monkeypatch):
    import suna_worker.microstructure as analysis
    monkeypatch.setattr(analysis,'__file__',str(tmp_path/'stale-wheel'/'microstructure.py'))
    with pytest.raises(tool.EvidenceError,match='scientific_component_source_mismatch'):
        tool.component_evidence()


def test_checkout_mismatch_and_missing_git_cannot_report_clean_commit(monkeypatch):
    monkeypatch.setattr(tool.subprocess,'check_output',lambda *a,**k: (_ for _ in ()).throw(OSError('private path')))
    assert tool.checkout_identity(None)=={'gitHead':None,'checkoutDirty':True,'expectedCommitMatches':False,'cleanCommitVerified':False}
    with pytest.raises(tool.EvidenceError,match='scientific_checkout_commit_mismatch'):tool.checkout_identity('a'*40)


def test_command_executes_component_checks_without_claiming_native_or_real_services(tmp_path,capsys):
    output=tmp_path/'evidence.json'
    assert tool.main(['--output',str(output)])==0
    assert json.loads(capsys.readouterr().out)['nativeAcceptance']=='NOT_RUN'
    assert json.loads(output.read_text())['serviceChecks']['europe_pmc']=='pending-network'
    assert tool.main(['--output',str(output)])==1
    assert 'scientific_output_already_exists' in capsys.readouterr().out
