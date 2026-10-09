use notify::Watcher;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

pub(super) struct ModelSizeIndex {
    pub(super) roots: Vec<(String, PathBuf)>,
    cached: Vec<lettuce_contracts::StorageSize>,
    identities: Vec<Option<same_file::Handle>>,
    events: mpsc::Receiver<notify::Result<notify::Event>>,
    _watcher: notify::RecommendedWatcher,
    pending: Arc<AtomicBool>,
    dirty: bool,
    failure: bool,
    #[cfg(test)]
    scans: usize,
}

impl ModelSizeIndex {
    pub(super) fn new(roots: Vec<(String, PathBuf)>) -> Result<Self, ()> {
        if !cfg!(any(
            target_os = "linux",
            target_os = "android",
            target_os = "macos",
            target_os = "windows",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly",
            target_os = "ios"
        )) {
            return Err(());
        }
        let identities = root_identities(&roots)?;
        let (send, events) = mpsc::channel();
        let pending = Arc::new(AtomicBool::new(false));
        let queued = Arc::clone(&pending);
        let observed = roots
            .iter()
            .map(|(_, root)| {
                let mut ancestor = root.as_path();
                let mut suffix = Vec::new();
                while !ancestor.try_exists().map_err(|_| ())? {
                    suffix.push(ancestor.file_name().ok_or(())?.to_os_string());
                    ancestor = ancestor.parent().ok_or(())?;
                }
                let mut resolved = ancestor.canonicalize().map_err(|_| ())?;
                for part in suffix.into_iter().rev() {
                    resolved.push(part);
                }
                Ok(resolved)
            })
            .collect::<Result<Vec<_>, ()>>()?;
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let relevant = match &event {
                    Ok(event) => {
                        !matches!(event.kind, notify::EventKind::Access(_))
                            && (event.need_rescan()
                                || event.paths.iter().any(|path| {
                                    observed.iter().any(|root| {
                                        path.starts_with(root) || root.starts_with(path)
                                    })
                                }))
                    }
                    Err(_) => true,
                };
                if relevant && (event.is_err() || !queued.swap(true, Ordering::AcqRel)) {
                    let _ = send.send(event);
                }
            })
            .map_err(|_| ())?;
        let mut watched = BTreeSet::new();
        for (_, root) in &roots {
            let mut ancestor = root.as_path();
            while !ancestor.try_exists().map_err(|_| ())? {
                ancestor = ancestor.parent().ok_or(())?;
            }
            let ancestor = ancestor.canonicalize().map_err(|_| ())?;
            if watched.insert(ancestor.clone()) {
                let mode = if root.try_exists().map_err(|_| ())? {
                    notify::RecursiveMode::Recursive
                } else {
                    notify::RecursiveMode::NonRecursive
                };
                watcher.watch(&ancestor, mode).map_err(|_| ())?;
            }
        }
        let mut index = Self {
            roots,
            cached: Vec::new(),
            identities,
            events,
            _watcher: watcher,
            pending,
            dirty: true,
            failure: false,
            #[cfg(test)]
            scans: 0,
        };
        index.refresh()?;
        Ok(index)
    }

    fn apply(&mut self, event: notify::Result<notify::Event>) -> Result<(), ()> {
        event.map_err(|_| {
            self.failure = true;
        })?;
        self.dirty = true;
        Ok(())
    }

    pub(super) fn sizes(&mut self) -> Result<Vec<lettuce_contracts::StorageSize>, ()> {
        self.pending.store(false, Ordering::Release);
        loop {
            match self.events.try_recv() {
                Ok(event) => self.apply(event)?,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.failure = true;
                    return Err(());
                }
            }
        }
        if self.failure {
            return Err(());
        }
        if root_identities(&self.roots)? != self.identities {
            let rebuilt = Self::new(self.roots.clone())?;
            #[cfg(test)]
            let rebuilt = {
                let mut rebuilt = rebuilt;
                rebuilt.scans += self.scans;
                rebuilt
            };
            let sizes = rebuilt.cached.clone();
            *self = rebuilt;
            return Ok(sizes);
        }
        if self.dirty {
            self.refresh()?;
        }
        Ok(self.cached.clone())
    }

    fn refresh(&mut self) -> Result<(), ()> {
        let mut files = BTreeMap::new();
        let mut directories = BTreeSet::new();
        for (_, root) in &self.roots {
            scan(root, &mut files, &mut directories)?;
        }
        let mut order = self.roots.clone();
        order.sort_by_key(|(_, path)| std::cmp::Reverse(path.components().count()));
        let mut bytes = BTreeMap::<String, u64>::new();
        #[cfg(unix)]
        let mut identities = BTreeSet::new();
        #[cfg(not(unix))]
        let mut identities = std::collections::HashSet::new();
        for (kind, root) in &order {
            bytes.entry(kind.clone()).or_default();
            for (path, size) in files.iter().filter(|(path, _)| path.starts_with(root)) {
                #[cfg(unix)]
                let identity = {
                    use std::os::unix::fs::MetadataExt;
                    let metadata = path.metadata().map_err(|_| ())?;
                    (metadata.dev(), metadata.ino())
                };
                #[cfg(not(unix))]
                let identity = same_file::Handle::from_path(path).map_err(|_| ())?;
                if identities.insert(identity) {
                    let total = bytes.entry(kind.clone()).or_default();
                    *total = total.checked_add(*size).ok_or(())?;
                }
            }
        }
        self.cached = bytes
            .into_iter()
            .map(|(kind, bytes)| lettuce_contracts::StorageSize { kind, bytes })
            .collect();
        self.dirty = false;
        #[cfg(test)]
        {
            self.scans += 1;
        }
        Ok(())
    }
}

fn root_identities(roots: &[(String, PathBuf)]) -> Result<Vec<Option<same_file::Handle>>, ()> {
    roots
        .iter()
        .map(|(_, root)| {
            if root.try_exists().map_err(|_| ())? {
                same_file::Handle::from_path(root).map(Some).map_err(|_| ())
            } else {
                Ok(None)
            }
        })
        .collect()
}

fn scan(
    root: &Path,
    files: &mut BTreeMap<PathBuf, u64>,
    visited: &mut BTreeSet<PathBuf>,
) -> Result<(), ()> {
    let attributes = match root.metadata() {
        Ok(attributes) => attributes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(()),
    };
    if attributes.is_file() {
        files.insert(root.to_path_buf(), attributes.len());
    } else if attributes.is_dir() && visited.insert(root.canonicalize().map_err(|_| ())?) {
        for entry in std::fs::read_dir(root).map_err(|_| ())? {
            scan(&entry.map_err(|_| ())?.path(), files, visited)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    #[test]
    fn model_index_caches_reads_and_changes_only_after_filesystem_events() {
        let root = std::env::temp_dir().join(format!(
            "lettuce-model-sizes-{}",
            lettuce_types::OperationId::new()
        ));
        std::fs::create_dir_all(root.join("nested")).expect("root");
        std::fs::write(root.join("nested/model.gguf"), [1_u8; 7]).expect("model");
        let mut index =
            super::ModelSizeIndex::new(vec![("llm".into(), root.clone())]).expect("index");
        assert_eq!(index.sizes().expect("sizes")[0].bytes, 7);
        let scanned = index.scans;
        assert_eq!(index.sizes().expect("cached")[0].bytes, 7);
        assert_eq!(index.scans, scanned);
        std::fs::write(root.join("nested/model.gguf"), [2_u8; 13]).expect("change");
        let event = index
            .events
            .recv_timeout(Duration::from_secs(5))
            .expect("native filesystem event");
        index.apply(event).expect("apply");
        assert_eq!(index.sizes().expect("changed")[0].bytes, 13);
        assert!(index.scans > scanned);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn replacing_a_model_folder_rebinds_its_native_watch() {
        let parent = std::env::temp_dir().join(format!(
            "lettuce-replaced-model-root-{}",
            lettuce_types::OperationId::new()
        ));
        let root = parent.join("models");
        std::fs::create_dir_all(&root).expect("root");
        std::fs::write(root.join("model"), [1_u8; 7]).expect("model");
        let mut index =
            super::ModelSizeIndex::new(vec![("llm".into(), root.clone())]).expect("index");
        std::fs::rename(&root, parent.join("previous")).expect("move original folder");
        std::fs::create_dir(&root).expect("replacement");
        std::fs::write(root.join("model"), [2_u8; 19]).expect("new model");
        let event = index
            .events
            .recv_timeout(Duration::from_secs(5))
            .expect("move event");
        index.apply(event).expect("apply");
        assert_eq!(index.sizes().expect("replacement size")[0].bytes, 19);
        std::fs::write(root.join("model"), [3_u8; 27]).expect("replace model contents");
        let event = index
            .events
            .recv_timeout(Duration::from_secs(5))
            .expect("replacement folder remains watched");
        index.apply(event).expect("apply");
        assert_eq!(index.sizes().expect("updated size")[0].bytes, 27);
        std::fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn a_stopped_filesystem_watcher_never_returns_stale_sizes() {
        let root = std::env::temp_dir().join(format!(
            "lettuce-stopped-model-watch-{}",
            lettuce_types::OperationId::new()
        ));
        std::fs::create_dir(&root).expect("root");
        let mut index =
            super::ModelSizeIndex::new(vec![("llm".into(), root.clone())]).expect("index");
        let (send, receive) = std::sync::mpsc::channel();
        drop(send);
        index.events = receive;
        assert!(index.sizes().is_err());
        std::fs::remove_dir(root).expect("cleanup");
    }

    #[test]
    fn nested_model_folders_and_hardlinks_count_each_file_once() {
        let root = std::env::temp_dir().join(format!(
            "lettuce-model-overlap-{}",
            lettuce_types::OperationId::new()
        ));
        std::fs::create_dir_all(root.join("images")).expect("root");
        std::fs::write(root.join("images/model"), [1_u8; 11]).expect("model");
        std::fs::hard_link(root.join("images/model"), root.join("alias")).expect("hardlink");
        let mut index = super::ModelSizeIndex::new(vec![
            ("llm".into(), root.clone()),
            ("image".into(), root.join("images")),
        ])
        .expect("index");
        assert_eq!(
            index
                .sizes()
                .expect("sizes")
                .iter()
                .map(|item| item.bytes)
                .sum::<u64>(),
            11
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
