-- M9 runtime contract.  M8 rows remain readable as legacy rows while every
-- row written with contract_version = 9 carries a typed, durable target and
-- provider-affecting request inputs.

ALTER TABLE conversation_turns ADD COLUMN target_kind TEXT
    CHECK (target_kind IS NULL OR target_kind IN ('new_assistant', 'existing_candidate'));
ALTER TABLE conversation_turns ADD COLUMN target_message_id TEXT;
ALTER TABLE conversation_turns ADD COLUMN target_parent_message_id TEXT;
ALTER TABLE conversation_turns ADD COLUMN target_prior_candidate_id TEXT;
ALTER TABLE conversation_turns ADD COLUMN retry_of_turn_id TEXT;
ALTER TABLE conversation_turns ADD COLUMN guidance TEXT
    CHECK (guidance IS NULL OR (length(trim(guidance)) > 0 AND length(CAST(guidance AS BLOB)) <= 262144));
ALTER TABLE conversation_turns ADD COLUMN requested_model_override_json TEXT
    CHECK (requested_model_override_json IS NULL OR
           (json_valid(requested_model_override_json) AND
            json_extract(requested_model_override_json, '$.format_version') = 1));
ALTER TABLE conversation_turns ADD COLUMN forced_speaker_participant_id TEXT;
ALTER TABLE conversation_turns ADD COLUMN swap_roles INTEGER NOT NULL DEFAULT 0
    CHECK (swap_roles IN (0, 1));
-- M8 rows are legacy and may have no target. New adapters must explicitly
-- opt into the M9-ready contract on INSERT.
ALTER TABLE conversation_turns ADD COLUMN contract_version INTEGER NOT NULL DEFAULT 8
    CHECK (contract_version IN (8, 9));

CREATE INDEX conversation_turns_id_idx ON conversation_turns(id);
CREATE INDEX conversation_message_revisions_id_idx ON conversation_message_revisions(id);
CREATE INDEX conversation_message_candidates_id_idx ON conversation_message_candidates(id);

-- Normalize rows written by M8 before enforcing the stricter pairing rules.
UPDATE conversation_settings
SET author_note = CASE WHEN author_note IS NULL OR length(trim(author_note)) = 0 THEN NULL ELSE author_note END,
    author_note_provenance = CASE WHEN author_note IS NULL THEN CASE WHEN author_note_provenance = 'launch_inherited' THEN 'launch_inherited' ELSE 'disabled' END WHEN length(trim(author_note)) = 0 THEN 'disabled' ELSE 'current_override' END,
    memory_provenance = CASE WHEN memory_json IS NULL THEN CASE WHEN memory_provenance = 'launch_inherited' THEN 'launch_inherited' ELSE 'disabled' END ELSE 'current_override' END,
    model_provenance = CASE WHEN model_override_json IS NULL THEN CASE WHEN model_provenance = 'launch_inherited' THEN 'launch_inherited' ELSE 'disabled' END ELSE 'current_override' END,
    voice_provenance = CASE WHEN voice_json IS NULL THEN CASE WHEN voice_provenance = 'launch_inherited' THEN 'launch_inherited' ELSE 'disabled' END ELSE 'current_override' END;

CREATE TRIGGER conversation_candidate_m9_target_insert
BEFORE INSERT ON conversation_message_candidates
WHEN EXISTS (
    SELECT 1 FROM conversation_turns AS turn
    WHERE turn.conversation_id = NEW.conversation_id
      AND turn.id = NEW.turn_id
      AND turn.contract_version = 9
      AND (
          NEW.message_id <> turn.target_message_id OR
          (turn.target_kind = 'new_assistant' AND NOT EXISTS (
              SELECT 1 FROM conversation_messages AS message
              WHERE message.conversation_id = NEW.conversation_id
                AND message.id = turn.target_message_id
                AND message.branch_id = NEW.branch_id
                AND message.role = 'assistant'
                AND message.parent_message_id = turn.target_parent_message_id
          ))
      )
)
BEGIN SELECT RAISE(ABORT, 'candidate does not match turn target'); END;

CREATE TRIGGER conversation_candidate_m9_target_update
BEFORE UPDATE OF conversation_id, message_id, branch_id, turn_id ON conversation_message_candidates
WHEN EXISTS (
    SELECT 1 FROM conversation_turns AS turn
    WHERE turn.conversation_id = NEW.conversation_id
      AND turn.id = NEW.turn_id
      AND turn.contract_version = 9
      AND (
          NEW.message_id <> turn.target_message_id OR
          (turn.target_kind = 'new_assistant' AND NOT EXISTS (
              SELECT 1 FROM conversation_messages AS message
              WHERE message.conversation_id = NEW.conversation_id
                AND message.id = turn.target_message_id
                AND message.branch_id = NEW.branch_id
                AND message.role = 'assistant'
                AND message.parent_message_id = turn.target_parent_message_id
          ))
      )
)
BEGIN SELECT RAISE(ABORT, 'candidate does not match turn target'); END;

CREATE TRIGGER conversation_settings_provenance_insert
BEFORE INSERT ON conversation_settings
WHEN (NEW.author_note_provenance = 'current_override') <> (NEW.author_note IS NOT NULL)
  OR (NEW.author_note_provenance <> 'current_override' AND NEW.author_note IS NOT NULL)
  OR (NEW.memory_provenance = 'current_override') <> (NEW.memory_json IS NOT NULL)
  OR (NEW.memory_provenance <> 'current_override' AND NEW.memory_json IS NOT NULL)
  OR (NEW.model_provenance = 'current_override') <> (NEW.model_override_json IS NOT NULL)
  OR (NEW.model_provenance <> 'current_override' AND NEW.model_override_json IS NOT NULL)
  OR (NEW.voice_provenance = 'current_override') <> (NEW.voice_json IS NOT NULL)
  OR (NEW.voice_provenance <> 'current_override' AND NEW.voice_json IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'settings provenance does not match value'); END;

CREATE TRIGGER conversation_settings_provenance_update
BEFORE UPDATE OF author_note, author_note_provenance, memory_json, memory_provenance,
    model_override_json, model_provenance, voice_json, voice_provenance
ON conversation_settings
WHEN (NEW.author_note_provenance = 'current_override') <> (NEW.author_note IS NOT NULL)
  OR (NEW.author_note_provenance <> 'current_override' AND NEW.author_note IS NOT NULL)
  OR (NEW.memory_provenance = 'current_override') <> (NEW.memory_json IS NOT NULL)
  OR (NEW.memory_provenance <> 'current_override' AND NEW.memory_json IS NOT NULL)
  OR (NEW.model_provenance = 'current_override') <> (NEW.model_override_json IS NOT NULL)
  OR (NEW.model_provenance <> 'current_override' AND NEW.model_override_json IS NOT NULL)
  OR (NEW.voice_provenance = 'current_override') <> (NEW.voice_json IS NOT NULL)
  OR (NEW.voice_provenance <> 'current_override' AND NEW.voice_json IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'settings provenance does not match value'); END;

CREATE TRIGGER conversation_settings_note_bounds_insert
BEFORE INSERT ON conversation_settings
WHEN NEW.author_note IS NOT NULL AND (length(trim(NEW.author_note)) = 0 OR length(CAST(NEW.author_note AS BLOB)) > 1048576)
BEGIN SELECT RAISE(ABORT, 'settings author note is blank or too large'); END;
CREATE TRIGGER conversation_settings_note_bounds_update
BEFORE UPDATE OF author_note ON conversation_settings
WHEN NEW.author_note IS NOT NULL AND (length(trim(NEW.author_note)) = 0 OR length(CAST(NEW.author_note AS BLOB)) > 1048576)
BEGIN SELECT RAISE(ABORT, 'settings author note is blank or too large'); END;

CREATE TRIGGER conversation_turn_m9_insert_contract
BEFORE INSERT ON conversation_turns
WHEN NEW.contract_version = 9 AND (
    NEW.target_kind IS NULL OR NEW.target_message_id IS NULL OR length(trim(NEW.target_message_id)) = 0 OR
    (NEW.target_parent_message_id IS NOT NULL AND length(trim(NEW.target_parent_message_id)) = 0) OR
    (NEW.target_prior_candidate_id IS NOT NULL AND length(trim(NEW.target_prior_candidate_id)) = 0) OR
    (NEW.target_kind = 'new_assistant' AND EXISTS (
        SELECT 1 FROM conversation_messages
        WHERE conversation_id = NEW.conversation_id AND id = NEW.target_message_id
    )) OR
    (NEW.target_kind = 'new_assistant' AND (
        NEW.target_parent_message_id IS NULL OR NEW.target_prior_candidate_id IS NOT NULL OR
        (NEW.operation = 'send' AND (
            NEW.input_kind <> 'user_message' OR NEW.user_message_id IS NULL OR
            NEW.target_message_id = NEW.user_message_id OR
            NEW.target_parent_message_id <> NEW.user_message_id)) OR
        (NEW.operation = 'continue' AND (
            NEW.input_kind <> 'existing_head' OR NEW.head_message_id IS NULL OR
            NEW.target_message_id = NEW.head_message_id OR
            NEW.target_parent_message_id <> NEW.head_message_id)) OR
        NEW.operation NOT IN ('send', 'continue')
    )) OR
    (NEW.target_kind = 'existing_candidate' AND (
        NEW.target_parent_message_id IS NOT NULL OR NEW.target_prior_candidate_id IS NULL OR
        NEW.operation <> 'regenerate' OR NEW.input_kind <> 'existing_candidate' OR
        NEW.candidate_message_id IS NULL OR NEW.candidate_id IS NULL OR
        NEW.target_message_id <> NEW.candidate_message_id OR
        NEW.target_prior_candidate_id <> NEW.candidate_id
    )) OR
    (NEW.retry_of_turn_id IS NOT NULL AND (
        NEW.retry_of_turn_id = NEW.id OR NOT EXISTS (
            SELECT 1 FROM conversation_turns AS source
            WHERE source.conversation_id = NEW.conversation_id
              AND source.id = NEW.retry_of_turn_id
              AND source.branch_id = NEW.branch_id
              AND source.status IN ('failed', 'cancelled')
              AND source.contract_version = 9
              AND source.operation = NEW.operation
              AND source.input_kind = NEW.input_kind
              AND coalesce(source.user_message_id, '') = coalesce(NEW.user_message_id, '')
              AND coalesce(source.head_message_id, '') = coalesce(NEW.head_message_id, '')
              AND coalesce(source.candidate_message_id, '') = coalesce(NEW.candidate_message_id, '')
              AND coalesce(source.candidate_id, '') = coalesce(NEW.candidate_id, '')
              AND source.target_kind = NEW.target_kind
              AND source.target_message_id = NEW.target_message_id
              AND coalesce(source.target_parent_message_id, '') = coalesce(NEW.target_parent_message_id, '')
              AND coalesce(source.target_prior_candidate_id, '') = coalesce(NEW.target_prior_candidate_id, '')
              AND coalesce(source.guidance, '') = coalesce(NEW.guidance, '')
              AND coalesce(source.requested_model_override_json, '') = coalesce(NEW.requested_model_override_json, '')
              AND coalesce(source.forced_speaker_participant_id, '') = coalesce(NEW.forced_speaker_participant_id, '')
              AND source.swap_roles = NEW.swap_roles
        )
    )) OR
    (NEW.forced_speaker_participant_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM conversations AS conversation
        JOIN conversation_participants AS participant
          ON participant.conversation_id = conversation.id
         AND participant.id = NEW.forced_speaker_participant_id
         AND participant.role = 'character'
        WHERE conversation.id = NEW.conversation_id
          AND conversation.kind = 'group'
    )) OR
    (NEW.swap_roles <> 0 AND EXISTS (
        SELECT 1 FROM conversations
        WHERE id = NEW.conversation_id AND kind = 'group'
    )) OR
    (NEW.operation = 'continue' AND (
        NEW.guidance IS NOT NULL OR NEW.requested_model_override_json IS NOT NULL
    )) OR
    (NEW.operation = 'regenerate' AND EXISTS (
        SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group'
    ) AND NEW.selected_speaker_participant_id IS NOT NULL AND (
        (NEW.forced_speaker_participant_id IS NULL AND NOT EXISTS (
            SELECT 1 FROM conversation_messages AS target
            WHERE target.conversation_id = NEW.conversation_id
              AND target.id = NEW.target_message_id
              AND target.author_participant_id = NEW.selected_speaker_participant_id
        )) OR
        (NEW.forced_speaker_participant_id IS NOT NULL AND
            NEW.selected_speaker_participant_id <> NEW.forced_speaker_participant_id)
    ))
)
BEGIN SELECT RAISE(ABORT, 'M9 generation turn contract violation'); END;

CREATE TRIGGER conversation_turn_m9_reject_legacy_insert
BEFORE INSERT ON conversation_turns
WHEN NEW.contract_version = 8
BEGIN SELECT RAISE(ABORT, 'new generation turns require M9 contract'); END;

CREATE TRIGGER conversation_turn_m9_update_contract
BEFORE UPDATE OF conversation_id, id, branch_id, operation, input_kind,
    user_message_id, head_message_id, candidate_message_id, candidate_id,
    target_kind, target_message_id, target_parent_message_id,
    target_prior_candidate_id, retry_of_turn_id, guidance,
    requested_model_override_json, forced_speaker_participant_id,
    swap_roles, contract_version ON conversation_turns
WHEN NEW.contract_version = 9 AND (
    NEW.target_kind IS NULL OR NEW.target_message_id IS NULL OR length(trim(NEW.target_message_id)) = 0 OR
    (NEW.target_parent_message_id IS NOT NULL AND length(trim(NEW.target_parent_message_id)) = 0) OR
    (NEW.target_prior_candidate_id IS NOT NULL AND length(trim(NEW.target_prior_candidate_id)) = 0) OR
    (NEW.target_kind = 'new_assistant' AND (
        NEW.target_parent_message_id IS NULL OR NEW.target_prior_candidate_id IS NOT NULL OR
        (NEW.operation = 'send' AND (
            NEW.input_kind <> 'user_message' OR NEW.user_message_id IS NULL OR
            NEW.target_message_id = NEW.user_message_id OR
            NEW.target_parent_message_id <> NEW.user_message_id)) OR
        (NEW.operation = 'continue' AND (
            NEW.input_kind <> 'existing_head' OR NEW.head_message_id IS NULL OR
            NEW.target_message_id = NEW.head_message_id OR
            NEW.target_parent_message_id <> NEW.head_message_id)) OR
        NEW.operation NOT IN ('send', 'continue')
    )) OR
    (NEW.target_kind = 'existing_candidate' AND (
        NEW.target_parent_message_id IS NOT NULL OR NEW.target_prior_candidate_id IS NULL OR
        NEW.operation <> 'regenerate' OR NEW.input_kind <> 'existing_candidate' OR
        NEW.candidate_message_id IS NULL OR NEW.candidate_id IS NULL OR
        NEW.target_message_id <> NEW.candidate_message_id OR
        NEW.target_prior_candidate_id <> NEW.candidate_id
    )) OR
    (NEW.retry_of_turn_id IS NOT NULL AND (
        NEW.retry_of_turn_id = NEW.id OR NOT EXISTS (
            SELECT 1 FROM conversation_turns AS source
            WHERE source.conversation_id = NEW.conversation_id
              AND source.id = NEW.retry_of_turn_id
              AND source.branch_id = NEW.branch_id
              AND source.status IN ('failed', 'cancelled')
              AND source.contract_version = 9
              AND source.operation = NEW.operation
              AND source.input_kind = NEW.input_kind
              AND coalesce(source.user_message_id, '') = coalesce(NEW.user_message_id, '')
              AND coalesce(source.head_message_id, '') = coalesce(NEW.head_message_id, '')
              AND coalesce(source.candidate_message_id, '') = coalesce(NEW.candidate_message_id, '')
              AND coalesce(source.candidate_id, '') = coalesce(NEW.candidate_id, '')
              AND source.target_kind = NEW.target_kind
              AND source.target_message_id = NEW.target_message_id
              AND coalesce(source.target_parent_message_id, '') = coalesce(NEW.target_parent_message_id, '')
              AND coalesce(source.target_prior_candidate_id, '') = coalesce(NEW.target_prior_candidate_id, '')
              AND coalesce(source.guidance, '') = coalesce(NEW.guidance, '')
              AND coalesce(source.requested_model_override_json, '') = coalesce(NEW.requested_model_override_json, '')
              AND coalesce(source.forced_speaker_participant_id, '') = coalesce(NEW.forced_speaker_participant_id, '')
              AND source.swap_roles = NEW.swap_roles
        )
    )) OR
    (NEW.forced_speaker_participant_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM conversations AS conversation
        JOIN conversation_participants AS participant
          ON participant.conversation_id = conversation.id
         AND participant.id = NEW.forced_speaker_participant_id
         AND participant.role = 'character'
        WHERE conversation.id = NEW.conversation_id
          AND conversation.kind = 'group'
    )) OR
    (NEW.swap_roles <> 0 AND EXISTS (
        SELECT 1 FROM conversations
        WHERE id = NEW.conversation_id AND kind = 'group'
    )) OR
    (NEW.operation = 'continue' AND (
        NEW.guidance IS NOT NULL OR NEW.requested_model_override_json IS NOT NULL
    )) OR
    (NEW.operation = 'regenerate' AND EXISTS (
        SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group'
    ) AND NEW.selected_speaker_participant_id IS NOT NULL AND (
        (NEW.forced_speaker_participant_id IS NULL AND NOT EXISTS (
            SELECT 1 FROM conversation_messages AS target
            WHERE target.conversation_id = NEW.conversation_id
              AND target.id = NEW.target_message_id
              AND target.author_participant_id = NEW.selected_speaker_participant_id
        )) OR
        (NEW.forced_speaker_participant_id IS NOT NULL AND
            NEW.selected_speaker_participant_id <> NEW.forced_speaker_participant_id)
    ))
)
BEGIN SELECT RAISE(ABORT, 'M9 generation turn contract violation'); END;

CREATE TRIGGER conversation_turn_m9_legacy_target_update
BEFORE UPDATE OF target_kind, target_message_id, target_parent_message_id,
    target_prior_candidate_id, retry_of_turn_id, guidance,
    requested_model_override_json, forced_speaker_participant_id, swap_roles ON conversation_turns
WHEN NEW.contract_version = 8 AND (
    coalesce(NEW.target_kind, '') <> coalesce(OLD.target_kind, '') OR
    coalesce(NEW.target_message_id, '') <> coalesce(OLD.target_message_id, '') OR
    coalesce(NEW.target_parent_message_id, '') <> coalesce(OLD.target_parent_message_id, '') OR
    coalesce(NEW.target_prior_candidate_id, '') <> coalesce(OLD.target_prior_candidate_id, '') OR
    coalesce(NEW.retry_of_turn_id, '') <> coalesce(OLD.retry_of_turn_id, '') OR
    coalesce(NEW.guidance, '') <> coalesce(OLD.guidance, '') OR
    coalesce(NEW.requested_model_override_json, '') <> coalesce(OLD.requested_model_override_json, '') OR
    coalesce(NEW.forced_speaker_participant_id, '') <> coalesce(OLD.forced_speaker_participant_id, '')
    OR NEW.swap_roles <> OLD.swap_roles
)
BEGIN SELECT RAISE(ABORT, 'legacy generation turn cannot set M9 contract fields'); END;

CREATE TRIGGER conversation_turn_m9_intent_immutable
BEFORE UPDATE OF conversation_id, id, branch_id, operation, input_kind,
    user_message_id, head_message_id, candidate_message_id, candidate_id,
    target_kind, target_message_id, target_parent_message_id,
    target_prior_candidate_id, retry_of_turn_id, guidance,
    requested_model_override_json, forced_speaker_participant_id, swap_roles
ON conversation_turns
WHEN OLD.contract_version = 9 AND (
    NEW.conversation_id <> OLD.conversation_id OR NEW.id <> OLD.id OR
    NEW.branch_id <> OLD.branch_id OR NEW.operation <> OLD.operation OR
    NEW.input_kind <> OLD.input_kind OR coalesce(NEW.user_message_id, '') <> coalesce(OLD.user_message_id, '') OR
    coalesce(NEW.head_message_id, '') <> coalesce(OLD.head_message_id, '') OR
    coalesce(NEW.candidate_message_id, '') <> coalesce(OLD.candidate_message_id, '') OR
    coalesce(NEW.candidate_id, '') <> coalesce(OLD.candidate_id, '') OR
    coalesce(NEW.target_kind, '') <> coalesce(OLD.target_kind, '') OR
    coalesce(NEW.target_message_id, '') <> coalesce(OLD.target_message_id, '') OR
    coalesce(NEW.target_parent_message_id, '') <> coalesce(OLD.target_parent_message_id, '') OR
    coalesce(NEW.target_prior_candidate_id, '') <> coalesce(OLD.target_prior_candidate_id, '') OR
    coalesce(NEW.retry_of_turn_id, '') <> coalesce(OLD.retry_of_turn_id, '') OR
    coalesce(NEW.guidance, '') <> coalesce(OLD.guidance, '') OR
    coalesce(NEW.requested_model_override_json, '') <> coalesce(OLD.requested_model_override_json, '') OR
    coalesce(NEW.forced_speaker_participant_id, '') <> coalesce(OLD.forced_speaker_participant_id, '') OR
    NEW.swap_roles <> OLD.swap_roles
)
BEGIN SELECT RAISE(ABORT, 'generation turn request intent is immutable'); END;

CREATE TRIGGER conversation_turn_m9_contract_version_immutable
BEFORE UPDATE OF contract_version ON conversation_turns
WHEN NEW.contract_version <> OLD.contract_version
BEGIN SELECT RAISE(ABORT, 'generation turn contract version is immutable'); END;

CREATE TRIGGER conversation_turn_retry_source_delete
BEFORE DELETE ON conversation_turns
WHEN EXISTS (
    SELECT 1 FROM conversation_turns AS child
    WHERE child.retry_of_turn_id = OLD.id
      AND child.conversation_id = OLD.conversation_id
)
BEGIN SELECT RAISE(ABORT, 'retry source is protected'); END;

CREATE TRIGGER conversation_turn_selected_speaker_character_m9_insert
BEFORE INSERT ON conversation_turns
WHEN NEW.selected_speaker_participant_id IS NOT NULL AND (
    NOT EXISTS (SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group') OR
    NOT EXISTS (
    SELECT 1 FROM conversation_participants
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.selected_speaker_participant_id
      AND role = 'character'
    )
)
BEGIN SELECT RAISE(ABORT, 'selected speaker must be a character'); END;

CREATE TRIGGER conversation_turn_selected_speaker_character_m9_update
BEFORE UPDATE OF conversation_id, selected_speaker_participant_id ON conversation_turns
WHEN NEW.selected_speaker_participant_id IS NOT NULL AND (
    NOT EXISTS (SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group') OR
    NOT EXISTS (
    SELECT 1 FROM conversation_participants
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.selected_speaker_participant_id
      AND role = 'character'
    )
)
BEGIN SELECT RAISE(ABORT, 'selected speaker must be a character'); END;

CREATE TRIGGER conversation_turn_regenerate_speaker_m9_insert
BEFORE INSERT ON conversation_turns
WHEN NEW.contract_version = 9
  AND NEW.operation = 'regenerate'
  AND EXISTS (SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group')
  AND NEW.selected_speaker_participant_id IS NOT NULL
  AND (
      (NEW.forced_speaker_participant_id IS NOT NULL
       AND NEW.selected_speaker_participant_id <> NEW.forced_speaker_participant_id)
      OR
      (NEW.forced_speaker_participant_id IS NULL AND NOT EXISTS (
          SELECT 1 FROM conversation_messages AS target
          WHERE target.conversation_id = NEW.conversation_id
            AND target.id = NEW.target_message_id
            AND target.author_participant_id = NEW.selected_speaker_participant_id
      ))
  )
BEGIN SELECT RAISE(ABORT, 'regenerate speaker does not match target author or force'); END;

CREATE TRIGGER conversation_turn_regenerate_speaker_m9_update
BEFORE UPDATE OF conversation_id, operation, target_message_id,
    forced_speaker_participant_id, selected_speaker_participant_id ON conversation_turns
WHEN NEW.contract_version = 9
  AND NEW.operation = 'regenerate'
  AND EXISTS (SELECT 1 FROM conversations WHERE id = NEW.conversation_id AND kind = 'group')
  AND NEW.selected_speaker_participant_id IS NOT NULL
  AND (
      (NEW.forced_speaker_participant_id IS NOT NULL
       AND NEW.selected_speaker_participant_id <> NEW.forced_speaker_participant_id)
      OR
      (NEW.forced_speaker_participant_id IS NULL AND NOT EXISTS (
          SELECT 1 FROM conversation_messages AS target
          WHERE target.conversation_id = NEW.conversation_id
            AND target.id = NEW.target_message_id
            AND target.author_participant_id = NEW.selected_speaker_participant_id
      ))
  )
BEGIN SELECT RAISE(ABORT, 'regenerate speaker does not match target author or force'); END;

CREATE TRIGGER conversation_turn_selected_speaker_immutable
BEFORE UPDATE OF selected_speaker_participant_id ON conversation_turns
WHEN OLD.selected_speaker_participant_id IS NOT NULL
 AND coalesce(NEW.selected_speaker_participant_id, '') <> OLD.selected_speaker_participant_id
BEGIN SELECT RAISE(ABORT, 'selected speaker is immutable once resolved'); END;

CREATE TRIGGER conversation_message_identity_immutable
BEFORE UPDATE OF conversation_id, id, branch_id, parent_message_id ON conversation_messages
WHEN NEW.conversation_id <> OLD.conversation_id
   OR NEW.id <> OLD.id
   OR NEW.branch_id <> OLD.branch_id
   OR coalesce(NEW.parent_message_id, '') <> coalesce(OLD.parent_message_id, '')
BEGIN SELECT RAISE(ABORT, 'message topology is immutable'); END;

CREATE TRIGGER conversation_branch_head_same_branch_insert
BEFORE INSERT ON conversation_branches
WHEN NEW.head_message_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_messages
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.head_message_id
      AND branch_id = NEW.id
)
BEGIN SELECT RAISE(ABORT, 'branch head must belong to branch'); END;
CREATE TRIGGER conversation_branch_head_same_branch_update
BEFORE UPDATE OF conversation_id, id, head_message_id ON conversation_branches
WHEN NEW.head_message_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_messages
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.head_message_id
      AND branch_id = NEW.id
)
BEGIN SELECT RAISE(ABORT, 'branch head must belong to branch'); END;

CREATE TRIGGER conversation_branch_fork_message_parent_insert
BEFORE INSERT ON conversation_branches
WHEN NEW.fork_message_id IS NOT NULL AND NEW.parent_branch_id <> NEW.id AND NOT EXISTS (
    SELECT 1 FROM conversation_messages
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.fork_message_id
      AND branch_id = NEW.parent_branch_id
)
BEGIN SELECT RAISE(ABORT, 'fork message must belong to parent branch'); END;
CREATE TRIGGER conversation_branch_fork_message_parent_update
BEFORE UPDATE OF conversation_id, id, parent_branch_id, fork_message_id ON conversation_branches
WHEN NEW.fork_message_id IS NOT NULL AND NEW.parent_branch_id <> NEW.id AND NOT EXISTS (
    SELECT 1 FROM conversation_messages
    WHERE conversation_id = NEW.conversation_id
      AND id = NEW.fork_message_id
      AND branch_id = NEW.parent_branch_id
)
BEGIN SELECT RAISE(ABORT, 'fork message must belong to parent branch'); END;

CREATE TRIGGER conversation_branch_parent_fork_immutable
BEFORE UPDATE OF parent_branch_id, fork_message_id ON conversation_branches
WHEN coalesce(NEW.parent_branch_id, '') <> coalesce(OLD.parent_branch_id, '')
   OR coalesce(NEW.fork_message_id, '') <> coalesce(OLD.fork_message_id, '')
BEGIN SELECT RAISE(ABORT, 'branch topology is immutable'); END;

CREATE TRIGGER conversation_message_parent_topology_insert
BEFORE INSERT ON conversation_messages
WHEN NEW.parent_message_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_messages AS parent
    WHERE parent.conversation_id = NEW.conversation_id
      AND parent.id = NEW.parent_message_id
      AND (
          parent.branch_id = NEW.branch_id OR
          EXISTS (
              SELECT 1 FROM conversation_branches AS branch
              WHERE branch.conversation_id = NEW.conversation_id
                AND branch.id = NEW.branch_id
                AND branch.parent_branch_id = parent.branch_id
                AND branch.fork_message_id = parent.id
          )
      )
)
BEGIN SELECT RAISE(ABORT, 'message parent violates branch topology'); END;
CREATE TRIGGER conversation_message_parent_topology_update
BEFORE UPDATE OF conversation_id, id, branch_id, parent_message_id ON conversation_messages
WHEN NEW.parent_message_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM conversation_messages AS parent
    WHERE parent.conversation_id = NEW.conversation_id
      AND parent.id = NEW.parent_message_id
      AND (
          parent.branch_id = NEW.branch_id OR
          EXISTS (
              SELECT 1 FROM conversation_branches AS branch
              WHERE branch.conversation_id = NEW.conversation_id
                AND branch.id = NEW.branch_id
                AND branch.parent_branch_id = parent.branch_id
                AND branch.fork_message_id = parent.id
          )
      )
)
BEGIN SELECT RAISE(ABORT, 'message parent violates branch topology'); END;
