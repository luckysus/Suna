CREATE TABLE microstructure_images (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,project_id TEXT NOT NULL,sample_id TEXT NOT NULL,
 modality TEXT NOT NULL,source_type TEXT NOT NULL,metadata_json TEXT NOT NULL,
 source_sha256 TEXT NOT NULL,storage_ref TEXT NOT NULL,preview_ref TEXT NOT NULL,
 width INTEGER NOT NULL,height INTEGER NOT NULL,source_artifact_id TEXT NOT NULL,created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),FOREIGN KEY(workspace_id,project_id) REFERENCES research_projects(workspace_id,id),
 FOREIGN KEY(workspace_id,source_artifact_id) REFERENCES research_artifacts(workspace_id,id)
);
CREATE INDEX idx_microstructure_project ON microstructure_images(workspace_id,project_id,created_at DESC,id);
CREATE TABLE microstructure_annotations (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,image_id TEXT NOT NULL,parameters_json TEXT NOT NULL,
 content_sha256 TEXT NOT NULL,created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),FOREIGN KEY(workspace_id,image_id) REFERENCES microstructure_images(workspace_id,id)
);
CREATE INDEX idx_microstructure_annotations ON microstructure_annotations(workspace_id,image_id,created_at DESC,id);
CREATE TABLE microstructure_analyses (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,image_id TEXT NOT NULL,annotation_id TEXT NOT NULL,task_id TEXT NOT NULL,
 result_json TEXT NOT NULL,error_code TEXT,artifact_id TEXT,created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),UNIQUE(workspace_id,task_id,image_id),
 FOREIGN KEY(workspace_id,image_id) REFERENCES microstructure_images(workspace_id,id),
 FOREIGN KEY(workspace_id,annotation_id) REFERENCES microstructure_annotations(workspace_id,id),
 FOREIGN KEY(workspace_id,artifact_id) REFERENCES research_artifacts(workspace_id,id)
);
CREATE INDEX idx_microstructure_analyses ON microstructure_analyses(workspace_id,image_id,created_at DESC,id);
CREATE TABLE microstructure_reviews (
 workspace_id TEXT NOT NULL,id TEXT NOT NULL,analysis_id TEXT NOT NULL,state TEXT NOT NULL CHECK(state IN ('accepted','rejected')),
 note TEXT NOT NULL,created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id),UNIQUE(workspace_id,analysis_id),
 FOREIGN KEY(workspace_id,analysis_id) REFERENCES microstructure_analyses(workspace_id,id)
);
CREATE TRIGGER microstructure_image_immutable BEFORE UPDATE ON microstructure_images BEGIN SELECT RAISE(ABORT,'immutable_microstructure_image'); END;
CREATE TRIGGER microstructure_annotation_immutable BEFORE UPDATE ON microstructure_annotations BEGIN SELECT RAISE(ABORT,'immutable_microstructure_annotation'); END;
CREATE TRIGGER microstructure_analysis_immutable BEFORE UPDATE ON microstructure_analyses BEGIN SELECT RAISE(ABORT,'immutable_microstructure_analysis'); END;
CREATE TRIGGER microstructure_review_immutable BEFORE UPDATE ON microstructure_reviews BEGIN SELECT RAISE(ABORT,'immutable_microstructure_review'); END;
CREATE TRIGGER microstructure_image_no_delete BEFORE DELETE ON microstructure_images BEGIN SELECT RAISE(ABORT,'immutable_microstructure_image'); END;
CREATE TRIGGER microstructure_annotation_no_delete BEFORE DELETE ON microstructure_annotations BEGIN SELECT RAISE(ABORT,'immutable_microstructure_annotation'); END;
CREATE TRIGGER microstructure_analysis_no_delete BEFORE DELETE ON microstructure_analyses BEGIN SELECT RAISE(ABORT,'immutable_microstructure_analysis'); END;
CREATE TRIGGER microstructure_review_no_delete BEFORE DELETE ON microstructure_reviews BEGIN SELECT RAISE(ABORT,'immutable_microstructure_review'); END;
