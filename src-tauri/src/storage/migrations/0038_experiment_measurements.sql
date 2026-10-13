CREATE TABLE experiment_executions (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,project_id TEXT NOT NULL,plan_id TEXT NOT NULL,plan_artifact_id TEXT,
 sample_id TEXT NOT NULL,heat_id TEXT,batch_id TEXT,replicate INTEGER NOT NULL CHECK(replicate>0),
 actual_variables_json TEXT NOT NULL CHECK(json_valid(actual_variables_json)),variable_units_json TEXT NOT NULL CHECK(json_valid(variable_units_json)),performed_at TEXT NOT NULL,method TEXT NOT NULL,
 source_type TEXT NOT NULL CHECK(source_type IN ('measured','synthetic','literature')),note TEXT NOT NULL,
 created_at TEXT NOT NULL,PRIMARY KEY(workspace_id,id),UNIQUE(workspace_id,plan_id,replicate),
 FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id),
 FOREIGN KEY(workspace_id,plan_artifact_id) REFERENCES research_artifacts(workspace_id,id)
);
CREATE INDEX idx_executions_project ON experiment_executions(workspace_id,project_id,created_at DESC,id);
CREATE TABLE experiment_measurements (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,execution_id TEXT NOT NULL,property TEXT NOT NULL,value REAL NOT NULL,
 unit TEXT NOT NULL CHECK(length(unit)>0),method TEXT NOT NULL CHECK(length(method)>0),uncertainty REAL CHECK(uncertainty>=0),
 repeat_count INTEGER NOT NULL CHECK(repeat_count>0),measured_at TEXT NOT NULL,note TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),FOREIGN KEY(workspace_id,execution_id) REFERENCES experiment_executions(workspace_id,id),
 UNIQUE(workspace_id,execution_id,property)
);
CREATE TABLE experiment_reviews (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT,workspace_id TEXT NOT NULL,id TEXT NOT NULL,execution_id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('draft','submitted','accepted','rejected')),
 reviewer TEXT NOT NULL,reason TEXT NOT NULL,created_at TEXT NOT NULL,
 UNIQUE(workspace_id,id),FOREIGN KEY(workspace_id,execution_id) REFERENCES experiment_executions(workspace_id,id)
);
CREATE INDEX idx_experiment_reviews ON experiment_reviews(workspace_id,execution_id,sequence DESC);
CREATE TABLE research_dataset_versions (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,project_id TEXT NOT NULL,dataset_id TEXT NOT NULL,parent_dataset_id TEXT,
 measurement_ids_json TEXT NOT NULL CHECK(json_valid(measurement_ids_json)),created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),UNIQUE(workspace_id,dataset_id),
 FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id)
);
CREATE TRIGGER execution_immutable_update BEFORE UPDATE ON experiment_executions BEGIN SELECT RAISE(ABORT,'immutable_experiment_execution'); END;
CREATE TRIGGER execution_immutable_delete BEFORE DELETE ON experiment_executions BEGIN SELECT RAISE(ABORT,'immutable_experiment_execution'); END;
CREATE TRIGGER measurement_immutable_update BEFORE UPDATE ON experiment_measurements BEGIN SELECT RAISE(ABORT,'immutable_experiment_measurement'); END;
CREATE TRIGGER measurement_immutable_delete BEFORE DELETE ON experiment_measurements BEGIN SELECT RAISE(ABORT,'immutable_experiment_measurement'); END;
CREATE TRIGGER experiment_review_immutable_update BEFORE UPDATE ON experiment_reviews BEGIN SELECT RAISE(ABORT,'immutable_experiment_review'); END;
CREATE TRIGGER experiment_review_immutable_delete BEFORE DELETE ON experiment_reviews BEGIN SELECT RAISE(ABORT,'immutable_experiment_review'); END;
