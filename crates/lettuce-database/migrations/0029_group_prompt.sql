-- A group has one prompt: the roleplay group prompt.

UPDATE prompt_documents
SET purpose = 'group_chat_roleplay'
WHERE purpose = 'group_chat_conversational' AND built_in_key IS NULL;

UPDATE conversation_settings
SET prompt_json = replace(prompt_json, '"group_chat_conversational"', '"group_chat_roleplay"')
WHERE prompt_json LIKE '%"group_chat_conversational"%';

UPDATE conversation_settings
SET roleplay_prompt_json = replace(roleplay_prompt_json, '"group_chat_conversational"', '"group_chat_roleplay"')
WHERE roleplay_prompt_json LIKE '%"group_chat_conversational"%';

UPDATE prompt_documents
SET derived_source_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
WHERE derived_source_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

UPDATE app_settings
SET default_prompt_document_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
WHERE default_prompt_document_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

UPDATE conversation_starters
SET prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
WHERE prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

UPDATE characters
SET direct_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
WHERE direct_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

UPDATE characters
SET group_conversation_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
WHERE group_conversation_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

UPDATE characters
SET group_roleplay_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
WHERE group_roleplay_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

UPDATE groups
SET group_conversation_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
WHERE group_conversation_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

UPDATE groups
SET group_roleplay_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
WHERE group_roleplay_prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

UPDATE conversation_settings
SET prompt_json = replace(
    prompt_json,
    (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat'),
    (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
)
WHERE instr(prompt_json, (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat')) > 0;

UPDATE conversation_settings
SET roleplay_prompt_json = replace(
    roleplay_prompt_json,
    (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat'),
    (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat_roleplay')
)
WHERE instr(roleplay_prompt_json, (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat')) > 0;

DELETE FROM prompt_entries
WHERE prompt_id = (SELECT id FROM prompt_documents WHERE built_in_key = 'prompt_app_group_chat');

DELETE FROM prompt_documents
WHERE built_in_key = 'prompt_app_group_chat';

UPDATE characters
SET group_roleplay_prompt_id = COALESCE(group_roleplay_prompt_id, group_conversation_prompt_id),
    group_conversation_prompt_id = NULL,
    defaults_json = json_remove(defaults_json, '$.value.group_conversation_prompt_id')
WHERE group_conversation_prompt_id IS NOT NULL;

UPDATE characters
SET defaults_json = json_remove(defaults_json, '$.value.group_conversation_prompt_id')
WHERE json_extract(defaults_json, '$.value.group_conversation_prompt_id') IS NOT NULL;

UPDATE groups
SET group_roleplay_prompt_id = COALESCE(group_roleplay_prompt_id, group_conversation_prompt_id),
    group_conversation_prompt_id = NULL
WHERE group_conversation_prompt_id IS NOT NULL;

UPDATE conversations
SET revision = revision + 1
WHERE id IN (
    SELECT conversation_id
    FROM conversation_settings
    WHERE roleplay_prompt_json IS NOT NULL
       OR roleplay_prompt_provenance <> 'launch_inherited'
       OR (
           conversation_id IN (SELECT id FROM conversations WHERE kind = 'group')
           AND (prompt_json IS NOT NULL OR prompt_provenance <> 'launch_inherited')
       )
);

UPDATE conversation_settings
SET roleplay_prompt_json = NULL,
    roleplay_prompt_provenance = 'launch_inherited'
WHERE roleplay_prompt_json IS NOT NULL
   OR roleplay_prompt_provenance <> 'launch_inherited';

UPDATE conversation_settings
SET prompt_json = NULL,
    prompt_provenance = 'launch_inherited'
WHERE conversation_id IN (SELECT id FROM conversations WHERE kind = 'group')
  AND (prompt_json IS NOT NULL OR prompt_provenance <> 'launch_inherited');
