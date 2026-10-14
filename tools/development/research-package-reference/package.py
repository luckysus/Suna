"""Metadata package validator and temporary SQLite import prototype.

No live Suna database, model loader, original source file or credential access.
This is not the workspace backup format or a complete asset restoration tool.
"""
from __future__ import annotations

import hashlib
import json
import math
import stat
import unicodedata
import uuid
import zipfile
from pathlib import Path

VERSION = "research-package-manifest.draft.v1"
FORBIDDEN = {"credentials", "password", "api_key", "source_path", "model_base64", "model_pickle_base64", "raw_rows", "absolute_path"}
RELATIONS = {"uses_dataset", "uses_model", "generated_by_run", "based_on_evidence",
             "recommends_experiment", "measured_against", "supersedes", "included_in_report"}


def _json(data):
    def pairs(items):
        body = {}
        for key, value in items:
            if key in body:
                raise ValueError("duplicate_json_field")
            body[key] = value
        return body

    def invalid_constant(_):
        raise ValueError("non_finite_metadata_number")
    return json.loads(data, object_pairs_hook=pairs, parse_constant=invalid_constant)


def _lineage(objects, dependencies, edges):
    if not isinstance(dependencies, list) or len(dependencies) > 2048:
        raise ValueError("external_dependency_schema")
    external = set()
    for dep in dependencies:
        if not isinstance(dep, dict) or set(dep) - {"resource_id", "status", "kind", "expected_sha256", "unit"}:
            raise ValueError("external_dependency_schema")
        resource = dep.get("resource_id")
        if not isinstance(resource, str) or not resource or len(resource) > 128 or resource in external or dep.get("status") != "missing":
            raise ValueError("external_dependency_not_reference_only")
        if dep.get("expected_sha256") is not None and not _digest(dep["expected_sha256"]):
            raise ValueError("external_dependency_schema")
        external.add(resource)
    ids = {original for original, _, _ in objects}
    for _, body, _ in objects:
        refs = body.get("references", [])
        if not isinstance(refs, list) or len(refs) > 2048 or any(not isinstance(ref, str) for ref in refs):
            raise ValueError("lineage_reference_schema")
        if any(ref not in ids | external for ref in refs):
            raise ValueError("lineage_missing_reference")
    if not isinstance(edges, list) or len(edges) > 10000:
        raise ValueError("lineage_edge_budget")
    adjacency = {key: set() for key in ids}
    degrees = {key: 0 for key in ids}
    for original, body, _ in objects:
        for source in body.get("references", []):
            if source == original:
                raise ValueError("lineage_cycle")
            if source in ids and original not in adjacency[source]:
                adjacency[source].add(original)
                degrees[original] += 1
    seen = set()
    for edge in edges:
        if not isinstance(edge, dict) or set(edge) != {"from_id", "to_id", "relation"}:
            raise ValueError("lineage_edge_schema")
        source, target, relation = edge["from_id"], edge["to_id"], edge["relation"]
        if not isinstance(source, str) or not isinstance(target, str) or not isinstance(relation, str) or relation not in RELATIONS:
            raise ValueError("lineage_edge_schema")
        if source not in ids or target not in ids:
            raise ValueError("lineage_missing_reference")
        if source == target:
            raise ValueError("lineage_cycle")
        key = (source, target, relation)
        if key in seen:
            raise ValueError("duplicate_lineage_edge")
        seen.add(key)
        if target not in adjacency[source]:
            adjacency[source].add(target)
            degrees[target] += 1
    ready = [key for key, degree in degrees.items() if degree == 0]
    visited = 0
    while ready:
        node = ready.pop()
        visited += 1
        for target in adjacency[node]:
            degrees[target] -= 1
            if degrees[target] == 0:
                ready.append(target)
    if visited != len(ids):
        raise ValueError("lineage_cycle")
    return edges


def _digest(value):
    return isinstance(value, str) and len(value) == 64 and all(c in "0123456789abcdef" for c in value)


def _metadata(value, depth=0):
    if depth > 32:
        raise ValueError("metadata_depth_budget")
    if isinstance(value, dict):
        if any(str(key).casefold() in FORBIDDEN for key in value):
            raise ValueError("sensitive_metadata_field")
        for item in value.values():
            _metadata(item, depth + 1)
    elif isinstance(value, list):
        if len(value) > 10000:
            raise ValueError("metadata_array_budget")
        for item in value:
            _metadata(item, depth + 1)
    elif isinstance(value, float) and not math.isfinite(value):
        raise ValueError("non_finite_metadata_number")


def _name(name):
    if not isinstance(name, str):
        raise ValueError("unsafe_zip_path")
    parts = name.split("/")
    if not name or any(part in ("", ".", "..") for part in parts) or "\\" in name or ":" in name:
        raise ValueError("unsafe_zip_path")
    if any(part.endswith((".", " ")) or any(ord(c) < 32 for c in part) for part in parts):
        raise ValueError("unsafe_zip_path")
    for part in parts:
        stem = part.split(".")[0].upper()
        if stem in {"CON", "PRN", "AUX", "NUL", *("COM" + str(i) for i in range(1, 10)), *("LPT" + str(i) for i in range(1, 10))}:
            raise ValueError("unsafe_zip_path")
    if not name.endswith(".json"):
        raise ValueError("metadata_only_file_type")
    return unicodedata.normalize("NFC", name).casefold()


def inspect(path: Path):
    with zipfile.ZipFile(path) as archive:
        infos = archive.infolist()
        if not 1 <= len(infos) <= 2048 or sum(info.file_size for info in infos) > 64 * 1024 * 1024:
            raise ValueError("package_size_budget")
        seen = set()
        for info in infos:
            key = _name(info.filename)
            if key in seen:
                raise ValueError("duplicate_zip_entry")
            seen.add(key)
            mode = stat.S_IFMT(info.external_attr >> 16)
            if mode not in (0, stat.S_IFREG):
                raise ValueError("zip_non_regular_file")
            if info.file_size > 8 * 1024 * 1024 or info.file_size > max(info.compress_size, 1) * 128:
                raise ValueError("package_expansion_budget")
        manifest = _json(archive.read("manifest.json"))
        if not isinstance(manifest, dict) or set(manifest) - {"schema_version", "scope", "source_project_id", "entries", "external_dependencies", "edges", "app_version"}:
            raise ValueError("unsupported_package_schema")
        if manifest.get("schema_version") != VERSION or manifest.get("scope") != "metadata_only":
            raise ValueError("unsupported_package_schema")
        source_id = str(uuid.UUID(manifest["source_project_id"]))
        _metadata(manifest)
        declared = manifest.get("entries")
        if not isinstance(declared, list) or len(declared) != len(infos) - 1:
            raise ValueError("package_entry_set_mismatch")
        expected = {"manifest.json"}
        objects = []
        ids = set()
        for entry in declared:
            if not isinstance(entry, dict) or set(entry) != {"name", "bytes", "sha256"} or not isinstance(entry["bytes"], int) or isinstance(entry["bytes"], bool) or not _digest(entry["sha256"]):
                raise ValueError("package_entry_schema")
            name = entry["name"]
            _name(name)
            if name in expected or name not in archive.namelist():
                raise ValueError("package_entry_set_mismatch")
            expected.add(name)
            data = archive.read(name)
            if len(data) != entry["bytes"] or hashlib.sha256(data).hexdigest() != entry["sha256"]:
                raise ValueError("package_hash_mismatch")
            body = _json(data)
            _metadata(body)
            if not isinstance(body, dict) or set(body) - {
                "id", "kind", "name", "version", "review_status", "source_sha256",
                "source_task_id", "source_artifact_sha256", "model_sha256", "references",
                "origin_kind", "origin_schema", "units",
            }:
                raise ValueError("metadata_only_record_schema")
            if body.get("kind") not in {"project", "artifact", "model", "dataset", "report", "evidence"}:
                raise ValueError("metadata_only_record_schema")
            units = body.get("units", [])
            if not isinstance(units, list) or len(units) > 128 or any(not isinstance(field, dict) or set(field) != {"name", "unit"} or not isinstance(field["name"], str) or (field["unit"] is not None and not isinstance(field["unit"], str)) for field in units):
                raise ValueError("metadata_only_unit_schema")
            original_id = str(uuid.UUID(body["id"]))
            if original_id in ids:
                raise ValueError("duplicate_resource_id")
            ids.add(original_id)
            objects.append((original_id, body, entry["sha256"]))
        if expected != {info.filename for info in infos}:
            raise ValueError("package_entry_set_mismatch")
        dependencies = manifest.get("external_dependencies", [])
        edges = _lineage(objects, dependencies, manifest.get("edges", []))
        canonical = json.dumps(manifest, sort_keys=True, ensure_ascii=False, separators=(",", ":"), allow_nan=False).encode()
        return {"receipt_sha256": hashlib.sha256(canonical).hexdigest(), "source_project_id": source_id,
                "objects": objects, "missing_dependencies": dependencies, "edges": edges}


def write_metadata_package(destination, project_id, records, *, dependencies=(), edges=(), app_version="unknown"):
    """Independent exporter for explicitly supplied metadata, no asset discovery.

    The tests use only temporary paths. Production integration must use Tauri's
    existing authorized file writer; this prototype is not a desktop endpoint.
    """
    project_id = str(uuid.UUID(project_id))
    if not isinstance(records, list) or len(records) > 2047:
        raise ValueError("package_size_budget")
    entries = []
    bodies = []
    for ordinal, record in enumerate(records):
        _metadata(record)
        data = json.dumps(record, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
        entries.append({"name": f"records/{ordinal:04d}.json", "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
        bodies.append(data)
    manifest = {"schema_version": VERSION, "scope": "metadata_only", "source_project_id": project_id,
                "entries": entries, "external_dependencies": list(dependencies), "edges": list(edges), "app_version": app_version}
    _metadata(manifest)
    # Validate exact ZIP contents before creating the requested output. This
    # uses memory only, enforces the same budgets and never extracts files.
    import io
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", compression=zipfile.ZIP_STORED) as archive:
        archive.writestr("manifest.json", json.dumps(manifest, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False))
        for entry, data in zip(entries, bodies):
            archive.writestr(entry["name"], data)
    buffer.seek(0)
    checked = inspect(buffer)
    # Exclusive creation preserves an existing user file. No overwrite flag.
    with Path(destination).open("xb") as file:
        file.write(buffer.getvalue())
    return checked


def import_into_temporary_database(connection, package):
    """Caller supplies a prototype temporary DB, never a live Suna workspace.

    Only reference metadata is persisted. Source IDs/hashes are retained and
    new IDs are allocated separately; no historical payload is rewritten.
    """
    existing = connection.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall()
    if any(name not in ("package_receipts", "package_metadata", "package_edges") for (name,) in existing):
        raise ValueError("live_database_import_forbidden")
    receipt = package["receipt_sha256"]
    mapping = {original: str(uuid.uuid5(uuid.NAMESPACE_URL, "suna-research:" + receipt + ":" + original))
               for original, _, _ in package["objects"]}
    if connection.in_transaction:
        raise ValueError("temporary_import_requires_own_transaction")
    connection.execute("BEGIN IMMEDIATE")
    with connection:
        connection.execute("CREATE TABLE IF NOT EXISTS package_receipts(sha256 TEXT PRIMARY KEY, source_project_id TEXT, missing_json TEXT)")
        connection.execute("CREATE TABLE IF NOT EXISTS package_metadata(id TEXT PRIMARY KEY, original_id TEXT, receipt_sha256 TEXT, source_sha256 TEXT, metadata_json TEXT, status TEXT)")
        connection.execute("CREATE TABLE IF NOT EXISTS package_edges(receipt_sha256 TEXT, from_id TEXT, to_id TEXT, relation TEXT, PRIMARY KEY(receipt_sha256,from_id,to_id,relation))")
        old = connection.execute("SELECT source_project_id FROM package_receipts WHERE sha256=?", (receipt,)).fetchone()
        if old:
            if old[0] != package["source_project_id"]:
                raise ValueError("package_receipt_conflict")
            return mapping
        connection.execute("INSERT INTO package_receipts VALUES(?,?,?)", (receipt, package["source_project_id"], json.dumps(package["missing_dependencies"])))
        for original, body, source_sha in package["objects"]:
            connection.execute("INSERT INTO package_metadata VALUES(?,?,?,?,?,?)", (mapping[original], original, receipt, source_sha,
                json.dumps(body, ensure_ascii=False, sort_keys=True), "metadata_only_missing_assets"))
        for edge in package["edges"]:
            connection.execute("INSERT INTO package_edges VALUES(?,?,?,?)", (receipt, mapping[edge["from_id"]], mapping[edge["to_id"]], edge["relation"]))
    return mapping
