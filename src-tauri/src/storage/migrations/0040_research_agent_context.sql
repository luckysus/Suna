CREATE TABLE research_agent_contexts (
 workspace_id TEXT NOT NULL,run_id TEXT NOT NULL,project_id TEXT,created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,run_id),FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id)
);
-- Legacy runs retain an explicitly unassigned context; never infer historical ownership.
INSERT INTO research_agent_contexts SELECT workspace_id,id,NULL,created_at FROM agent_runs;
CREATE TRIGGER capture_research_agent_context AFTER INSERT ON agent_runs BEGIN
 INSERT INTO research_agent_contexts SELECT NEW.workspace_id,NEW.id,p.id,NEW.created_at
 FROM (SELECT 1) LEFT JOIN research_projects p ON p.workspace_id=NEW.workspace_id AND p.id=(SELECT json_extract(value_json,'$') FROM settings WHERE workspace_id=NEW.workspace_id AND key='suna.research.current_project') AND p.status='active';
 INSERT OR IGNORE INTO research_project_links SELECT NEW.workspace_id,project_id,'agent_run',NEW.id,'generated_by',NEW.created_at,'agent_runtime' FROM research_agent_contexts WHERE workspace_id=NEW.workspace_id AND run_id=NEW.id AND project_id IS NOT NULL;
 INSERT OR IGNORE INTO research_project_links SELECT NEW.workspace_id,project_id,'conversation',NEW.conversation_id,'attached',NEW.created_at,'agent_runtime' FROM research_agent_contexts WHERE workspace_id=NEW.workspace_id AND run_id=NEW.id AND project_id IS NOT NULL;
END;
CREATE TRIGGER research_agent_context_immutable BEFORE UPDATE ON research_agent_contexts BEGIN SELECT RAISE(ABORT,'immutable_agent_project_context'); END;
