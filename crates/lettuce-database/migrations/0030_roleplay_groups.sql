-- Groups are roleplay-only. The stored mode columns stay for older formats.

UPDATE groups SET chat_mode = 'roleplay';

UPDATE conversation_settings SET chat_mode = NULL;

UPDATE conversations
SET kind_json = json_set(kind_json, '$.value.details.group.chat_mode', 'roleplay')
WHERE kind = 'group'
  AND json_extract(kind_json, '$.value.details.group.chat_mode') = 'conversation';
