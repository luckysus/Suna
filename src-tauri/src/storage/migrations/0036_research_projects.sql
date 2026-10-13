-- 项目只引用现有对象；归档与解绑不删除原数据。
CREATE TABLE research_projects (
 workspace_id TEXT NOT NULL, id TEXT NOT NULL, name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 160),
 description TEXT NOT NULL DEFAULT '', material_system TEXT,
 status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','archived')),
 tags_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags_json)),
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id)
);
CREATE INDEX idx_research_projects_status ON research_projects(workspace_id,status,updated_at DESC,id);
CREATE TABLE research_project_links (
 workspace_id TEXT NOT NULL, project_id TEXT NOT NULL,
 target_kind TEXT NOT NULL CHECK(target_kind IN ('conversation','knowledge_base','document','dataset','model','background_task','agent_run','optimization','experiment','report','artifact')),
 target_id TEXT NOT NULL,
 relation TEXT NOT NULL CHECK(relation IN ('attached','generated_by','derived_from','referenced_by')),
 created_at TEXT NOT NULL, created_by TEXT NOT NULL,
 PRIMARY KEY(workspace_id,project_id,target_kind,target_id,relation),
 FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id)
);
CREATE INDEX idx_research_links_target ON research_project_links(workspace_id,target_kind,target_id);
CREATE TABLE research_audit_events (
 workspace_id TEXT NOT NULL, id TEXT NOT NULL, project_id TEXT NOT NULL,
 action TEXT NOT NULL, metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json)),
 actor TEXT NOT NULL, created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),
 FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id)
);
CREATE TRIGGER research_audit_immutable_update BEFORE UPDATE ON research_audit_events BEGIN SELECT RAISE(ABORT,'immutable_research_audit'); END;
CREATE TRIGGER research_audit_immutable_delete BEFORE DELETE ON research_audit_events BEGIN SELECT RAISE(ABORT,'immutable_research_audit'); END;
