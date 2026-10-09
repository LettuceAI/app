use lettuce_usage::{
    JobInferenceUsage, JobInferenceUsageResult, USAGE_AUTO_COST_KEY_PREFIX, UsageCostScope,
    UsageCostTarget, UsageCostTargetReader, UsageLedgerError, UsageTombstone,
};

impl UsageCostTargetReader for crate::Database {
    fn missing_cost_targets(
        &self,
        scope: UsageCostScope,
    ) -> Result<Vec<UsageCostTarget>, UsageLedgerError> {
        let (owner, automatic) = match scope {
            UsageCostScope::Recalculate => (None, false),
            UsageCostScope::Automatic { job_id } => (job_id.map(|id| id.to_string()), true),
        };
        let connection = self.connection().map_err(|_| UsageLedgerError::Storage)?;
        let mut statement = connection.prepare("SELECT u.id,u.job_id,u.admitted_at,u.record_json,u.result_json,j.kind,j.state,a.provider_kind FROM job_inference_usage u LEFT JOIN jobs j ON j.id=u.job_id LEFT JOIN provider_accounts a ON a.id=json_extract(u.record_json,'$.value.provider_account_id') WHERE u.result_json IS NOT NULL AND (?1 IS NULL OR u.job_id=?1) AND NOT EXISTS (SELECT 1 FROM job_usage_costs c WHERE c.event_id=u.id) AND (?2=0 OR NOT EXISTS (SELECT 1 FROM job_operations o WHERE o.operation_key=?3 || u.id)) ORDER BY u.admitted_at,u.id").map_err(|_| UsageLedgerError::Storage)?;
        let mut targets = Vec::new();
        let rows = statement
            .query_map(
                rusqlite::params![owner, automatic, USAGE_AUTO_COST_KEY_PREFIX],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .map_err(|_| UsageLedgerError::Storage)?;
        let mut unreadable = Vec::new();
        for row in rows {
            let (id, owner, at, record, result, kind, state, account_kind) =
                row.map_err(|_| UsageLedgerError::Storage)?;
            let decoded = (|| {
                let mut event: JobInferenceUsage = crate::decode_versioned(&record, 1).ok()?;
                if event.id.to_string() != id
                    || event.job_id.to_string() != owner
                    || event.admitted_at.get() != at
                    || event.result.is_some()
                {
                    return None;
                }
                event.result = Some(crate::decode_versioned(&result, 1).ok()?);
                event.validate_snapshot().ok()?;
                Some(event)
            })();
            let Some(event) = decoded else {
                unreadable.push(id);
                continue;
            };
            if !matches!(
                &event.result,
                Some(JobInferenceUsageResult::Response { usage: Some(_), .. })
            ) {
                continue;
            }
            let provider_kind = event
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.provider_kind.as_deref())
                .or(account_kind.as_deref());
            if provider_kind.is_some_and(|kind| !kind.eq_ignore_ascii_case("openrouter")) {
                continue;
            }
            if automatic {
                if state.as_deref().is_some_and(|state| {
                    !matches!(state, "succeeded" | "failed" | "cancelled" | "interrupted")
                }) {
                    continue;
                }
                let historical_chat = event
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.operation_kind.as_deref())
                    .is_some_and(|operation| {
                        matches!(
                            operation,
                            "chat"
                                | "continue"
                                | "regenerate"
                                | "group_chat_message"
                                | "group_chat_continue"
                                | "group_chat_regenerate"
                        )
                    });
                if kind
                    .as_deref()
                    .map_or(!historical_chat, |kind| kind != "conversation_generation")
                {
                    continue;
                }
            }
            targets.push(UsageCostTarget {
                event_id: event.id,
                attempt_id: event.logical_attempt_id,
                job_id: event.job_id,
            });
        }
        drop(statement);
        if !unreadable.is_empty() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
                .ok_or(UsageLedgerError::Storage)?;
            for id in unreadable {
                crate::Database::record_open_notice(
                    &connection,
                    crate::PurgeNoticeEntity::UsageRecord,
                    &id,
                    crate::PurgeNoticeReason::UsageRecordUnreadable,
                    lettuce_types::TimestampMillis::new(now),
                )
                .map_err(|_| UsageLedgerError::Storage)?;
            }
        }
        Ok(targets)
    }

    fn cleared_cost_target(&self, target: &UsageCostTarget) -> Result<bool, UsageLedgerError> {
        let connection = self.connection().map_err(|_| UsageLedgerError::Storage)?;
        match crate::usage_clear::dispatch_tombstone_in(&connection, target.event_id)
            .map_err(|_| UsageLedgerError::Storage)?
        {
            None => Ok(false),
            Some(UsageTombstone::Dispatch {
                event_id,
                attempt_id,
                job_id,
            }) if event_id == target.event_id
                && attempt_id == target.attempt_id
                && job_id == target.job_id =>
            {
                Ok(true)
            }
            Some(_) => Err(UsageLedgerError::Invalid),
        }
    }
}

#[cfg(test)]
mod tests {
    use lettuce_usage::{UsageCostScope, UsageCostTargetReader};

    #[test]
    fn an_unreadable_dispatch_row_is_a_notice_and_never_blocks_other_targets() {
        let database = crate::Database::open_in_memory().expect("database");
        database
            .connection()
            .expect("connection")
            .execute(
                "INSERT INTO job_inference_usage VALUES ('broken-dispatch', 'job', 1, '{\"format_version\":1,\"value\":{\"broken\":true}}', '{\"format_version\":1,\"value\":null}')",
                [],
            )
            .expect("corrupt row");
        for scope in [
            UsageCostScope::Automatic { job_id: None },
            UsageCostScope::Recalculate,
        ] {
            assert_eq!(
                database.missing_cost_targets(scope).expect("other rows"),
                Vec::new()
            );
        }
        let notices = database.purge_notices().expect("notices");
        assert_eq!(notices.len(), 1, "one open notice per unreadable row");
        assert_eq!(notices[0].entity, crate::PurgeNoticeEntity::UsageRecord);
        assert_eq!(notices[0].entity_id, "broken-dispatch");
        assert_eq!(
            notices[0].reason,
            crate::PurgeNoticeReason::UsageRecordUnreadable
        );
    }
}
