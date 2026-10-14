-- Template history only; task states and model versions keep their existing stores.
CREATE TABLE workflow_templates (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision>0 AND revision<=256),
 name TEXT NOT NULL,schema_version TEXT NOT NULL,
 payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
 content_sha256 TEXT NOT NULL CHECK(length(content_sha256)=64),
 created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id,revision)
);
CREATE INDEX idx_workflow_template_versions ON workflow_templates(workspace_id,created_at DESC,id,revision DESC);
CREATE TRIGGER workflow_template_immutable BEFORE UPDATE ON workflow_templates
 BEGIN SELECT RAISE(ABORT,'immutable_workflow_template'); END;
CREATE TRIGGER workflow_template_no_delete BEFORE DELETE ON workflow_templates
 BEGIN SELECT RAISE(ABORT,'immutable_workflow_template'); END;
