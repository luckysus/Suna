import copy
import hashlib
import json

import pytest

from adapter import export_project_metadata, references_from_project_manifest
from package import inspect

PROJECT = "11111111-1111-4111-8111-111111111111"
DATASET = "22222222-2222-4222-8222-222222222222"
ARTIFACT = "33333333-3333-4333-8333-333333333333"


def manifest():
    payload = {"dataset_id": DATASET, "source_sha256": "a"*64, "columns": [{"canonical_field":"c","unit":"wt%"}],
               "source_path":"C:/synthetic-private/raw.csv", "raw_rows":[[987.123456]], "api_key":"synthetic-secret"}
    return {"schemaVersion":"research-manifest.v1","appVersion":"1.0.0","project":{"id":PROJECT,"name":"合成项目","description":"synthetic-private-description"},
            "objects":[{"kind":"dataset","id":DATASET,"title":"合成数据","availability":"available"}],
            "artifacts":[{"id":ARTIFACT,"kind":"dataset_snapshot","title":"数据版本","schemaVersion":"research-artifact.v1","contentSha256":hashlib.sha256(json.dumps(payload).encode()).hexdigest(),"payload":payload}],"edges":[]}


def test_actual_project_contract_exports_only_references_units_and_missing_asset_dependencies(tmp_path):
    source=manifest();before=copy.deepcopy(source);path=tmp_path/"项目元数据.zip"
    exported=export_project_metadata(source,path)
    assert inspect(path)==exported
    projection=references_from_project_manifest(source)
    assert projection["dependencies"]==[{"resource_id":DATASET,"kind":"dataset","status":"missing","expected_sha256":"a"*64}]
    artifact=next(body for identity,body,_ in exported["objects"] if identity==ARTIFACT)
    assert artifact["units"]==[{"name":"c","unit":"wt%"}]
    assert artifact["source_artifact_sha256"]==source["artifacts"][0]["contentSha256"]
    serialized=json.dumps(projection)
    for secret in ("synthetic-private","987.123456","synthetic-secret","raw_rows"):
        assert secret not in serialized
    assert source==before


def test_dangling_existing_project_edge_fails_with_clear_reference_error_before_writing(tmp_path):
    source=manifest();source["edges"]=[{"fromId":ARTIFACT,"toId":PROJECT,"relation":"uses_dataset"}]
    # The project entity exists; an edge outside the exported set is rejected.
    source["edges"][0]["toId"]="44444444-4444-4444-8444-444444444444"
    path=tmp_path/"rejected.zip"
    with pytest.raises(ValueError,match="lineage_missing_reference"):
        export_project_metadata(source,path)
    assert not path.exists()


def test_unknown_manifest_and_missing_original_artifact_digest_cannot_be_claimed_valid(tmp_path):
    source=manifest();source["schemaVersion"]="future"
    with pytest.raises(ValueError,match="unsupported_project_manifest"):
        export_project_metadata(source,tmp_path/"wrong.zip")
    source=manifest();source["artifacts"][0]["contentSha256"]="unknown"
    with pytest.raises(ValueError,match="project_artifact_digest_missing"):
        export_project_metadata(source,tmp_path/"wrong.zip")


def test_generated_artifact_links_are_merged_once_without_mutating_original_hashes(tmp_path):
    source=manifest()
    source["objects"].append({"kind":"artifact","id":ARTIFACT,"title":"已关联的数据快照","availability":"available"})
    source["objects"].append({"kind":"artifact","id":ARTIFACT,"title":"已关联的数据快照","relation":"attached","availability":"available"})
    projection=references_from_project_manifest(source)
    assert len(projection["records"])==3
    exported=export_project_metadata(source,tmp_path/"实际成果关联.zip")
    assert len(exported["objects"])==3
    artifact=next(body for identity,body,_ in exported["objects"] if identity==ARTIFACT)
    assert artifact["source_artifact_sha256"]==source["artifacts"][0]["contentSha256"]


def test_large_linked_project_budget_counts_distinct_records(tmp_path):
    import uuid
    source=manifest();source["objects"]=[];source["artifacts"]=[]
    for i in range(1100):
        identity=str(uuid.uuid5(uuid.NAMESPACE_URL,f"synthetic-artifact-{i}"))
        source["objects"].append({"kind":"artifact","id":identity,"title":"合成成果","availability":"available"})
        source["artifacts"].append({"id":identity,"kind":"analysis_result","title":"合成成果","schemaVersion":"research-artifact.v1","contentSha256":"a"*64,"payload":{}})
    exported=export_project_metadata(source,tmp_path/"千对象元数据.zip")
    assert len(exported["objects"])==1101


def test_different_object_kinds_cannot_share_one_identity_and_labels_hide_paths(tmp_path):
    source=manifest();source["objects"].append({"kind":"model","id":ARTIFACT,"title":"冲突"})
    with pytest.raises(ValueError,match="project_object_identity_conflict"):
        export_project_metadata(source,tmp_path/"conflict.zip")
    source=manifest();source["project"]["name"]="  /synthetic-private/project"
    assert references_from_project_manifest(source)["records"][0]["name"]=="未公开名称"
