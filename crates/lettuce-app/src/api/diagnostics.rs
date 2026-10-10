use std::path::Path;

use lettuce_contracts::{self as dto, ApiError, ApiErrorCode};
use lettuce_models::ModelCatalog;
use lettuce_settings::GlobalSettingsStore;

use super::{ApiContext, embedding_health, error::model_error, models::ModelLoad};

fn redact_support_path(path: &str, homes: &[&str]) -> String {
    let path = path.replace('\\', "/");
    let windows = path.as_bytes().get(1) == Some(&b':');
    for home in homes {
        let home = home.replace('\\', "/");
        let home = home.trim_end_matches('/');
        if home.is_empty() {
            continue;
        }
        let matches = if windows {
            path.get(..home.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(home))
        } else {
            path.starts_with(home)
        };
        if matches && (path.len() == home.len() || path.as_bytes().get(home.len()) == Some(&b'/')) {
            return format!("~{}", &path[home.len()..]);
        }
    }
    let parts = path.split('/').collect::<Vec<_>>();
    let prefix = match parts.as_slice() {
        ["", "home" | "Users", user, ..] if !user.is_empty() => Some(3),
        [drive, users, user, ..]
            if drive.ends_with(':') && users.eq_ignore_ascii_case("users") && !user.is_empty() =>
        {
            Some(3)
        }
        ["", "data", "user" | "user_de", uid, app, ..]
            if uid.parse::<u32>().is_ok() && !app.is_empty() =>
        {
            Some(5)
        }
        ["", "data", "data", app, ..] if !app.is_empty() => Some(4),
        ["", "root", ..] => Some(2),
        _ => None,
    };
    prefix.map_or(path.clone(), |prefix| {
        let suffix = parts[prefix..].join("/");
        if suffix.is_empty() {
            "~".into()
        } else {
            format!("~/{suffix}")
        }
    })
}

fn redact_support_text(value: &str, homes: &[&str]) -> String {
    let redacted = redact_support_path(value, homes);
    if redacted.starts_with('~') {
        redacted
    } else {
        value.to_owned()
    }
}

fn redact_support_json(value: &serde_json::Value, homes: &[&str]) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(redact_support_text(text, homes))
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| redact_support_json(item, homes))
                .collect(),
        ),
        serde_json::Value::Object(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(key, item)| (key.clone(), redact_support_json(item, homes)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn storage_error() -> ApiError {
    super::logs::unavailable(
        dto::LogFailureReason::Storage,
        "diagnostics storage cannot be read",
    )
}

fn database_size(path: &Path) -> Result<u64, ApiError> {
    let mut size = 0_u64;
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        match std::fs::symlink_metadata(Path::new(&name)) {
            Ok(metadata) if metadata.is_file() => {
                size = size.checked_add(metadata.len()).ok_or_else(storage_error)?;
            }
            Err(error) if !suffix.is_empty() && error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(storage_error()),
        }
    }
    Ok(size)
}

pub async fn logs_diagnostics_report(context: &ApiContext) -> Result<String, ApiError> {
    context
        .blocking(|context| {
            let engine = match context.models().resolve_embedding(context) {
                ModelLoad::Loaded(engine) => engine,
                ModelLoad::NotInstalled => {
                    return Err(model_error(
                        ApiErrorCode::ModelRequired,
                        dto::RequiredModel::Embedding,
                    ));
                }
                ModelLoad::Unavailable => {
                    return Err(model_error(
                        ApiErrorCode::ModelUnavailable,
                        dto::RequiredModel::Embedding,
                    ));
                }
            };
            let health = embedding_health::report(engine.as_ref(), context.shutdown_token())
                .map_err(|_| {
                    model_error(
                        ApiErrorCode::ModelUnavailable,
                        dto::RequiredModel::Embedding,
                    )
                })?;
            let folder = context.app_folder().ok_or_else(storage_error)?;
            let files = context.database_files().ok_or_else(storage_error)?;
            let roots = context.retained_model_roots()?;
            let embedding = crate::EmbeddingModelCoordinator::new(
                Path::new(roots.embedding.as_deref().ok_or_else(storage_error)?),
                context.backend().database(),
            )
            .active()
            .map_err(|_| {
                model_error(
                    ApiErrorCode::ModelUnavailable,
                    dto::RequiredModel::Embedding,
                )
            })?
            .ok_or_else(|| {
                model_error(ApiErrorCode::ModelRequired, dto::RequiredModel::Embedding)
            })?;
            embedding.verify().map_err(|_| {
                model_error(
                    ApiErrorCode::ModelUnavailable,
                    dto::RequiredModel::Embedding,
                )
            })?;
            if embedding.family.vector_space() != engine.source_revision() {
                return Err(model_error(
                    ApiErrorCode::ModelUnavailable,
                    dto::RequiredModel::Embedding,
                ));
            }
            let database = context.backend().database();
            let settings = database
                .load()
                .map_err(super::app::settings_error)?
                .settings;
            let providers = database.provider_accounts().map_err(|_| storage_error())?;
            let (models, _, _) = database
                .model_catalog_snapshot()
                .map_err(|_| storage_error())?;
            let homes = [
                std::env::var("HOME").ok(),
                std::env::var("USERPROFILE").ok(),
            ];
            let homes = homes
                .iter()
                .filter_map(Option::as_deref)
                .collect::<Vec<_>>();
            let generated = chrono::DateTime::from_timestamp_millis(context.now().get())
                .ok_or_else(storage_error)?
                .to_rfc3339();
            let memory = &settings.dynamic_memory;
            let mut lines = vec![
                "LettuceAI Diagnostics".into(),
                format!("Generated: {generated}"),
                String::new(),
                "App".into(),
                "- Name: LettuceAI".into(),
                format!(
                    "- Version: {}",
                    crate::app_version(env!("CARGO_PKG_VERSION"))
                ),
                format!(
                    "- Build: {}",
                    if cfg!(debug_assertions) {
                        "debug"
                    } else {
                        "release"
                    }
                ),
                String::new(),
                "Device".into(),
                format!(
                    "- Platform: {} ({})",
                    if cfg!(any(target_os = "android", target_os = "ios")) {
                        "mobile"
                    } else {
                        "desktop"
                    },
                    std::env::consts::OS
                ),
                format!("- Arch: {}", std::env::consts::ARCH),
                String::new(),
                "Storage".into(),
                format!(
                    "- App data path: {}",
                    redact_support_path(folder.to_str().ok_or_else(storage_error)?, &homes)
                ),
                format!("- Database size: {} bytes", database_size(&files.active)?),
                String::new(),
                "App State".into(),
                format!("- Pure Mode: {:?}", settings.pure_mode),
                format!(
                    "- Analytics: {}",
                    if settings.analytics_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ),
                String::new(),
                "Providers".into(),
            ];
            if providers.is_empty() {
                lines.push("- none".into());
            }
            for provider in &providers {
                lines.push(format!("- {} ({})", provider.provider_kind, provider.label));
            }
            lines.extend([String::new(), "Models".into()]);
            if models.is_empty() {
                lines.push("- none".into());
            }
            for model in models {
                let provider = providers
                    .iter()
                    .find(|provider| provider.id == model.provider_account_id)
                    .ok_or_else(storage_error)?;
                let view = super::model_profiles::view(model).map_err(|_| storage_error())?;
                lines.extend([
                    format!("- {}", view.id),
                    format!(
                        "  name: {}",
                        redact_support_text(&view.external_model_id, &homes)
                    ),
                    format!("  display: {}", view.display_name),
                    format!(
                        "  provider: {} ({})",
                        provider.provider_kind, provider.label
                    ),
                    format!("  inputScopes: {:?}", view.input_scopes),
                    format!("  outputScopes: {:?}", view.output_scopes),
                    format!("  advanced: {}", redact_support_json(&view.config, &homes)),
                ]);
            }
            lines.extend([
                String::new(),
                "Memory System".into(),
                format!(
                    "- Dynamic memory enabled: {}",
                    if memory.enabled { "yes" } else { "no" }
                ),
                format!("- Summary interval: {}", memory.summary_message_interval),
                format!("- Max entries: {}", memory.max_entries),
                format!(
                    "- Min similarity: {}",
                    memory.min_similarity_basis_points.map_or_else(
                        || "model default".into(),
                        |value| (f64::from(value) / 10000.0).to_string()
                    )
                ),
                format!("- Hot token budget: {}", memory.hot_memory_token_budget),
                format!(
                    "- Decay rate: {}",
                    f64::from(memory.decay_rate_basis_points) / 10000.0
                ),
                format!(
                    "- Cold threshold: {}",
                    f64::from(memory.cold_threshold_basis_points) / 10000.0
                ),
                format!(
                    "- Context enrichment: {}",
                    if memory.context_enrichment_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ),
                String::new(),
                "Embedding Model".into(),
                "- Installed: yes".into(),
                format!("- Version: {:?}", embedding.family),
                format!("- Source version: {}", embedding.source_revision),
                format!("- Max tokens: {}", embedding.max_sequence_length),
                String::new(),
                "Embedding Test".into(),
                format!("- Success: {}", if health.passed() { "yes" } else { "no" }),
                format!(
                    "- Message: {}",
                    if health.passed() {
                        "Embedding health check passed"
                    } else {
                        "Embedding health check failed"
                    }
                ),
                format!(
                    "- Health: {} (identity {:.4})",
                    if health.identity_cosine >= 0.9990 {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                    health.identity_cosine
                ),
                format!(
                    "- Retrieval: {} (top-1 {:.0}%, top-3 {:.0}%, mrr {:.2})",
                    if health.top1_rate >= 0.60 {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                    health.top1_rate * 100.0,
                    health.top3_rate * 100.0,
                    health.mrr
                ),
                format!(
                    "- Separation: {} (related {:.3}, unrelated {:.3}, margin {:.3})",
                    if health.related_avg - health.unrelated_avg >= 0.10 {
                        "PASS"
                    } else {
                        "FAIL"
                    },
                    health.related_avg,
                    health.unrelated_avg,
                    health.related_avg - health.unrelated_avg
                ),
                "- Retrieval cases:".into(),
            ]);
            for case in health.cases {
                lines.push(format!(
                    "  - {}: rank #{} {} (top {:.3}, expected {:.3})",
                    case.name,
                    case.rank,
                    if case.rank == 1 { "PASS" } else { "FAIL" },
                    case.top_score,
                    case.correct_score
                ));
            }
            Ok(lines.join("\n"))
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct DiagnosticModels;

    struct DiagnosticEngine(Arc<dyn crate::MemoryEmbeddingEngine>);

    impl crate::MemoryEmbeddingEngine for DiagnosticEngine {
        fn source_revision(&self) -> &str {
            "v4"
        }
        fn dimensions(&self) -> lettuce_embeddings::EmbeddingDimensions {
            self.0.dimensions()
        }
        fn count_tokens(&self, text: &str) -> Result<u32, crate::EmbeddingGenerationError> {
            self.0.count_tokens(text)
        }
        fn embed_memory(
            &self,
            request: &lettuce_embeddings::EmbeddingRequest,
            cancel: &lettuce_jobs::handle::CancellationToken,
        ) -> Result<lettuce_embeddings::EmbeddingVector, crate::EmbeddingGenerationError> {
            let mut vector = self.0.embed_memory(request, cancel)?;
            vector.source_revision = "v4".into();
            Ok(vector)
        }
    }

    #[async_trait::async_trait]
    impl super::super::ModelLoader for DiagnosticModels {
        fn installed(&self, _: &ApiContext, model: dto::RequiredModel) -> bool {
            model == dto::RequiredModel::Embedding
        }
        async fn prepare(&self, _: &ApiContext) -> bool {
            true
        }
        fn embedding(&self, _: &ApiContext) -> ModelLoad<Arc<dyn crate::MemoryEmbeddingEngine>> {
            ModelLoad::Loaded(Arc::new(DiagnosticEngine(embedding_health::tests::engine(
                false,
            ))))
        }
        fn emotion(&self, _: &ApiContext) -> ModelLoad<Arc<dyn crate::CompanionEmotionEngine>> {
            ModelLoad::NotInstalled
        }
    }

    #[tokio::test]
    async fn diagnostics_reports_real_measurements_and_refuses_missing_installed_files() {
        use lettuce_model_hub::{
            EmbeddingInstallStore, EmbeddingModelFamily, InstalledEmbeddingManifest,
            InstalledModelArtifact,
        };
        use lettuce_platform::{DirectorySnapshot, FilesystemAuthority};
        use lettuce_types::{OperationId, TimestampMillis};

        let root = std::env::temp_dir().join(format!("lettuce-diagnostics-{}", OperationId::new()));
        let authority = FilesystemAuthority::new(&DirectorySnapshot::new(&root).expect("snapshot"))
            .expect("authority");
        let location =
            crate::AppDatabaseLocation::new(root.join("private-persistent-v2"), &authority)
                .expect("location");
        let active = location.active_path().expect("active");
        let backend =
            Arc::new(crate::AppBackend::open(&active, TimestampMillis::new(10)).expect("backend"));
        let h = super::super::tests::harness_over_files(
            backend,
            super::super::tests::Reply::Text("Hello."),
            Arc::new(lettuce_jobs::SystemClock),
            None,
            Some(root.clone()),
            Arc::new(DiagnosticModels),
            Arc::new(super::super::tests::NoImages),
            Some(super::super::ApiDatabaseFiles { location, active }),
        );
        let roots = h.context.retained_model_roots().expect("roots");
        let embedding_root = Path::new(roots.embedding.as_deref().expect("embedding root"));
        std::fs::create_dir_all(embedding_root).expect("embedding directory");
        let model = embedding_root.join("diagnostic-model.onnx");
        let tokenizer = embedding_root.join("diagnostic-tokenizer.json");
        std::fs::write(&model, b"fixture model").expect("model");
        std::fs::write(&tokenizer, b"fixture tokenizer").expect("tokenizer");
        EmbeddingInstallStore::new(embedding_root)
            .record(&InstalledEmbeddingManifest {
                family: EmbeddingModelFamily::LettuceEmbV4,
                source_revision: "diagnostic-revision".into(),
                model: InstalledModelArtifact::inspect(model.clone()).expect("model identity"),
                tokenizer: InstalledModelArtifact::inspect(tokenizer.clone())
                    .expect("tokenizer identity"),
                calibration: None,
                max_sequence_length: 128,
                native_dimensions: 768,
            })
            .expect("verified manifest");
        let report = logs_diagnostics_report(&h.context)
            .await
            .expect("diagnostics");
        for field in [
            "App\n",
            "Device\n",
            "Storage\n",
            "App State\n",
            "Providers\n",
            "Models\n",
            "Memory System\n",
            "Embedding Model\n",
            "Embedding Test\n",
            "- Source version: diagnostic-revision",
            "identity 1.0000",
            "top-1 100%, top-3 100%, mrr 1.00",
            "related 1.000, unrelated 0.000, margin 1.000",
        ] {
            assert!(report.contains(field), "missing field {field}");
        }
        assert_eq!(report.matches(": rank #1 PASS").count(), 50);
        std::fs::remove_file(tokenizer).expect("remove installed tokenizer");
        let error = logs_diagnostics_report(&h.context)
            .await
            .expect_err("missing file is typed");
        assert_eq!(error.code, ApiErrorCode::ModelUnavailable);
        drop(h);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn support_paths_redact_home_components_on_desktop_and_android() {
        for (path, homes, expected) in [
            (
                "/home/alice/.local/share/lettuce",
                vec!["/home/alice"],
                "~/.local/share/lettuce",
            ),
            ("/home/alice/app", vec!["/home/al"], "~/app"),
            (
                "/Users/alice/Library/Application Support/lettuce",
                vec![],
                "~/Library/Application Support/lettuce",
            ),
            (
                r"C:\Users\Alice\AppData\Roaming\lettuce",
                vec![r"c:\users\alice"],
                "~/AppData/Roaming/lettuce",
            ),
            ("/data/user/0/com.lettuceai.app/files", vec![], "~/files"),
            ("/data/data/com.lettuceai.app/files", vec![], "~/files"),
            (
                "/tmp/lettuce-diagnostics",
                vec!["/home/alice"],
                "/tmp/lettuce-diagnostics",
            ),
        ] {
            assert_eq!(redact_support_path(path, &homes), expected);
        }
    }

    #[test]
    fn model_names_and_advanced_settings_redact_local_paths() {
        let homes = ["/home/alice", r"C:\Users\Alice"];
        assert_eq!(
            redact_support_text("/home/alice/models/local.gguf", &homes),
            "~/models/local.gguf"
        );
        assert_eq!(redact_support_text("gpt-4o", &homes), "gpt-4o");
        let config = serde_json::json!({
            "companionPaths": {"mmproj": r"C:\Users\Alice\models\mmproj.gguf"},
            "draft": ["/home/alice/models/draft.gguf"],
            "stop": r"a\b",
            "contextLength": 4096
        });
        assert_eq!(
            redact_support_json(&config, &homes),
            serde_json::json!({
                "companionPaths": {"mmproj": "~/models/mmproj.gguf"},
                "draft": ["~/models/draft.gguf"],
                "stop": r"a\b",
                "contextLength": 4096
            })
        );
    }

    #[tokio::test]
    async fn diagnostics_missing_embedding_is_a_typed_model_requirement() {
        let h = super::super::tests::harness(super::super::tests::Reply::Text("Hello."));
        let error = logs_diagnostics_report(&h.context)
            .await
            .expect_err("missing required model");
        assert_eq!(error.code, lettuce_contracts::ApiErrorCode::ModelRequired);
        assert!(matches!(
            error.details,
            Some(lettuce_contracts::ApiErrorDetails::Model { .. })
        ));
    }
}
