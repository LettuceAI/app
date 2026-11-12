ALTER TABLE creation_generated_images ADD COLUMN job_id TEXT;
ALTER TABLE creation_generated_images ADD COLUMN failed INTEGER NOT NULL DEFAULT 0 CHECK (failed IN (0, 1));
