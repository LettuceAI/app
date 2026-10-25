use std::{
    fmt,
    ops::Deref,
    sync::{Arc, Mutex, Weak, atomic::{AtomicU64, Ordering}},
};

use lettuce_platform::MemorySnapshot;
use lettuce_settings::OnnxModelKind;

const MEMORY_HEADROOM_DIVISOR: u64 = 10;

type PressureObserver = dyn Fn(Vec<OnnxModelKind>) + Send + Sync;
type MemoryProbe = dyn Fn() -> Option<MemorySnapshot> + Send + Sync;

trait IdleCache: Send + Sync {
    fn kind(&self) -> OnnxModelKind;
    fn last_used(&self) -> Option<u64>;
    fn release_idle(&self, requested: bool) -> bool;
    fn release_requested(&self) -> bool;
    fn request_release(&self);
    fn completed(&self, generation: u64);
    fn invalidate(&self);
    fn keep_loaded(&self, keep: bool);
}

pub(crate) struct OnnxModelManager {
    caches: Mutex<Vec<Weak<dyn IdleCache>>>,
    trim_lock: Mutex<()>,
    clock: AtomicU64,
    probe: Arc<MemoryProbe>,
    observer: Arc<PressureObserver>,
    wake: tokio::sync::Notify,
    forced: std::sync::atomic::AtomicBool,
    policies: [std::sync::atomic::AtomicBool; 3],
    policy_lock: Mutex<()>,
}

impl fmt::Debug for OnnxModelManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnnxModelManager").finish_non_exhaustive()
    }
}

impl OnnxModelManager {
    pub(crate) fn new(observer: impl Fn(Vec<OnnxModelKind>) + Send + Sync + 'static) -> Arc<Self> {
        Self::with_probe(lettuce_platform::system_memory, observer)
    }

    fn with_probe(
        probe: impl Fn() -> Option<MemorySnapshot> + Send + Sync + 'static,
        observer: impl Fn(Vec<OnnxModelKind>) + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            caches: Mutex::new(Vec::new()),
            trim_lock: Mutex::new(()),
            clock: AtomicU64::new(0),
            probe: Arc::new(probe),
            observer: Arc::new(observer),
            wake: tokio::sync::Notify::new(),
            forced: std::sync::atomic::AtomicBool::new(false),
            policies: [false, true, false].map(std::sync::atomic::AtomicBool::new),
            policy_lock: Mutex::new(()),
        })
    }

    fn caches(&self) -> Vec<Arc<dyn IdleCache>> {
        let mut caches = self.caches.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        caches.retain(|cache| cache.strong_count() > 0);
        caches.iter().filter_map(Weak::upgrade).collect()
    }

    fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn check(&self) {
        self.trim(self.forced.swap(false, Ordering::AcqRel));
    }

    pub(crate) fn memory_warning(&self) {
        self.forced.store(true, Ordering::Release);
        self.notify();
    }

    fn finished(&self) {
        let mut released = Vec::new();
        for cache in self.caches() {
            cache.keep_loaded(self.retains(cache.kind()));
            if cache.release_requested() && !released.contains(&cache.kind()) {
                released.push(cache.kind());
            }
        }
        if !released.is_empty() { (self.observer)(released); }
        self.check();
    }

    fn trim(&self, force: bool) {
        let Ok(_guard) = self.trim_lock.try_lock() else {
            if force { self.forced.store(true, Ordering::Release); self.notify(); }
            return;
        };
        let pressure = || (self.probe)().is_some_and(|memory| {
            memory.available_bytes < memory.total_bytes / MEMORY_HEADROOM_DIVISOR
        });
        if !force && !pressure() { return; }
        let caches = self.caches();
        if force { for cache in &caches { cache.request_release(); } }
        let mut candidates = caches.into_iter()
            .filter_map(|cache| cache.last_used().map(|used| (used, cache)))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(used, _)| *used);
        let mut released = Vec::new();
        for (_, cache) in candidates {
            if !force && !pressure() { break; }
            if cache.release_idle(force) && !released.contains(&cache.kind()) {
                released.push(cache.kind());
            }
        }
        if !released.is_empty() {
            (self.observer)(released);
        }
    }

    pub(crate) fn invalidate(&self, kind: OnnxModelKind) {
        for cache in self.caches().into_iter().filter(|cache| cache.kind() == kind) {
            cache.invalidate();
        }
    }

    fn policy_index(kind: OnnxModelKind) -> usize {
        match kind { OnnxModelKind::Eidos => 0, OnnxModelKind::Thymos => 1, OnnxModelKind::Kokoro => 2 }
    }

    fn retains(&self, kind: OnnxModelKind) -> bool {
        self.policies[Self::policy_index(kind)].load(Ordering::Acquire)
    }

    pub(crate) fn refresh_settings(
        &self,
        read: impl FnOnce() -> Result<lettuce_settings::DeviceSettings, lettuce_settings::GlobalSettingsStoreError>,
    ) -> Result<(), lettuce_settings::GlobalSettingsStoreError> {
        let _guard = self.policy_lock.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let device = read()?;
        for kind in [OnnxModelKind::Eidos, OnnxModelKind::Thymos, OnnxModelKind::Kokoro] {
            self.policies[Self::policy_index(kind)].store(device.keep_onnx_model_loaded(kind), Ordering::Release);
        }
        for cache in self.caches() { cache.keep_loaded(self.retains(cache.kind())); }
        Ok(())
    }

    pub(crate) fn notify(&self) { self.wake.notify_one(); }
    pub(crate) async fn notified(&self) { self.wake.notified().await; }
}

struct Entry<K, T: ?Sized> {
    key: K,
    retained: Option<Arc<T>>,
    active: Weak<T>,
    last_used: u64,
    generation: u64,
}

struct CacheState<K, T: ?Sized> {
    entry: Option<Entry<K, T>>,
    active: Vec<Weak<T>>,
}

pub(crate) struct OnnxSessionCache<K, T: ?Sized> {
    kind: OnnxModelKind,
    state: Mutex<CacheState<K, T>>,
    manager: Arc<OnnxModelManager>,
    pressure_requested: std::sync::atomic::AtomicBool,
}

impl<K, T: ?Sized> fmt::Debug for OnnxSessionCache<K, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnnxSessionCache").field("kind", &self.kind).finish_non_exhaustive()
    }
}

impl<K: Clone + Eq + Send + 'static, T: ?Sized + Send + Sync + 'static> OnnxSessionCache<K, T> {
    pub(crate) fn new(kind: OnnxModelKind, manager: Arc<OnnxModelManager>) -> Arc<Self> {
        let cache = Arc::new(Self {
            kind,
            state: Mutex::new(CacheState { entry: None, active: Vec::new() }),
            manager,
            pressure_requested: std::sync::atomic::AtomicBool::new(false),
        });
        let erased: Arc<dyn IdleCache> = cache.clone();
        cache.manager.caches.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Arc::downgrade(&erased));
        cache
    }

    pub(crate) fn acquire<E>(
        self: &Arc<Self>, key: K,
        load: impl FnOnce() -> Result<Arc<T>, E>,
    ) -> Result<OnnxModelLease<T>, E> {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let existing = state.entry.as_ref().filter(|entry| entry.key == key)
            .and_then(|entry| entry.active.upgrade());
        let model = match existing {
            Some(model) => model,
            None => {
                state.entry = None;
                self.manager.check();
                load()?
            },
        };
        state.active.retain(|active| active.strong_count() > 0);
        if !state.active.iter().any(|active| Weak::ptr_eq(active, &Arc::downgrade(&model))) {
            state.active.push(Arc::downgrade(&model));
        }
        let generation = state.entry.as_ref().filter(|entry| entry.key == key)
            .map(|entry| entry.generation).unwrap_or_else(|| self.manager.tick());
        state.entry = Some(Entry {
            generation,
            key, retained: self.manager.retains(self.kind).then(|| model.clone()), active: Arc::downgrade(&model),
            last_used: self.manager.tick(),
        });
        drop(state);
        let erased: Arc<dyn IdleCache> = self.clone();
        Ok(OnnxModelLease { model: Some(model), manager: self.manager.clone(), cache: Arc::downgrade(&erased), generation })
    }

    pub(crate) fn refresh_settings(
        &self,
        read: impl FnOnce() -> Result<lettuce_settings::DeviceSettings, lettuce_settings::GlobalSettingsStoreError>,
    ) -> Result<(), lettuce_settings::GlobalSettingsStoreError> {
        self.manager.refresh_settings(read)
    }

    pub(crate) fn has_current(&self) -> bool {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry.as_ref().is_some_and(|entry| entry.active.strong_count() > 0)
    }

    pub(crate) fn contains(&self, key: &K) -> bool {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry.as_ref().is_some_and(|entry| &entry.key == key && entry.active.strong_count() > 0)
    }

    pub(crate) fn is_loaded(&self) -> bool {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
            .active.iter().any(|active| active.strong_count() > 0)
    }
}

impl<K: Clone + Eq + Send + 'static, T: ?Sized + Send + Sync + 'static> IdleCache for OnnxSessionCache<K, T> {
    fn kind(&self) -> OnnxModelKind { self.kind }
    fn last_used(&self) -> Option<u64> {
        self.state.try_lock().ok()?.entry.as_ref().map(|entry| entry.last_used)
    }
    fn release_idle(&self, requested: bool) -> bool {
        let Ok(mut state) = self.state.try_lock() else { return false; };
        let Some(entry) = state.entry.as_mut() else { return false; };
        if requested { self.pressure_requested.store(true, Ordering::Release); }
        if entry.active.strong_count() == 0 {
            self.pressure_requested.store(false, Ordering::Release);
            return false;
        }
        if entry.retained.as_ref().is_some_and(|model| Arc::strong_count(model) == 1) {
            entry.retained = None;
            self.pressure_requested.store(false, Ordering::Release);
            return true;
        }
        false
    }
    fn release_requested(&self) -> bool {
        self.pressure_requested.load(Ordering::Acquire) && self.release_idle(false)
    }
    fn request_release(&self) {
        if self.state.try_lock().map_or(true, |state| state.entry.is_some()) {
            self.pressure_requested.store(true, Ordering::Release);
        }
    }
    fn completed(&self, generation: u64) {
        if let Ok(mut state) = self.state.try_lock()
            && let Some(entry) = state.entry.as_mut()
            && entry.generation == generation
        { entry.last_used = self.manager.tick(); }
    }
    fn invalidate(&self) {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).entry = None;
        self.pressure_requested.store(false, Ordering::Release);
    }
    fn keep_loaded(&self, keep: bool) {
        let Ok(mut state) = self.state.try_lock() else { return; };
        if let Some(entry) = state.entry.as_mut() {
            if !keep { entry.retained = None; }
            else if entry.retained.is_none() && !self.pressure_requested.load(Ordering::Acquire) {
                entry.retained = entry.active.upgrade();
            }
        }
    }
}

pub(crate) struct OnnxModelLease<T: ?Sized> {
    model: Option<Arc<T>>,
    manager: Arc<OnnxModelManager>,
    cache: Weak<dyn IdleCache>,
    generation: u64,
}

impl<T: ?Sized> Clone for OnnxModelLease<T> {
    fn clone(&self) -> Self { Self { model: self.model.clone(), manager: self.manager.clone(), cache: self.cache.clone(), generation: self.generation } }
}
impl<T: ?Sized> Deref for OnnxModelLease<T> {
    type Target = T;
    fn deref(&self) -> &T { self.model.as_deref().expect("a live model lease owns its session") }
}
impl<T: ?Sized> AsRef<T> for OnnxModelLease<T> {
    fn as_ref(&self) -> &T { self }
}
impl<T: ?Sized> fmt::Debug for OnnxModelLease<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("OnnxModelLease") }
}
impl<T: ?Sized> Drop for OnnxModelLease<T> {
    fn drop(&mut self) {
        self.model.take();
        if let Some(cache) = self.cache.upgrade() { cache.completed(self.generation); }
        self.manager.finished();
    }
}

#[cfg(test)]
#[path = "onnx_models_tests.rs"]
mod tests;
