-- Keep an in-progress project's cover available until the project is saved or cancelled.
ALTER TABLE creation_staged_lorebook_projects
ADD COLUMN cover_asset_id TEXT GENERATED ALWAYS AS (
    CASE WHEN json_extract(project_json, '$.value.stage') = 'created'
         THEN json_extract(project_json, '$.value.cover_asset_id') END
) VIRTUAL REFERENCES media_assets(id) ON DELETE RESTRICT;

ALTER TABLE creation_staged_lorebook_runs
ADD COLUMN cover_asset_id TEXT GENERATED ALWAYS AS (
    CASE WHEN stage NOT IN ('committed', 'cancelled')
         THEN json_extract(run_json, '$.value.project.cover_asset_id') END
) VIRTUAL REFERENCES media_assets(id) ON DELETE RESTRICT;

CREATE INDEX creation_staged_lorebook_projects_cover_idx
    ON creation_staged_lorebook_projects(cover_asset_id) WHERE cover_asset_id IS NOT NULL;
CREATE INDEX creation_staged_lorebook_runs_cover_idx
    ON creation_staged_lorebook_runs(cover_asset_id) WHERE cover_asset_id IS NOT NULL;
