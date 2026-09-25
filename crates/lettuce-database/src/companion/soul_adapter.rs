use std::str::FromStr;

use lettuce_companions::{
    SoulApplyReceipt, SoulCategory, SoulChangeSet, SoulFact, SoulFactKind, SoulFactPolicy,
    SoulOwner, SoulPolicyError, SoulRepository, SoulRepositoryError, SoulState, apply_change_set,
    validate_state,
};
use lettuce_types::{CharacterId, OperationRecordId, Revision, TimestampMillis};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};

use crate::Database;

fn failure(_: impl std::fmt::Debug) -> SoulRepositoryError {
    SoulRepositoryError::Failure
}

fn corrupt(_: impl std::fmt::Debug) -> SoulRepositoryError {
    SoulRepositoryError::Corrupt
}

fn sql_revision(value: Revision) -> Result<i64, SoulRepositoryError> {
    i64::try_from(value.get()).map_err(corrupt)
}

fn parse_revision(value: i64) -> Result<Revision, SoulRepositoryError> {
    let value = u64::try_from(value).map_err(corrupt)?;
    if value == 0 {
        return Err(SoulRepositoryError::Corrupt);
    }
    Ok(Revision::new(value))
}

fn category_name(value: SoulCategory) -> &'static str {
    value.as_str()
}

fn parse_category(value: &str) -> Result<SoulCategory, SoulRepositoryError> {
    match value {
        "essence" => Ok(SoulCategory::Essence),
        "traits" => Ok(SoulCategory::Traits),
        "backstory" => Ok(SoulCategory::Backstory),
        "appearance" => Ok(SoulCategory::Appearance),
        "goals" => Ok(SoulCategory::Goals),
        "likes" => Ok(SoulCategory::Likes),
        "voice" => Ok(SoulCategory::Voice),
        "relationalStyle" => Ok(SoulCategory::RelationalStyle),
        "vulnerabilities" => Ok(SoulCategory::Vulnerabilities),
        "fears" => Ok(SoulCategory::Fears),
        "habits" => Ok(SoulCategory::Habits),
        "boundaries" => Ok(SoulCategory::Boundaries),
        _ => Err(SoulRepositoryError::Corrupt),
    }
}

fn kind_name(value: SoulFactKind) -> &'static str {
    match value {
        SoulFactKind::Add => "add",
        SoulFactKind::Adjust => "adjust",
        SoulFactKind::Authored => "authored",
        SoulFactKind::Consolidated => "consolidated",
    }
}

fn parse_kind(value: &str) -> Result<SoulFactKind, SoulRepositoryError> {
    match value {
        "add" => Ok(SoulFactKind::Add),
        "adjust" => Ok(SoulFactKind::Adjust),
        "authored" => Ok(SoulFactKind::Authored),
        "consolidated" => Ok(SoulFactKind::Consolidated),
        _ => Err(SoulRepositoryError::Corrupt),
    }
}

fn policy_name(value: SoulFactPolicy) -> &'static str {
    match value {
        SoulFactPolicy::Current => "current",
        SoulFactPolicy::Adaptive => "adaptive",
        SoulFactPolicy::Historical => "historical",
    }
}

fn parse_policy(value: &str) -> Result<SoulFactPolicy, SoulRepositoryError> {
    match value {
        "current" => Ok(SoulFactPolicy::Current),
        "adaptive" => Ok(SoulFactPolicy::Adaptive),
        "historical" => Ok(SoulFactPolicy::Historical),
        _ => Err(SoulRepositoryError::Corrupt),
    }
}

/// The `scope` column of an owner's rows: empty for the character's shared
/// Soul, else the conversation id.
pub(crate) fn scope(owner: SoulOwner) -> String {
    owner
        .conversation_id()
        .map(|id| id.to_string())
        .unwrap_or_default()
}

pub(crate) fn owner_of(
    character_id: CharacterId,
    scope: &str,
) -> Result<SoulOwner, SoulRepositoryError> {
    Ok(if scope.is_empty() {
        SoulOwner::Character(character_id)
    } else {
        SoulOwner::Conversation {
            character_id,
            conversation_id: scope.parse().map_err(corrupt)?,
        }
    })
}

fn load_strings(
    tx: &Transaction<'_>,
    table: &str,
    column: &str,
    owner: SoulOwner,
    fact_id: &str,
) -> Result<Vec<String>, SoulRepositoryError> {
    let sql = format!(
        "SELECT {column} FROM {table} WHERE character_id = ?1 AND scope = ?2 AND fact_id = ?3 ORDER BY ordinal"
    );
    let mut statement = tx.prepare(&sql).map_err(corrupt)?;
    statement
        .query_map(
            params![owner.character_id().to_string(), scope(owner), fact_id],
            |row| row.get(0),
        )
        .map_err(corrupt)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(corrupt)
}

pub(crate) fn get_in(
    tx: &Transaction<'_>,
    owner: SoulOwner,
) -> Result<Option<SoulState>, SoulRepositoryError> {
    let character_id = owner.character_id();
    let revision = tx
        .query_row(
            "SELECT revision FROM companion_soul_states WHERE character_id = ?1 AND scope = ?2",
            params![character_id.to_string(), scope(owner)],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(corrupt)?;
    let Some(revision) = revision else {
        return Ok(None);
    };
    let facts = {
        let mut statement = tx
            .prepare(
                "SELECT id, category, value, kind, policy, slot, confidence, evidence_count,
                        weight, valid_from, valid_until, locked, created_at, superseded_by,
                        superseded_at
                   FROM companion_soul_facts
                  WHERE character_id = ?1 AND scope = ?2
                  ORDER BY ordinal",
            )
            .map_err(corrupt)?;
        let rows = statement
            .query_map(params![character_id.to_string(), scope(owner)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, f64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, f64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                    row.get::<_, bool>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, Option<i64>>(14)?,
                ))
            })
            .map_err(corrupt)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(corrupt)?;
        let mut facts = Vec::with_capacity(rows.len());
        for row in rows {
            let evidence_count = u32::try_from(row.7).map_err(corrupt)?;
            facts.push(SoulFact {
                id: row.0.clone(),
                category: parse_category(&row.1)?,
                value: row.2,
                kind: parse_kind(&row.3)?,
                policy: parse_policy(&row.4)?,
                slot: row.5,
                confidence: row.6,
                evidence_count,
                weight: row.8,
                valid_from: TimestampMillis::new(row.9),
                valid_until: row.10.map(TimestampMillis::new),
                locked: row.11,
                source_memory_ids: load_strings(
                    tx,
                    "companion_soul_fact_sources",
                    "memory_id",
                    owner,
                    &row.0,
                )?,
                created_at: TimestampMillis::new(row.12),
                supersedes: load_strings(
                    tx,
                    "companion_soul_fact_supersedes",
                    "superseded_fact_id",
                    owner,
                    &row.0,
                )?,
                superseded_by: row.13,
                superseded_at: row.14.map(TimestampMillis::new),
            });
        }
        facts
    };
    let state = SoulState {
        revision: parse_revision(revision)?,
        facts,
    };
    validate_state(&state).map_err(|_| SoulRepositoryError::Corrupt)?;
    Ok(Some(state))
}

pub(crate) fn insert_facts(
    tx: &Transaction<'_>,
    owner: SoulOwner,
    facts: &[SoulFact],
) -> Result<(), SoulRepositoryError> {
    let character_id = owner.character_id();
    let scope = scope(owner);
    for (ordinal, fact) in facts.iter().enumerate() {
        tx.execute(
            "INSERT INTO companion_soul_facts (
                character_id, id, ordinal, category, value, kind, policy, slot, confidence,
                evidence_count, weight, valid_from, valid_until, locked, created_at,
                superseded_by, superseded_at, scope
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                character_id.to_string(),
                fact.id,
                i64::try_from(ordinal).map_err(corrupt)?,
                category_name(fact.category),
                fact.value,
                kind_name(fact.kind),
                policy_name(fact.policy),
                fact.slot,
                fact.confidence,
                i64::from(fact.evidence_count),
                fact.weight,
                fact.valid_from.get(),
                fact.valid_until.map(TimestampMillis::get),
                fact.locked,
                fact.created_at.get(),
                fact.superseded_by,
                fact.superseded_at.map(TimestampMillis::get),
                scope,
            ],
        )
        .map_err(failure)?;
        for (source_ordinal, memory_id) in fact.source_memory_ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO companion_soul_fact_sources (character_id, fact_id, ordinal, memory_id, scope)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![character_id.to_string(), fact.id, i64::try_from(source_ordinal).map_err(corrupt)?, memory_id, scope],
            )
            .map_err(failure)?;
        }
        for (supersedes_ordinal, superseded_id) in fact.supersedes.iter().enumerate() {
            tx.execute(
                "INSERT INTO companion_soul_fact_supersedes
                    (character_id, fact_id, ordinal, superseded_fact_id, scope)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    character_id.to_string(),
                    fact.id,
                    i64::try_from(supersedes_ordinal).map_err(corrupt)?,
                    superseded_id,
                    scope
                ],
            )
            .map_err(failure)?;
        }
    }
    Ok(())
}

/// Replaces an owner's Soul facts with an imported or synced snapshot,
/// creating its Soul state when it has none yet.
pub(crate) fn replace_facts_in(
    tx: &Transaction<'_>,
    owner: SoulOwner,
    facts: &[SoulFact],
    now: TimestampMillis,
) -> Result<(), SoulRepositoryError> {
    let character_id = owner.character_id();
    validate_state(&SoulState {
        revision: Revision::INITIAL,
        facts: facts.to_vec(),
    })
    .map_err(SoulRepositoryError::Invalid)?;
    tx.execute(
        "INSERT OR IGNORE INTO companion_soul_states (character_id, scope, revision, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?3)",
        params![character_id.to_string(), scope(owner), now.get()],
    )
    .map_err(failure)?;
    tx.execute(
        "DELETE FROM companion_soul_facts WHERE character_id = ?1 AND scope = ?2",
        params![character_id.to_string(), scope(owner)],
    )
    .map_err(failure)?;
    insert_facts(tx, owner, facts)
}

/// Writes `facts` as an owner's Soul as one new revision, creating the Soul
/// when the owner has none.
fn overwrite_in(
    tx: &Transaction<'_>,
    owner: SoulOwner,
    facts: &[SoulFact],
    now: TimestampMillis,
) -> Result<(), SoulRepositoryError> {
    let existed = get_in(tx, owner)?.is_some();
    replace_facts_in(tx, owner, facts, now)?;
    if existed {
        tx.execute(
            "UPDATE companion_soul_states SET revision = revision + 1, updated_at = max(updated_at, ?3)
              WHERE character_id = ?1 AND scope = ?2",
            params![owner.character_id().to_string(), scope(owner), now.get()],
        )
        .map_err(failure)?;
    }
    Ok(())
}

/// Seeds a companion conversation that has no Soul of its own with a copy of
/// its character's shared Soul; one that has kept its own Soul resumes it
/// (user decisions 2026-09-23).
pub(crate) fn seed_conversation_soul_in(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    conversation_id: lettuce_types::ConversationId,
    now: TimestampMillis,
) -> Result<(), SoulRepositoryError> {
    let owner = SoulOwner::Conversation {
        character_id,
        conversation_id,
    };
    if get_in(tx, owner)?.is_some() {
        return Ok(());
    }
    let Some(shared) = get_in(tx, SoulOwner::Character(character_id))? else {
        return Ok(());
    };
    replace_facts_in(tx, owner, &shared.facts, now)
}

/// Follows a change of the character's share-Soul-growth toggle (user
/// decisions 2026-09-23): turned off, each companion conversation without a
/// Soul of its own starts from a copy of the shared Soul, and the others
/// resume theirs; turned on, the most recently updated Soul of a conversation
/// that still exists becomes the shared one when it is newer and differs. The
/// conversation Souls are kept. A synced toggle change only seeds: the Souls
/// the other device promoted arrive by sync themselves.
pub(crate) fn apply_sharing_change_in(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    was_shared: bool,
    shared: bool,
    synced: bool,
    now: TimestampMillis,
) -> Result<(), SoulRepositoryError> {
    match (was_shared, shared) {
        (true, false) => {
            let conversations = tx
                .prepare(
                    "SELECT conversation_id FROM companion_session_states
                      WHERE character_id = ?1 ORDER BY conversation_id",
                )
                .map_err(failure)?
                .query_map([character_id.to_string()], |row| row.get::<_, String>(0))
                .map_err(failure)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(failure)?;
            for conversation_id in conversations {
                seed_conversation_soul_in(
                    tx,
                    character_id,
                    conversation_id.parse().map_err(corrupt)?,
                    now,
                )?;
            }
        }
        (false, true) if !synced => {
            let latest = tx
                .query_row(
                    "SELECT soul.scope FROM companion_soul_states soul
                       JOIN conversations conversation ON conversation.id = soul.scope
                      WHERE soul.character_id = ?1 AND conversation.lifecycle <> 'tombstoned'
                        AND soul.updated_at > coalesce(
                            (SELECT updated_at FROM companion_soul_states
                              WHERE character_id = ?1 AND scope = ''),
                            -9223372036854775808)
                      ORDER BY soul.updated_at DESC, soul.scope DESC LIMIT 1",
                    [character_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(failure)?;
            if let Some(scope) = latest {
                let facts = get_in(tx, owner_of(character_id, &scope)?)?
                    .ok_or(SoulRepositoryError::Corrupt)?
                    .facts;
                if get_in(tx, SoulOwner::Character(character_id))?
                    .is_none_or(|current| current.facts != facts)
                {
                    overwrite_in(tx, SoulOwner::Character(character_id), &facts, now)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Gives a companion character its shared Soul when it has none, seeded from
/// its current settings (legacy `default_state`). With `reconcile_authored`,
/// an existing Soul also gains every authored fact of the settings whose id it
/// lacks, as legacy merged a new session's authored facts into the stored
/// fact pool; facts it already holds keep their stored form.
pub(crate) fn ensure_character_soul_in(
    tx: &Transaction<'_>,
    character_id: CharacterId,
    config: Option<&lettuce_companions::CompanionSoulConfig>,
    reconcile_authored: bool,
    now: TimestampMillis,
) -> Result<(), SoulRepositoryError> {
    let owner = SoulOwner::Character(character_id);
    let Some(current) = get_in(tx, owner)? else {
        let state = lettuce_companions::initial_soul_state(config, now)
            .map_err(SoulRepositoryError::Invalid)?;
        return create_in(tx, owner, &state, now);
    };
    let Some(config) = config.filter(|_| reconcile_authored) else {
        return Ok(());
    };
    let known = current
        .facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let missing = lettuce_companions::CompanionSoulConfig {
        authored_facts: config
            .authored_facts
            .iter()
            .filter(|fact| !fact.id.trim().is_empty() && !known.contains(fact.id.as_str()))
            .cloned()
            .collect(),
        ..config.clone()
    };
    if missing.authored_facts.is_empty() {
        return Ok(());
    }
    let added = lettuce_companions::initial_soul_state(Some(&missing), now)
        .map_err(SoulRepositoryError::Invalid)?
        .facts;
    let mut facts = current.facts;
    facts.extend(added);
    overwrite_in(tx, owner, &facts, now)
}

/// Gives a duplicated companion its own copy of the source character's shared
/// Soul, or a fresh one from its settings when the source has none.
pub(crate) fn copy_character_soul_in(
    tx: &Transaction<'_>,
    source: CharacterId,
    destination: CharacterId,
    config: Option<&lettuce_companions::CompanionSoulConfig>,
    now: TimestampMillis,
) -> Result<(), SoulRepositoryError> {
    match get_in(tx, SoulOwner::Character(source))? {
        Some(state) => replace_facts_in(tx, SoulOwner::Character(destination), &state.facts, now),
        None => ensure_character_soul_in(tx, destination, config, false, now),
    }
}

pub(crate) fn create_in(
    tx: &Transaction<'_>,
    owner: SoulOwner,
    state: &SoulState,
    now: TimestampMillis,
) -> Result<(), SoulRepositoryError> {
    validate_state(state).map_err(SoulRepositoryError::Invalid)?;
    if state.revision != Revision::INITIAL {
        return Err(SoulRepositoryError::Invalid(SoulPolicyError::InvalidFact));
    }
    let character_id = owner.character_id();
    let inserted = tx
        .execute(
            "INSERT OR IGNORE INTO companion_soul_states (character_id, scope, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![character_id.to_string(), scope(owner), sql_revision(state.revision)?, now.get()],
        )
        .map_err(failure)?;
    if inserted != 1 {
        let created_at = tx
            .query_row(
                "SELECT created_at FROM companion_soul_states WHERE character_id = ?1 AND scope = ?2",
                params![character_id.to_string(), scope(owner)],
                |row| row.get::<_, i64>(0),
            )
            .map_err(corrupt)?;
        if created_at == now.get() && get_in(tx, owner)?.as_ref() == Some(state) {
            return Ok(());
        }
        return Err(SoulRepositoryError::AlreadyExists);
    }
    insert_facts(tx, owner, &state.facts)
}

fn put_hash_part(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

fn change_hash(change: &SoulChangeSet) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    put_hash_part(&mut hasher, b"lettuce-companion-soul-change-v1");
    hasher.update(&change.expected_revision.get().to_le_bytes());
    hasher.update(&change.resulting_revision.get().to_le_bytes());
    hasher.update(&change.applied_at.get().to_le_bytes());
    hasher.update(&(change.additions.len() as u64).to_le_bytes());
    for fact in &change.additions {
        put_hash_part(&mut hasher, fact.id.as_bytes());
        put_hash_part(&mut hasher, category_name(fact.category).as_bytes());
        put_hash_part(&mut hasher, fact.value.as_bytes());
        put_hash_part(&mut hasher, kind_name(fact.kind).as_bytes());
        put_hash_part(&mut hasher, policy_name(fact.policy).as_bytes());
        put_hash_part(&mut hasher, fact.slot.as_bytes());
        hasher.update(&fact.confidence.to_bits().to_le_bytes());
        hasher.update(&fact.evidence_count.to_le_bytes());
        hasher.update(&fact.weight.to_bits().to_le_bytes());
        hasher.update(&fact.valid_from.get().to_le_bytes());
        hasher.update(&[u8::from(fact.valid_until.is_some())]);
        if let Some(value) = fact.valid_until {
            hasher.update(&value.get().to_le_bytes());
        }
        hasher.update(&[u8::from(fact.locked)]);
        hasher.update(&(fact.source_memory_ids.len() as u64).to_le_bytes());
        for id in &fact.source_memory_ids {
            put_hash_part(&mut hasher, id.as_bytes());
        }
        hasher.update(&fact.created_at.get().to_le_bytes());
        hasher.update(&(fact.supersedes.len() as u64).to_le_bytes());
        for id in &fact.supersedes {
            put_hash_part(&mut hasher, id.as_bytes());
        }
        hasher.update(&[u8::from(fact.superseded_by.is_some())]);
        if let Some(value) = &fact.superseded_by {
            put_hash_part(&mut hasher, value.as_bytes());
        }
        hasher.update(&[u8::from(fact.superseded_at.is_some())]);
        if let Some(value) = fact.superseded_at {
            hasher.update(&value.get().to_le_bytes());
        }
    }
    hasher.update(&(change.supersessions.len() as u64).to_le_bytes());
    for item in &change.supersessions {
        put_hash_part(&mut hasher, item.fact_id.as_bytes());
        put_hash_part(&mut hasher, item.superseded_by.as_bytes());
    }
    if !change.user_edits.is_empty() {
        hasher.update(&(change.user_edits.len() as u64).to_le_bytes());
        for edit in &change.user_edits {
            match edit {
                lettuce_companions::SoulUserEdit::ClearAll => put_hash_part(&mut hasher, b"clear"),
                lettuce_companions::SoulUserEdit::Remove { fact_id } => {
                    put_hash_part(&mut hasher, b"remove");
                    put_hash_part(&mut hasher, fact_id.as_bytes());
                }
                lettuce_companions::SoulUserEdit::SetLocked { fact_id, locked } => {
                    put_hash_part(&mut hasher, b"lock");
                    put_hash_part(&mut hasher, fact_id.as_bytes());
                    hasher.update(&[u8::from(*locked)]);
                }
            }
        }
    }
    *hasher.finalize().as_bytes()
}

fn load_receipt(
    tx: &Transaction<'_>,
    operation_id: OperationRecordId,
) -> Result<Option<(SoulApplyReceipt, Vec<u8>)>, SoulRepositoryError> {
    tx.query_row(
        "SELECT character_id, expected_revision, resulting_revision, applied_at, change_hash, scope
           FROM companion_soul_apply_receipts WHERE operation_id = ?1",
        [operation_id.to_string()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, String>(5)?,
            ))
        },
    )
    .optional()
    .map_err(corrupt)?
    .map(|row| {
        let character_id = CharacterId::from_str(&row.0).map_err(corrupt)?;
        let receipt = SoulApplyReceipt {
            operation_id,
            owner: owner_of(character_id, &row.5)?,
            expected_revision: parse_revision(row.1)?,
            resulting_revision: parse_revision(row.2)?,
            applied_at: TimestampMillis::new(row.3),
        };
        if receipt.resulting_revision != receipt.expected_revision.next().map_err(corrupt)?
            || row.4.len() != 32
        {
            return Err(SoulRepositoryError::Corrupt);
        }
        Ok((receipt, row.4))
    })
    .transpose()
}

impl SoulRepository for Database {
    fn create(
        &self,
        owner: SoulOwner,
        state: SoulState,
        now: TimestampMillis,
    ) -> Result<SoulState, SoulRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        create_in(&tx, owner, &state, now)?;
        tx.commit().map_err(failure)?;
        Ok(state)
    }

    fn get(&self, owner: SoulOwner) -> Result<Option<SoulState>, SoulRepositoryError> {
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(failure)?;
        let state = get_in(&tx, owner)?;
        tx.commit().map_err(failure)?;
        Ok(state)
    }

    fn apply(
        &self,
        owner: SoulOwner,
        operation_id: OperationRecordId,
        change_set: SoulChangeSet,
    ) -> Result<SoulApplyReceipt, SoulRepositoryError> {
        let hash = change_hash(&change_set);
        let mut connection = self.connection().map_err(failure)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(failure)?;
        if let Some((receipt, stored_hash)) = load_receipt(&tx, operation_id)? {
            if receipt.owner == owner
                && receipt.expected_revision == change_set.expected_revision
                && receipt.resulting_revision == change_set.resulting_revision
                && receipt.applied_at == change_set.applied_at
                && stored_hash.as_slice() == hash
            {
                tx.commit().map_err(failure)?;
                return Ok(receipt);
            }
            return Err(SoulRepositoryError::OperationMismatch);
        }
        let current = get_in(&tx, owner)?.ok_or(SoulRepositoryError::NotFound)?;
        let next = apply_change_set(&current, &change_set).map_err(|error| match error {
            SoulPolicyError::StaleRevision => SoulRepositoryError::Conflict,
            other => SoulRepositoryError::Invalid(other),
        })?;
        let character_id = owner.character_id();
        tx.execute(
            "DELETE FROM companion_soul_facts WHERE character_id = ?1 AND scope = ?2",
            params![character_id.to_string(), scope(owner)],
        )
        .map_err(failure)?;
        insert_facts(&tx, owner, &next.facts)?;
        let updated = tx.execute(
            "UPDATE companion_soul_states SET revision = ?2, updated_at = max(updated_at, ?3) WHERE character_id = ?1 AND revision = ?4 AND scope = ?5",
            params![character_id.to_string(), sql_revision(next.revision)?, change_set.recorded_at.get(), sql_revision(change_set.expected_revision)?, scope(owner)],
        ).map_err(failure)?;
        if updated != 1 {
            return Err(SoulRepositoryError::Conflict);
        }
        tx.execute(
            "INSERT INTO companion_soul_apply_receipts (operation_id, character_id, expected_revision, resulting_revision, applied_at, change_hash, scope) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![operation_id.to_string(), character_id.to_string(), sql_revision(change_set.expected_revision)?, sql_revision(change_set.resulting_revision)?, change_set.applied_at.get(), hash.as_slice(), scope(owner)],
        ).map_err(failure)?;
        let receipt = SoulApplyReceipt {
            operation_id,
            owner,
            expected_revision: change_set.expected_revision,
            resulting_revision: change_set.resulting_revision,
            applied_at: change_set.applied_at,
        };
        tx.commit().map_err(failure)?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use lettuce_companions::{
        ProposedSoulFact, SoulCategory, SoulFact, SoulFactKind, SoulFactPolicy, SoulOwner,
        SoulRepository, SoulRepositoryError, SoulState, prepare_growth_change_set,
    };
    use lettuce_types::{CharacterId, OperationRecordId, Revision, TimestampMillis};
    use rusqlite::params;

    use super::Database;

    fn owner(id: CharacterId) -> SoulOwner {
        SoulOwner::Character(id)
    }

    fn insert_character(database: &Database, id: CharacterId, name: &str) {
        let connection = database.connection().expect("connection");
        connection
            .execute(
                "INSERT INTO characters (
                id, status, name, nickname, normalized_name, normalized_nickname,
                profile_json, provenance_json, defaults_json, interaction_mode, memory_policy,
                model_profile_id, default_scene_id, default_starter_id, direct_prompt_id,
                group_conversation_prompt_id, group_roleplay_prompt_id, voice_profile_id,
                voice_legacy_locator, voice_autoplay, presentation_json, image_recommendation_json,
                revision, created_at, updated_at
             ) VALUES (
                ?1, 'active', ?2, NULL, ?3, NULL, '{}', '{}', '{}', 'companion', 'dynamic',
                NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 0, '{}', NULL, 1, 1, 1
             )",
                params![id.to_string(), name, name.to_lowercase()],
            )
            .expect("character");
    }

    fn proposed(id: &str) -> ProposedSoulFact {
        ProposedSoulFact {
            id: id.to_owned(),
            category: SoulCategory::Likes,
            value: format!("value-{id}"),
            kind: SoulFactKind::Add,
            policy: SoulFactPolicy::Adaptive,
            slot: "food".to_owned(),
            confidence: 0.75,
            weight: 0.8,
            valid_until: Some(TimestampMillis::new(100)),
            locked: false,
            source_memory_ids: vec!["memory-a".to_owned(), "memory-b".to_owned()],
            supersedes: Vec::new(),
        }
    }

    fn empty() -> SoulState {
        SoulState {
            revision: Revision::INITIAL,
            facts: Vec::new(),
        }
    }

    #[test]
    fn creates_and_round_trips_character_owned_typed_soul_state() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        insert_character(&database, character_id, "one");
        assert_eq!(database.get(owner(character_id)).expect("missing"), None);
        database
            .create(owner(character_id), empty(), TimestampMillis::new(1))
            .expect("create");
        assert_eq!(
            database.create(owner(character_id), empty(), TimestampMillis::new(1)),
            Ok(empty())
        );
        assert_eq!(
            database.create(owner(character_id), empty(), TimestampMillis::new(2)),
            Err(SoulRepositoryError::AlreadyExists)
        );
        let initial = database
            .get(owner(character_id))
            .expect("get")
            .expect("state");
        let change = prepare_growth_change_set(
            &initial,
            initial.revision,
            vec![proposed("fact-a")],
            TimestampMillis::new(2),
        )
        .expect("change");
        database
            .apply(owner(character_id), OperationRecordId::new(), change)
            .expect("apply");
        let stored = database
            .get(owner(character_id))
            .expect("get")
            .expect("state");
        assert_eq!(stored.revision, Revision::new(2));
        assert_eq!(stored.facts[0].source_memory_ids, ["memory-a", "memory-b"]);
        assert_eq!(stored.facts[0].valid_until, Some(TimestampMillis::new(100)));
    }

    #[test]
    fn exact_retry_returns_immutable_receipt_and_mismatches_fail_closed() {
        let database = Database::open_in_memory().expect("database");
        let first_id = CharacterId::new();
        let second_id = CharacterId::new();
        insert_character(&database, first_id, "first");
        insert_character(&database, second_id, "second");
        database
            .create(owner(first_id), empty(), TimestampMillis::new(1))
            .expect("first");
        database
            .create(owner(second_id), empty(), TimestampMillis::new(1))
            .expect("second");
        let change = prepare_growth_change_set(
            &empty(),
            Revision::INITIAL,
            vec![proposed("one")],
            TimestampMillis::new(2),
        )
        .expect("change");
        let operation_id = OperationRecordId::new();
        let receipt = database
            .apply(owner(first_id), operation_id, change.clone())
            .expect("apply");
        assert_eq!(
            database.apply(owner(first_id), operation_id, change.clone()),
            Ok(receipt)
        );
        assert_eq!(
            database.apply(owner(second_id), operation_id, change.clone()),
            Err(SoulRepositoryError::OperationMismatch)
        );
        let mut altered = change;
        altered.additions[0].value.push_str(" altered");
        assert_eq!(
            database.apply(owner(first_id), operation_id, altered),
            Err(SoulRepositoryError::OperationMismatch)
        );
        let connection = database.connection().expect("connection");
        assert!(connection.execute("UPDATE companion_soul_apply_receipts SET applied_at = 9 WHERE operation_id = ?1", [operation_id.to_string()]).is_err());
    }

    #[test]
    fn stale_apply_and_failed_fact_insert_roll_back_atomically() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        insert_character(&database, character_id, "rollback");
        database
            .create(owner(character_id), empty(), TimestampMillis::new(1))
            .expect("create");
        let first = prepare_growth_change_set(
            &empty(),
            Revision::INITIAL,
            vec![proposed("same")],
            TimestampMillis::new(2),
        )
        .expect("first");
        database
            .apply(owner(character_id), OperationRecordId::new(), first)
            .expect("apply");
        let stale = prepare_growth_change_set(
            &empty(),
            Revision::INITIAL,
            vec![proposed("stale")],
            TimestampMillis::new(3),
        )
        .expect("stale");
        assert_eq!(
            database.apply(owner(character_id), OperationRecordId::new(), stale),
            Err(SoulRepositoryError::Conflict)
        );
        let before = database
            .get(owner(character_id))
            .expect("get")
            .expect("state");
        let mut malformed = prepare_growth_change_set(
            &before,
            before.revision,
            vec![proposed("new")],
            TimestampMillis::new(4),
        )
        .expect("malformed");
        malformed.additions[0].id = "same".to_owned();
        assert!(matches!(
            database.apply(owner(character_id), OperationRecordId::new(), malformed),
            Err(SoulRepositoryError::Invalid(_))
        ));
        assert_eq!(
            database.get(owner(character_id)).expect("get"),
            Some(before)
        );
    }

    #[test]
    fn two_database_handles_enforce_revision_cas() {
        let path =
            std::env::temp_dir().join(format!("lettuce-soul-cas-{}.db", OperationRecordId::new()));
        let first = Database::open(&path).expect("first database");
        let second = Database::open(&path).expect("second database");
        let character_id = CharacterId::new();
        insert_character(&first, character_id, "cas");
        first
            .create(owner(character_id), empty(), TimestampMillis::new(1))
            .expect("create");
        let one = prepare_growth_change_set(
            &empty(),
            Revision::INITIAL,
            vec![proposed("one")],
            TimestampMillis::new(2),
        )
        .expect("one");
        let two = prepare_growth_change_set(
            &empty(),
            Revision::INITIAL,
            vec![proposed("two")],
            TimestampMillis::new(2),
        )
        .expect("two");
        first
            .apply(owner(character_id), OperationRecordId::new(), one)
            .expect("first apply");
        assert_eq!(
            second.apply(owner(character_id), OperationRecordId::new(), two),
            Err(SoulRepositoryError::Conflict)
        );
        drop(first);
        drop(second);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn apply_keeps_only_forty_superseded_facts() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        insert_character(&database, character_id, "history");
        let facts = (0..41)
            .map(|index| SoulFact {
                id: format!("old-{index}"),
                category: SoulCategory::Likes,
                value: format!("old {index}"),
                kind: SoulFactKind::Add,
                policy: SoulFactPolicy::Adaptive,
                slot: "history".to_owned(),
                confidence: 1.0,
                evidence_count: 0,
                weight: 1.0,
                valid_from: TimestampMillis::new(1),
                valid_until: None,
                locked: false,
                source_memory_ids: Vec::new(),
                created_at: TimestampMillis::new(1),
                supersedes: Vec::new(),
                superseded_by: Some("later".to_owned()),
                superseded_at: Some(TimestampMillis::new(2)),
            })
            .collect();
        let state = SoulState {
            revision: Revision::INITIAL,
            facts,
        };
        database
            .create(owner(character_id), state.clone(), TimestampMillis::new(1))
            .expect("create");
        let change = lettuce_companions::SoulChangeSet {
            expected_revision: Revision::INITIAL,
            resulting_revision: Revision::new(2),
            additions: Vec::new(),
            supersessions: Vec::new(),
            user_edits: Vec::new(),
            applied_at: TimestampMillis::new(3),
            recorded_at: TimestampMillis::new(3),
        };
        database
            .apply(owner(character_id), OperationRecordId::new(), change)
            .expect("apply");
        let stored = database
            .get(owner(character_id))
            .expect("get")
            .expect("state");
        assert_eq!(stored.facts.len(), 40);
        assert_eq!(stored.facts[0].id, "old-1");
    }

    #[test]
    fn user_edits_clear_remove_and_lock_soul_growth_like_legacy() {
        use lettuce_companions::{SoulUserEdit, prepare_user_edit};
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        insert_character(&database, character_id, "edits");
        let fact = |id: &str| SoulFact {
            id: id.to_owned(),
            category: SoulCategory::Likes,
            value: format!("value {id}"),
            kind: SoulFactKind::Authored,
            policy: SoulFactPolicy::Adaptive,
            slot: id.to_owned(),
            confidence: 1.0,
            evidence_count: 1,
            weight: 1.0,
            valid_from: TimestampMillis::new(1),
            valid_until: None,
            locked: false,
            source_memory_ids: Vec::new(),
            created_at: TimestampMillis::new(1),
            supersedes: Vec::new(),
            superseded_by: None,
            superseded_at: None,
        };
        database
            .create(
                owner(character_id),
                SoulState {
                    revision: Revision::INITIAL,
                    facts: vec![fact("tea"), fact("chess")],
                },
                TimestampMillis::new(1),
            )
            .expect("create");
        let run = |edit: SoulUserEdit, at: i64| {
            let state = database
                .get(owner(character_id))
                .expect("get")
                .expect("state");
            match prepare_user_edit(&state, edit, TimestampMillis::new(at)).expect("prepare") {
                Some(change) => {
                    database
                        .apply(owner(character_id), OperationRecordId::new(), change)
                        .expect("apply");
                    true
                }
                None => false,
            }
        };
        let lock = |id: &str, locked| SoulUserEdit::SetLocked {
            fact_id: id.to_owned(),
            locked,
        };
        assert!(run(lock("tea", true), 2));
        assert!(!run(lock("tea", true), 3));
        let state = database
            .get(owner(character_id))
            .expect("get")
            .expect("state");
        assert!(
            state
                .facts
                .iter()
                .any(|fact| fact.id == "tea" && fact.locked)
        );
        assert!(run(
            SoulUserEdit::Remove {
                fact_id: "chess".to_owned()
            },
            4
        ));
        assert!(!run(
            SoulUserEdit::Remove {
                fact_id: "chess".to_owned()
            },
            5
        ));
        assert!(run(SoulUserEdit::ClearAll, 6));
        let state = database
            .get(owner(character_id))
            .expect("get")
            .expect("state");
        assert!(state.facts.is_empty());
        assert_eq!(state.revision, Revision::new(4));
        assert!(!run(SoulUserEdit::ClearAll, 7));
    }

    #[test]
    fn malformed_storage_rows_fail_closed() {
        let database = Database::open_in_memory().expect("database");
        let character_id = CharacterId::new();
        insert_character(&database, character_id, "corrupt");
        let change = prepare_growth_change_set(
            &empty(),
            Revision::INITIAL,
            vec![proposed("fact")],
            TimestampMillis::new(2),
        )
        .expect("change");
        database
            .create(owner(character_id), empty(), TimestampMillis::new(1))
            .expect("create");
        database
            .apply(owner(character_id), OperationRecordId::new(), change)
            .expect("apply");
        let connection = database.connection().expect("connection");
        connection
            .pragma_update(None, "ignore_check_constraints", true)
            .expect("ignore checks");
        connection
            .execute(
                "UPDATE companion_soul_facts SET category = 'unknown' WHERE character_id = ?1",
                [character_id.to_string()],
            )
            .expect("corrupt");
        drop(connection);
        assert_eq!(
            database.get(owner(character_id)),
            Err(SoulRepositoryError::Corrupt)
        );
    }
}
