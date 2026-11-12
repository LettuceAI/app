UPDATE app_settings
SET payload_json = json_set(payload_json, '$.dynamic_memory.duplicate_threshold_basis_points', 9300)
WHERE json_extract(payload_json, '$.dynamic_memory.duplicate_threshold_basis_points') = 7800;

UPDATE app_settings
SET payload_json = json_set(payload_json, '$.group_dynamic_memory.duplicate_threshold_basis_points', 9300)
WHERE json_extract(payload_json, '$.group_dynamic_memory.duplicate_threshold_basis_points') = 7800;
