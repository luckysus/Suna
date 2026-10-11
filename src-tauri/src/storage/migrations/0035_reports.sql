-- 第 80 章数据模型：科研报告（Report）。
-- 本迁移只新增表与索引，不修改、不清空任何既有数据。

CREATE TABLE reports (
  workspace_id TEXT NOT NULL,
  id TEXT NOT NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  topic TEXT NOT NULL DEFAULT '',
  format TEXT NOT NULL CHECK (format IN ('markdown', 'docx', 'pptx', 'pdf')),
  content_markdown TEXT NOT NULL DEFAULT '',
  dataset_id TEXT,
  knowledge_base_ids_json TEXT NOT NULL DEFAULT '[]',
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, id)
);

CREATE INDEX idx_reports_workspace_created
  ON reports (workspace_id, created_at DESC);
