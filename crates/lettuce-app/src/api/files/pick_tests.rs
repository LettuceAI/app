use std::sync::{Arc, Mutex};

use lettuce_contracts::{self as dto, ApiErrorCode};
use lettuce_types::OperationId;

use super::pick_filter;
use crate::api::tests::{RecordingEvents, StdFiles};
use crate::api::{
    ApiContext, FileAccess, FileAccessError, FileDescription, FileReader, PickFilter,
    files_pick_directory, files_pick_open, files_pick_save,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Pick {
    Open(PickFilter, bool),
    Save(String, PickFilter),
    Directory,
}

struct PickingFiles {
    picked: Vec<String>,
    saved: Option<String>,
    calls: Mutex<Vec<Pick>>,
}

impl FileAccess for PickingFiles {
    fn describe(&self, _uri: &str) -> Result<FileDescription, FileAccessError> {
        Err(FileAccessError::Unsupported)
    }

    fn open(&self, _uri: &str) -> Result<Box<dyn FileReader>, FileAccessError> {
        Err(FileAccessError::Unsupported)
    }

    fn create(&self, _uri: &str) -> Result<Box<dyn std::io::Write + Send>, FileAccessError> {
        Err(FileAccessError::Unsupported)
    }

    fn pick_open(
        &self,
        filter: &PickFilter,
        multiple: bool,
    ) -> Result<Vec<String>, FileAccessError> {
        self.calls
            .lock()
            .expect("calls")
            .push(Pick::Open(filter.clone(), multiple));
        Ok(self.picked.clone())
    }

    fn pick_save(
        &self,
        suggested_name: &str,
        filter: &PickFilter,
    ) -> Result<Option<String>, FileAccessError> {
        self.calls
            .lock()
            .expect("calls")
            .push(Pick::Save(suggested_name.to_owned(), filter.clone()));
        Ok(self.saved.clone())
    }

    fn pick_directory(&self) -> Result<Option<String>, FileAccessError> {
        self.calls.lock().expect("calls").push(Pick::Directory);
        Ok(self.saved.clone())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn directory_picker_returns_native_path_and_explicit_cancellation() {
    for path in [Some("/home/user/models".to_owned()), None] {
        let files = Arc::new(PickingFiles {
            picked: Vec::new(),
            saved: path.clone(),
            calls: Mutex::new(Vec::new()),
        });
        let result = files_pick_directory(&context(files.clone()))
            .await
            .expect("picker");
        assert_eq!(result.path, path);
        assert_eq!(*files.calls.lock().expect("calls"), vec![Pick::Directory]);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn directory_picker_rejects_non_filesystem_results() {
    for path in ["relative/models", "content://models", ""] {
        let files = Arc::new(PickingFiles {
            picked: Vec::new(),
            saved: Some(path.to_owned()),
            calls: Mutex::new(Vec::new()),
        });
        let error = files_pick_directory(&context(files))
            .await
            .expect_err("invalid folder");
        assert_eq!(error.code, ApiErrorCode::InvalidInput);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn directory_picker_reports_platform_unsupported() {
    let error = files_pick_directory(&context(Arc::new(StdFiles)))
        .await
        .expect_err("unsupported");
    assert_eq!(error.code, ApiErrorCode::Unsupported);
}

fn context(files: Arc<dyn FileAccess>) -> ApiContext {
    let root = std::env::temp_dir().join(format!("lettuce-api-pick-{}", OperationId::new()));
    std::fs::create_dir_all(&root).expect("root");
    ApiContext::open_desktop(
        &root,
        None,
        Arc::new(lettuce_settings::InMemorySecretStore::new()),
        Arc::new(RecordingEvents::default()),
        files,
        "lettuce-asset://localhost".into(),
        None,
    )
    .expect("context opens")
}

#[test]
fn filters_union_kinds_and_open_every_file_when_a_kind_has_no_reliable_type() {
    assert_eq!(
        pick_filter(&[dto::FilePickKind::Image, dto::FilePickKind::CharacterCard]),
        PickFilter {
            extensions: vec!["png", "jpg", "jpeg", "webp", "json", "uec"],
            mime_types: vec![
                "image/*",
                "image/png",
                "application/json",
                "application/octet-stream"
            ],
        }
    );
    assert_eq!(
        pick_filter(&[dto::FilePickKind::Json]),
        PickFilter {
            extensions: vec!["json", "uec"],
            mime_types: vec!["application/json", "application/octet-stream"]
        }
    );
    assert_eq!(
        pick_filter(&[dto::FilePickKind::Backup]),
        PickFilter {
            extensions: vec!["lettuce", "zip"],
            mime_types: vec![],
        }
    );
    assert_eq!(
        pick_filter(&[dto::FilePickKind::Json, dto::FilePickKind::ChatLog]),
        PickFilter {
            extensions: vec!["json", "uec", "jsonl"],
            mime_types: vec![],
        }
    );
    assert_eq!(pick_filter(&[]), PickFilter::default());
    assert_eq!(
        pick_filter(&[dto::FilePickKind::Image, dto::FilePickKind::Any]),
        PickFilter::default()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn open_and_save_pickers_return_typed_sources_and_targets() {
    let files = Arc::new(PickingFiles {
        picked: vec!["content://media/1".into(), "/home/user/card.png".into()],
        saved: Some("content://downloads/backup.lettuce".into()),
        calls: Mutex::new(Vec::new()),
    });
    let context = context(files.clone());
    let picked = files_pick_open(
        &context,
        dto::FilesPickOpenRequest {
            kinds: vec![dto::FilePickKind::Image],
            multiple: true,
        },
    )
    .await
    .expect("picked");
    assert_eq!(
        picked.sources,
        vec![
            dto::FileSource {
                uri: "content://media/1".into()
            },
            dto::FileSource {
                uri: "/home/user/card.png".into()
            },
        ]
    );
    let saved = files_pick_save(
        &context,
        dto::FilesPickSaveRequest {
            suggested_name: " backup.lettuce ".into(),
            kind: dto::FilePickKind::Backup,
        },
    )
    .await
    .expect("saved");
    assert_eq!(
        saved.target,
        Some(dto::FileTarget {
            uri: "content://downloads/backup.lettuce".into()
        })
    );
    assert_eq!(
        *files.calls.lock().expect("calls"),
        vec![
            Pick::Open(pick_filter(&[dto::FilePickKind::Image]), true),
            Pick::Save(
                "backup.lettuce".into(),
                pick_filter(&[dto::FilePickKind::Backup])
            ),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelled_pickers_return_nothing() {
    let context = context(Arc::new(PickingFiles {
        picked: vec![],
        saved: None,
        calls: Mutex::new(Vec::new()),
    }));
    let picked = files_pick_open(
        &context,
        dto::FilesPickOpenRequest {
            kinds: vec![],
            multiple: false,
        },
    )
    .await
    .expect("cancelled open");
    assert!(picked.sources.is_empty());
    let saved = files_pick_save(
        &context,
        dto::FilesPickSaveRequest {
            suggested_name: "chat.jsonl".into(),
            kind: dto::FilePickKind::ChatLog,
        },
    )
    .await
    .expect("cancelled save");
    assert_eq!(saved.target, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn hosts_without_pickers_and_blank_names_fail_typed() {
    let context = context(Arc::new(StdFiles));
    let open = files_pick_open(
        &context,
        dto::FilesPickOpenRequest {
            kinds: vec![],
            multiple: false,
        },
    )
    .await
    .expect_err("no picker");
    assert_eq!(open.code, ApiErrorCode::Unsupported);
    let blank = files_pick_save(
        &context,
        dto::FilesPickSaveRequest {
            suggested_name: "  ".into(),
            kind: dto::FilePickKind::Any,
        },
    )
    .await
    .expect_err("blank name");
    assert_eq!(blank.code, ApiErrorCode::InvalidInput);
}
