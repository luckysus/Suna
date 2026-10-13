CREATE TABLE research_runs (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,project_id TEXT NOT NULL,title TEXT NOT NULL,kind TEXT NOT NULL,
 background_task_id TEXT,created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),UNIQUE(workspace_id,project_id,background_task_id),
 FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id)
);
CREATE INDEX idx_research_runs_project ON research_runs(workspace_id,project_id,created_at DESC,id);
CREATE TABLE research_artifacts (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,project_id TEXT NOT NULL,kind TEXT NOT NULL,title TEXT NOT NULL,
 schema_version TEXT NOT NULL,immutable_payload_json TEXT NOT NULL CHECK(json_valid(immutable_payload_json)),
 content_sha256 TEXT NOT NULL,source_key TEXT NOT NULL,source_run_id TEXT,
 state TEXT NOT NULL DEFAULT 'ready' CHECK(state IN ('ready','invalidated','unavailable')),
 created_at TEXT NOT NULL,created_by TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),UNIQUE(workspace_id,project_id,kind,source_key),
 FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id),
 FOREIGN KEY(workspace_id,source_run_id) REFERENCES research_runs(workspace_id,id)
);
CREATE INDEX idx_research_artifacts_project ON research_artifacts(workspace_id,project_id,kind,created_at DESC,id);
CREATE TRIGGER research_artifact_immutable BEFORE UPDATE ON research_artifacts WHEN
 NEW.id<>OLD.id OR NEW.workspace_id<>OLD.workspace_id OR NEW.project_id<>OLD.project_id OR NEW.kind<>OLD.kind OR NEW.title<>OLD.title OR NEW.schema_version<>OLD.schema_version OR NEW.immutable_payload_json<>OLD.immutable_payload_json OR NEW.content_sha256<>OLD.content_sha256 OR NEW.source_key<>OLD.source_key OR NEW.source_run_id IS NOT OLD.source_run_id OR NEW.created_at<>OLD.created_at OR NEW.created_by<>OLD.created_by
 BEGIN SELECT RAISE(ABORT,'immutable_research_artifact'); END;
CREATE TRIGGER research_artifact_no_delete BEFORE DELETE ON research_artifacts BEGIN SELECT RAISE(ABORT,'immutable_research_artifact'); END;
CREATE TABLE research_artifact_edges (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,from_artifact_id TEXT NOT NULL,to_artifact_id TEXT NOT NULL,
 relation TEXT NOT NULL CHECK(relation IN ('uses_dataset','uses_model','generated_by_run','based_on_evidence','recommends_experiment','measured_against','supersedes','included_in_report')),
 created_at TEXT NOT NULL,PRIMARY KEY(workspace_id,id),UNIQUE(workspace_id,from_artifact_id,to_artifact_id,relation),
 CHECK(from_artifact_id<>to_artifact_id),
 FOREIGN KEY(workspace_id,from_artifact_id) REFERENCES research_artifacts(workspace_id,id),
 FOREIGN KEY(workspace_id,to_artifact_id) REFERENCES research_artifacts(workspace_id,id)
);
CREATE INDEX idx_research_edges_from ON research_artifact_edges(workspace_id,from_artifact_id);
CREATE INDEX idx_research_edges_to ON research_artifact_edges(workspace_id,to_artifact_id);
CREATE TRIGGER research_edge_immutable BEFORE UPDATE ON research_artifact_edges BEGIN SELECT RAISE(ABORT,'immutable_research_edge'); END;
CREATE TABLE research_run_sources (
 workspace_id TEXT NOT NULL,run_id TEXT NOT NULL,artifact_id TEXT NOT NULL,relation TEXT NOT NULL,
 PRIMARY KEY(workspace_id,run_id,artifact_id,relation),
 FOREIGN KEY(workspace_id,run_id) REFERENCES research_runs(workspace_id,id),
 FOREIGN KEY(workspace_id,artifact_id) REFERENCES research_artifacts(workspace_id,id)
);
