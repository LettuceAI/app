UPDATE characters
SET defaults_json = json_set(defaults_json, '$.value.voice', json('null')),
    voice_profile_id = NULL,
    revision = revision + 1
WHERE voice_profile_id IS NOT NULL
  AND voice_profile_id NOT IN (SELECT id FROM user_voices);
