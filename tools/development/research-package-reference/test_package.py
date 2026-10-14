import hashlib
import json
import sqlite3
import zipfile

import pytest

from package import VERSION, import_into_temporary_database, inspect, write_metadata_package

PROJECT = "11111111-1111-4111-8111-111111111111"
RESOURCE = "22222222-2222-4222-8222-222222222222"


def fixture(path, *, extras=(), mutate_hash=False, version=VERSION, private=False):
    body = {"id": RESOURCE, "kind": "model", "name": "合成元数据", "review_status": "unreviewed"}
    if private:
        body["source_path"] = "C:/private/raw.csv"
    data = json.dumps(body, ensure_ascii=False).encode()
    manifest = {"schema_version": version, "scope": "metadata_only", "source_project_id": PROJECT,
                "entries": [{"name": "records/模型.json", "bytes": len(data), "sha256": "0" * 64 if mutate_hash else hashlib.sha256(data).hexdigest()}],
                "external_dependencies": [{"resource_id": "external-dataset", "status": "missing"}]}
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr("manifest.json", json.dumps(manifest))
        archive.writestr("records/模型.json", data)
        for name, content in extras:
            archive.writestr(name, content)
    return path


def test_temporary_import_persists_metadata_and_is_idempotent(tmp_path):
    archive = fixture(tmp_path / "中文 研究包.zip")
    package = inspect(archive)
    db_path = tmp_path / "temporary.sqlite3"
    connection = sqlite3.connect(db_path)
    mapping = import_into_temporary_database(connection, package)
    assert mapping[RESOURCE] != RESOURCE
    assert import_into_temporary_database(connection, package) == mapping
    connection.close()
    reopened = sqlite3.connect(db_path)
    assert reopened.execute("SELECT count(*) FROM package_metadata").fetchone()[0] == 1
    assert reopened.execute("SELECT original_id,status FROM package_metadata").fetchone() == (RESOURCE, "metadata_only_missing_assets")
    assert package["missing_dependencies"][0]["status"] == "missing"
    reopened.close()


@pytest.mark.parametrize("name", ["../escape.json", "/absolute.json", "C:/escape.json", "records\\escape.json", "records/file.json:secret", "records/NUL.json"])
def test_unsafe_zip_names_are_rejected_without_extraction(tmp_path, name):
    with pytest.raises(ValueError, match="unsafe_zip_path"):
        inspect(fixture(tmp_path / "bad.zip", extras=[(name, "{}")] ))


def test_duplicate_case_aliases_and_symlinks_are_rejected(tmp_path):
    with pytest.raises(ValueError, match="duplicate_zip_entry"):
        inspect(fixture(tmp_path / "case.zip", extras=[("RECORDS/模型.json", "{}")] ))
    link = zipfile.ZipInfo("link.json")
    link.create_system = 3
    link.external_attr = 0o120777 << 16
    with pytest.raises(ValueError, match="zip_non_regular_file"):
        inspect(fixture(tmp_path / "link.zip", extras=[(link, "outside")] ))


def test_hash_schema_and_private_fields_are_rejected(tmp_path):
    for option, error in [({"mutate_hash": True}, "package_hash_mismatch"), ({"version": "future"}, "unsupported_package_schema"), ({"private": True}, "sensitive_metadata_field")]:
        with pytest.raises(ValueError, match=error):
            inspect(fixture(tmp_path / "bad.zip", **option))


def test_compression_expansion_is_bounded(tmp_path):
    path = tmp_path / "bomb.zip"
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("manifest.json", "x" * 200_000)
    with pytest.raises(ValueError, match="package_expansion_budget"):
        inspect(path)


def test_live_database_schema_is_never_modified(tmp_path):
    package = inspect(fixture(tmp_path / "ok.zip"))
    c = sqlite3.connect(tmp_path / "protected.sqlite3")
    c.execute("CREATE TABLE research_projects(id TEXT PRIMARY KEY)")
    c.execute("INSERT INTO research_projects VALUES(?)", (PROJECT,))
    c.commit()
    with pytest.raises(ValueError, match="live_database_import_forbidden"):
        import_into_temporary_database(c, package)
    assert c.execute("SELECT id FROM research_projects").fetchall() == [(PROJECT,)]
    assert c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall() == [("research_projects",)]
    c.close()


def test_thousand_metadata_records_preserve_hashes_edges_and_missing_status(tmp_path):
    ids = [str(__import__("uuid").uuid5(__import__("uuid").NAMESPACE_URL, f"synthetic:{i}")) for i in range(1000)]
    records = [{"id": identity, "kind": "artifact", "name": "合成来源", "source_artifact_sha256": "a" * 64, "references": [ids[i-1]] if i else []} for i, identity in enumerate(ids)]
    edges = [{"from_id": ids[i-1], "to_id": ids[i], "relation": "uses_model"} for i in range(1, len(ids))]
    path = tmp_path / "千对象研究包.zip"
    checked = write_metadata_package(path, PROJECT, records, edges=edges)
    assert len(checked["objects"]) == 1000
    assert len(checked["edges"]) == 999
    c = sqlite3.connect(tmp_path / "isolated.sqlite3")
    mapping = import_into_temporary_database(c, inspect(path))
    assert len(mapping) == 1000
    assert c.execute("SELECT count(*) FROM package_edges").fetchone()[0] == 999
    assert c.execute("SELECT DISTINCT status FROM package_metadata").fetchall() == [("metadata_only_missing_assets",)]
    c.close()


def test_missing_references_cycles_and_duplicate_edges_are_rejected_before_write(tmp_path):
    second = "33333333-3333-4333-8333-333333333333"
    base = [{"id": RESOURCE, "kind": "model"}, {"id": second, "kind": "artifact"}]
    one = {"from_id": RESOURCE, "to_id": second, "relation": "uses_model"}
    reverse = {"from_id": second, "to_id": RESOURCE, "relation": "uses_model"}
    for edges, error in [([one, reverse], "lineage_cycle"), ([one, one], "duplicate_lineage_edge"), ([{**one, "to_id": PROJECT}], "lineage_missing_reference")]:
        path = tmp_path / "rejected.zip"
        with pytest.raises(ValueError, match=error):
            write_metadata_package(path, PROJECT, base, edges=edges)
        assert not path.exists()
    circular = [{**base[0], "references": [second]}, {**base[1], "references": [RESOURCE]}]
    with pytest.raises(ValueError, match="lineage_cycle"):
        write_metadata_package(tmp_path / "references.zip", PROJECT, circular)


def test_external_references_remain_missing_and_exports_never_overwrite(tmp_path):
    path = tmp_path / "中文.zip"
    records = [{"id": RESOURCE, "kind": "model", "references": ["external-dataset"]}]
    checked = write_metadata_package(path, PROJECT, records, dependencies=[{"resource_id": "external-dataset", "status": "missing"}])
    assert checked["missing_dependencies"][0]["status"] == "missing"
    saved = path.read_bytes()
    with pytest.raises(FileExistsError):
        write_metadata_package(path, PROJECT, records, dependencies=[{"resource_id": "external-dataset", "status": "missing"}])
    assert path.read_bytes() == saved


def test_duplicate_json_keys_and_nonfinite_numbers_are_rejected(tmp_path):
    for data, error in [(b'{"schema_version":"one","schema_version":"two"}', "duplicate_json_field"), (b'{"value":NaN}', "non_finite_metadata_number")]:
        path = tmp_path / "bad-json.zip"
        with zipfile.ZipFile(path, "w") as archive:
            archive.writestr("manifest.json", data)
        with pytest.raises(ValueError, match=error):
            inspect(path)


def test_failed_temporary_import_rolls_back_receipt_schema_and_rows(tmp_path):
    package = inspect(fixture(tmp_path / "ok.zip"))
    c = sqlite3.connect(tmp_path / "rollback.sqlite3")
    # Fail only at the resource insertion, after receipt/edge-table creation.
    c.execute("CREATE TABLE package_metadata(id TEXT PRIMARY KEY, original_id TEXT, receipt_sha256 TEXT, source_sha256 TEXT, metadata_json TEXT, status TEXT CHECK(status='blocked'))")
    c.commit()
    with pytest.raises(sqlite3.IntegrityError):
        import_into_temporary_database(c, package)
    assert c.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall() == [("package_metadata",)]
    assert c.execute("SELECT count(*) FROM package_metadata").fetchone()[0] == 0
    c.close()
