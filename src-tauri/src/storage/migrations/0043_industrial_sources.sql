CREATE TABLE material_source_snapshots(workspace_id TEXT NOT NULL,id TEXT NOT NULL,project_id TEXT NOT NULL,dataset_id TEXT NOT NULL,source_kind TEXT NOT NULL,metadata_json TEXT NOT NULL,content_sha256 TEXT NOT NULL,created_at TEXT NOT NULL,PRIMARY KEY(workspace_id,id),FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id),FOREIGN KEY(workspace_id,dataset_id) REFERENCES steel_datasets(workspace_id,id));
CREATE INDEX idx_material_sources ON material_source_snapshots(workspace_id,project_id,dataset_id);
CREATE TRIGGER material_source_immutable BEFORE UPDATE ON material_source_snapshots BEGIN SELECT RAISE(ABORT,'immutable_material_source'); END;
CREATE TRIGGER material_source_no_delete BEFORE DELETE ON material_source_snapshots BEGIN SELECT RAISE(ABORT,'immutable_material_source'); END;
CREATE TABLE database_read_profiles(workspace_id TEXT NOT NULL,id TEXT NOT NULL,connection_id TEXT NOT NULL,profile_json TEXT NOT NULL,created_at TEXT NOT NULL,PRIMARY KEY(workspace_id,id));
CREATE TRIGGER database_profile_immutable BEFORE UPDATE ON database_read_profiles BEGIN SELECT RAISE(ABORT,'immutable_database_read_profile'); END;
