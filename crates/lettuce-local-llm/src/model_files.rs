//! Which model files llama.cpp holds or is about to open, and the folders a
//! move keeps it from opening: the app deletes, adopts or moves a file only
//! when no request holds it, and a request for a file in a folder being
//! moved is refused.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Default)]
struct State {
    published: Vec<String>,
    queued: Vec<(u64, Vec<String>)>,
    next_ticket: u64,
    blocked: Vec<(u64, PathBuf)>,
    next_block: u64,
}

/// The files the worker holds (published by the worker) and the files of
/// requests queued or running (recorded when they are enqueued), under one
/// lock with the folders a move blocks, so a check and a block cannot
/// interleave with an enqueue.
#[derive(Debug, Default)]
pub struct ModelFileRegistry {
    state: Mutex<State>,
}

/// Why a request was refused: it names `path`, which is inside `folder`
/// while that folder moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderMoving {
    pub path: String,
    pub folder: PathBuf,
}

/// Keeps a folder blocked until dropped.
#[derive(Debug)]
pub struct FolderBlock {
    registry: Arc<ModelFileRegistry>,
    id: u64,
}

impl Drop for FolderBlock {
    fn drop(&mut self) {
        self.registry
            .lock()
            .blocked
            .retain(|(id, _)| *id != self.id);
    }
}

fn inside(path: &str, folder: &Path) -> bool {
    let path = Path::new(path);
    path.starts_with(folder)
        || matches!(
            (std::fs::canonicalize(path), std::fs::canonicalize(folder)),
            (Ok(path), Ok(folder)) if path.starts_with(&folder)
        )
}

fn distinct(files: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for file in files {
        if !file.trim().is_empty() && !out.contains(&file) {
            out.push(file);
        }
    }
    out
}

impl ModelFileRegistry {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The files the worker holds and every queued or running request
    /// names.
    #[must_use]
    pub fn files(&self) -> Vec<String> {
        let state = self.lock();
        distinct(
            state.published.iter().cloned().chain(
                state
                    .queued
                    .iter()
                    .flat_map(|(_, files)| files.iter().cloned()),
            ),
        )
    }

    pub(crate) fn publish(&self, files: Vec<String>) {
        self.lock().published = distinct(files);
    }

    /// Records a request's files; refused when one is inside a blocked
    /// folder.
    pub(crate) fn enqueue(&self, files: Vec<String>) -> Result<u64, FolderMoving> {
        let mut state = self.lock();
        if let Some((path, folder)) = files.iter().find_map(|file| {
            state
                .blocked
                .iter()
                .find(|(_, folder)| inside(file, folder))
                .map(|(_, folder)| (file.clone(), folder.clone()))
        }) {
            return Err(FolderMoving { path, folder });
        }
        state.next_ticket += 1;
        let ticket = state.next_ticket;
        state.queued.push((ticket, files));
        Ok(ticket)
    }

    pub(crate) fn finish(&self, ticket: u64) {
        self.lock().queued.retain(|(queued, _)| *queued != ticket);
    }

    /// Blocks `folder` for as long as the returned guard lives; refused with
    /// the file when llama.cpp holds or is about to open one inside it.
    pub fn block(self: &Arc<Self>, folder: &Path) -> Result<FolderBlock, String> {
        let mut state = self.lock();
        let held = state
            .published
            .iter()
            .chain(state.queued.iter().flat_map(|(_, files)| files.iter()))
            .find(|file| inside(file, folder))
            .cloned();
        if let Some(file) = held {
            return Err(file);
        }
        state.next_block += 1;
        let id = state.next_block;
        state.blocked.push((id, folder.to_path_buf()));
        Ok(FolderBlock {
            registry: Arc::clone(self),
            id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_files_count_until_finished_and_a_block_refuses_them() {
        let registry = Arc::new(ModelFileRegistry::default());
        let ticket = registry
            .enqueue(vec!["/m/a/model.gguf".into()])
            .expect("enqueued");
        registry.publish(vec!["/m/b/other.gguf".into()]);
        assert_eq!(registry.files(), ["/m/b/other.gguf", "/m/a/model.gguf"]);
        assert_eq!(
            registry.block(Path::new("/m/a")).map(|_| ()),
            Err("/m/a/model.gguf".to_owned())
        );
        registry.finish(ticket);
        registry.publish(Vec::new());
        let block = registry.block(Path::new("/m/a")).expect("block");
        assert_eq!(
            registry.enqueue(vec!["/m/a/model.gguf".into()]),
            Err(FolderMoving {
                path: "/m/a/model.gguf".into(),
                folder: PathBuf::from("/m/a"),
            })
        );
        assert!(registry.enqueue(vec!["/m/c/x.gguf".into()]).is_ok());
        drop(block);
        assert!(registry.enqueue(vec!["/m/a/model.gguf".into()]).is_ok());
    }
}
