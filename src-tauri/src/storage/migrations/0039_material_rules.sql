CREATE TABLE material_schema_overlays (
 workspace_id TEXT NOT NULL,project_id TEXT NOT NULL,version INTEGER NOT NULL,fields_json TEXT NOT NULL CHECK(json_valid(fields_json)),
 overrides_json TEXT NOT NULL CHECK(json_valid(overrides_json)),created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,project_id,version),FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id)
);
CREATE TABLE material_rule_versions (
 workspace_id TEXT NOT NULL,project_id TEXT NOT NULL,id TEXT NOT NULL,version INTEGER NOT NULL,body_json TEXT NOT NULL CHECK(json_valid(body_json)),created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,project_id,id,version),FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id)
);
CREATE INDEX idx_material_rules_project ON material_rule_versions(workspace_id,project_id,id,version DESC);
CREATE TABLE material_mapping_versions (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,project_id TEXT NOT NULL,source_dataset_id TEXT NOT NULL,output_dataset_id TEXT NOT NULL,
 schema_version TEXT NOT NULL,mapping_json TEXT NOT NULL CHECK(json_valid(mapping_json)),created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id)
);
CREATE TRIGGER material_rule_immutable_update BEFORE UPDATE ON material_rule_versions BEGIN SELECT RAISE(ABORT,'immutable_material_rule'); END;
CREATE TRIGGER material_rule_immutable_delete BEFORE DELETE ON material_rule_versions BEGIN SELECT RAISE(ABORT,'immutable_material_rule'); END;
CREATE TRIGGER material_schema_immutable_update BEFORE UPDATE ON material_schema_overlays BEGIN SELECT RAISE(ABORT,'immutable_material_schema'); END;
CREATE TRIGGER material_schema_immutable_delete BEFORE DELETE ON material_schema_overlays BEGIN SELECT RAISE(ABORT,'immutable_material_schema'); END;
CREATE TRIGGER material_mapping_immutable_update BEFORE UPDATE ON material_mapping_versions BEGIN SELECT RAISE(ABORT,'immutable_material_mapping'); END;
CREATE TRIGGER material_mapping_immutable_delete BEFORE DELETE ON material_mapping_versions BEGIN SELECT RAISE(ABORT,'immutable_material_mapping'); END;
