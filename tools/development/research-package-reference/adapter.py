"""Allowlisted references from the existing research-manifest.v1 contract.

Caller supplies a manifest previously read through Suna's verified repository.
No SQL connection/file discovery here; it does not assert live asset restoration.
"""
import re
import uuid

from package import write_metadata_package


def _id(value):
    return str(uuid.UUID(value))


def _sha(value):
    return value if isinstance(value, str) and re.fullmatch("[a-f0-9]{64}", value) else None


def _label(value):
    if not isinstance(value, str) or not value.strip() or len(value) > 128 or any(ord(c) < 32 for c in value) or value.strip().startswith(("/", "\\")) or ":" in value:
        return "未公开名称"
    return value


def references_from_project_manifest(manifest):
    if manifest.get("schemaVersion") != "research-manifest.v1":
        raise ValueError("unsupported_project_manifest")
    project_id = _id(manifest["project"]["id"])
    objects, artifacts = manifest["objects"], manifest["artifacts"]
    if not isinstance(objects, list) or not isinstance(artifacts, list) or len(objects) > 10000 or len(artifacts) > 10000:
        raise ValueError("package_size_budget")
    records = {project_id: {"id": project_id, "kind": "project", "name": _label(manifest["project"].get("name"))}}
    dependencies = {}
    for obj in objects:
        identity = _id(obj["id"])
        kind = obj["kind"] if obj["kind"] in {"model", "dataset", "report"} else "artifact"
        if identity in records:
            if records[identity].get("origin_kind") != obj["kind"]:
                raise ValueError("project_object_identity_conflict")
            continue  # The same object may have several project link relations.
        records[identity] = {"id": identity, "kind": kind, "origin_kind": obj["kind"], "name": _label(obj.get("title"))}
        if obj["kind"] in {"model", "dataset", "document"}:
            dependencies[identity] = {"resource_id": identity, "kind": obj["kind"], "status": "missing"}
    for artifact in artifacts:
        payload = artifact.get("payload") or {}
        if not isinstance(payload, dict):
            raise ValueError("project_artifact_metadata_schema")
        record = {"id": _id(artifact["id"]), "kind": "artifact", "origin_kind": artifact["kind"],
                  "origin_schema": _label(artifact.get("schemaVersion")), "name": _label(artifact.get("title")),
                  "source_artifact_sha256": _sha(artifact.get("contentSha256"))}
        if record["source_artifact_sha256"] is None:
            raise ValueError("project_artifact_digest_missing")
        if artifact["kind"] == "dataset_snapshot":
            record["source_sha256"] = _sha(payload.get("source_sha256"))
            identity = payload.get("dataset_id")
            if identity in dependencies and record["source_sha256"]:
                dependencies[identity]["expected_sha256"] = record["source_sha256"]
            columns = payload.get("columns") or []
            record["units"] = [{"name": _label(column.get("canonical_field") or column.get("original_name")), "unit": _label(column["unit"]) if column.get("unit") is not None else None} for column in columns[:128]]
        if artifact["kind"] == "model_snapshot":
            record["model_sha256"] = _sha(payload.get("model_sha256"))
            version = payload.get("version")
            record["version"] = version if isinstance(version, int) and not isinstance(version, bool) and version > 0 else None
            identity = payload.get("model_id")
            if identity in dependencies and record["model_sha256"]:
                dependencies[identity]["expected_sha256"] = record["model_sha256"]
        existing = records.get(record["id"])
        if existing is not None and existing.get("origin_kind") != "artifact":
            raise ValueError("project_object_identity_conflict")
        # Real projects link each generated artifact in objects AND artifacts.
        # Upgrade that reference, preserving its one identity and original SHA.
        records[record["id"]] = record
    # Actual ResearchArtifact::Edge serializes from_id/to_id as fromId/toId.
    edges = [{"from_id": _id(edge["fromId"]), "to_id": _id(edge["toId"]), "relation": edge["relation"]} for edge in manifest.get("edges", [])]
    if len(records) > 2047:
        raise ValueError("package_size_budget")
    return {"project_id": project_id, "records": list(records.values()), "dependencies": list(dependencies.values()), "edges": edges}


def export_project_metadata(manifest, destination):
    projection = references_from_project_manifest(manifest)
    return write_metadata_package(destination, projection["project_id"], projection["records"],
                                  dependencies=projection["dependencies"], edges=projection["edges"], app_version=_label(manifest.get("appVersion")))
