CREATE TABLE creation_workflows (
    id TEXT PRIMARY KEY,
    target_json TEXT NOT NULL CHECK (json_valid(target_json)),
    stage TEXT NOT NULL CHECK (stage IN ('drafting', 'awaiting_review', 'awaiting_confirmation')),
    current_proposal_id TEXT,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (current_proposal_id) REFERENCES creation_proposals(id) ON DELETE RESTRICT,
    CHECK (updated_at >= created_at)
) STRICT;

CREATE TABLE creation_proposals (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    turn_id TEXT,
    parent_id TEXT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    stage TEXT NOT NULL CHECK (stage IN ('drafting', 'awaiting_review', 'awaiting_confirmation')),
    draft_json TEXT NOT NULL CHECK (json_valid(draft_json)),
    outcomes_json TEXT NOT NULL CHECK (json_valid(outcomes_json) AND json_type(outcomes_json) = 'array'),
    created_at INTEGER NOT NULL,
    UNIQUE (workflow_id, ordinal),
    FOREIGN KEY (workflow_id) REFERENCES creation_workflows(id) ON DELETE RESTRICT,
    FOREIGN KEY (turn_id) REFERENCES creation_turns(id) ON DELETE RESTRICT,
    FOREIGN KEY (parent_id) REFERENCES creation_proposals(id) ON DELETE RESTRICT,
    CHECK ((ordinal = 0) = (turn_id IS NULL)),
    CHECK ((ordinal = 0) = (parent_id IS NULL))
) STRICT;

CREATE TABLE creation_turns (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    base_proposal_id TEXT NOT NULL,
    user_message TEXT NOT NULL CHECK (length(trim(user_message)) > 0),
    created_at INTEGER NOT NULL,
    UNIQUE (workflow_id, ordinal),
    FOREIGN KEY (workflow_id) REFERENCES creation_workflows(id) ON DELETE RESTRICT,
    FOREIGN KEY (base_proposal_id) REFERENCES creation_proposals(id) ON DELETE RESTRICT
) STRICT;

CREATE INDEX creation_proposals_workflow_idx
    ON creation_proposals(workflow_id, ordinal, id);
CREATE INDEX creation_turns_workflow_idx
    ON creation_turns(workflow_id, ordinal, id);

CREATE TRIGGER creation_proposals_immutable_update
BEFORE UPDATE ON creation_proposals
BEGIN SELECT RAISE(ABORT, 'creation proposal is immutable'); END;

CREATE TRIGGER creation_proposals_immutable_delete
BEFORE DELETE ON creation_proposals
BEGIN SELECT RAISE(ABORT, 'creation proposal cannot be deleted'); END;

CREATE TRIGGER creation_turns_immutable_update
BEFORE UPDATE ON creation_turns
BEGIN SELECT RAISE(ABORT, 'creation turn is immutable'); END;

CREATE TRIGGER creation_turns_immutable_delete
BEFORE DELETE ON creation_turns
BEGIN SELECT RAISE(ABORT, 'creation turn cannot be deleted'); END;

CREATE TRIGGER creation_workflow_transition_guard
BEFORE UPDATE OF stage, current_proposal_id, revision ON creation_workflows
WHEN OLD.current_proposal_id IS NOT NULL AND (
  NEW.revision != OLD.revision + 1
  OR NEW.current_proposal_id = OLD.current_proposal_id
  OR (OLD.stage = 'awaiting_confirmation' AND NEW.stage != 'awaiting_confirmation')
)
BEGIN SELECT RAISE(ABORT, 'invalid creation workflow transition'); END;

CREATE TRIGGER creation_workflow_proposal_ownership
BEFORE UPDATE OF stage, current_proposal_id ON creation_workflows
WHEN NEW.current_proposal_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM creation_proposals
    WHERE id = NEW.current_proposal_id
      AND workflow_id = NEW.id
      AND stage = NEW.stage
)
BEGIN SELECT RAISE(ABORT, 'creation proposal ownership mismatch'); END;

CREATE TRIGGER creation_proposal_lineage_guard
BEFORE INSERT ON creation_proposals
WHEN (NEW.ordinal = 0 AND (NEW.stage != 'drafting' OR NEW.turn_id IS NOT NULL OR NEW.parent_id IS NOT NULL))
  OR (NEW.ordinal > 0 AND NOT EXISTS (
      SELECT 1
      FROM creation_proposals parent
      JOIN creation_turns turn ON turn.id = NEW.turn_id
      WHERE parent.id = NEW.parent_id
        AND parent.workflow_id = NEW.workflow_id
        AND parent.ordinal + 1 = NEW.ordinal
        AND turn.workflow_id = NEW.workflow_id
        AND turn.base_proposal_id = parent.id
  ))
BEGIN SELECT RAISE(ABORT, 'invalid creation proposal lineage'); END;
