CREATE TABLE usage_events (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    attempt_id TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('succeeded', 'failed', 'cancelled', 'interrupted')),
    counters_kind TEXT NOT NULL CHECK (counters_kind IN ('known', 'unavailable')),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    unavailable_reason TEXT CHECK (
        unavailable_reason IS NULL OR unavailable_reason IN (
            'not_admitted', 'cancelled_before_response', 'provider_omitted', 'transport_failed'
        )
    ),
    model_profile_id TEXT,
    model_revision INTEGER CHECK (model_revision IS NULL OR model_revision >= 1),
    provider_account_id TEXT,
    provider_account_revision INTEGER CHECK (
        provider_account_revision IS NULL OR provider_account_revision >= 1
    ),
    recorded_at INTEGER NOT NULL,
    UNIQUE (conversation_id, turn_id, attempt_id),
    FOREIGN KEY (conversation_id, turn_id, attempt_id)
        REFERENCES generation_attempts(conversation_id, turn_id, id) ON DELETE RESTRICT,
    CHECK ((counters_kind = 'known') = (input_tokens IS NOT NULL)),
    CHECK ((counters_kind = 'known') = (output_tokens IS NOT NULL)),
    CHECK ((counters_kind = 'unavailable') = (unavailable_reason IS NOT NULL)),
    CHECK ((model_profile_id IS NULL) = (model_revision IS NULL)),
    CHECK ((provider_account_id IS NULL) = (provider_account_revision IS NULL)),
    CHECK (counters_kind != 'known' OR (
        model_profile_id IS NOT NULL AND provider_account_id IS NOT NULL
    ))
) STRICT;

CREATE INDEX usage_events_recorded_at_idx ON usage_events(recorded_at, id);
CREATE TABLE usage_costs (
    event_id TEXT PRIMARY KEY REFERENCES usage_events(id) ON DELETE RESTRICT,
    basis_json TEXT NOT NULL CHECK (json_valid(basis_json))
) STRICT;

CREATE TRIGGER usage_costs_immutable_update BEFORE UPDATE ON usage_costs
BEGIN SELECT RAISE(ABORT, 'usage cost is immutable'); END;
CREATE TRIGGER usage_costs_immutable_delete BEFORE DELETE ON usage_costs
BEGIN SELECT RAISE(ABORT, 'usage cost cannot be deleted'); END;
CREATE INDEX usage_events_model_idx ON usage_events(model_profile_id, recorded_at, id);
CREATE INDEX usage_events_provider_idx ON usage_events(provider_account_id, recorded_at, id);

CREATE TRIGGER usage_events_immutable_update
BEFORE UPDATE ON usage_events
BEGIN SELECT RAISE(ABORT, 'usage event is immutable'); END;

CREATE TRIGGER usage_events_immutable_delete
BEFORE DELETE ON usage_events
BEGIN SELECT RAISE(ABORT, 'usage event cannot be deleted'); END;
