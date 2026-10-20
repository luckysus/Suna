ALTER TABLE provider_profiles ADD COLUMN source TEXT NOT NULL DEFAULT 'remote';
ALTER TABLE provider_profiles ADD COLUMN status TEXT NOT NULL DEFAULT 'active';
ALTER TABLE provider_profiles ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';
