CREATE TABLE IF NOT EXISTS agent_feedback (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  workspace_id TEXT NOT NULL,
  conversation_id TEXT NOT NULL DEFAULT '',
  message_id TEXT NOT NULL DEFAULT '',
  rating TEXT NOT NULL CHECK (rating IN ('up', 'down')),
  reason TEXT,
  content TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_feedback_message
  ON agent_feedback (workspace_id, conversation_id, message_id);

CREATE INDEX IF NOT EXISTS idx_agent_feedback_conversation
  ON agent_feedback (workspace_id, conversation_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_agent_feedback_rating
  ON agent_feedback (workspace_id, rating, created_at DESC);
