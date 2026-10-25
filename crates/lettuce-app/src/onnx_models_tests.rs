use super::*;
use std::sync::{Barrier, atomic::{AtomicUsize, AtomicU64}};
use lettuce_settings::DeviceSettings;

struct Model {
    kind: OnnxModelKind,
    freed: Arc<Mutex<Vec<OnnxModelKind>>>,
    available: Arc<AtomicU64>,
}
impl Drop for Model {
    fn drop(&mut self) {
        self.freed.lock().expect("freed").push(self.kind);
        self.available.fetch_add(4, Ordering::SeqCst);
    }
}
struct Harness {
    manager: Arc<OnnxModelManager>,
    available: Arc<AtomicU64>,
    freed: Arc<Mutex<Vec<OnnxModelKind>>>,
    warnings: Arc<Mutex<Vec<Vec<OnnxModelKind>>>>,
}
impl Harness {
    fn new() -> Self {
        let available = Arc::new(AtomicU64::new(50));
        let warnings = Arc::new(Mutex::new(Vec::new()));
        let sampled = available.clone();
        let emitted = warnings.clone();
        let manager = OnnxModelManager::with_probe(
            move || Some(MemorySnapshot { total_bytes: 100, available_bytes: sampled.load(Ordering::SeqCst) }),
            move |models| emitted.lock().expect("warnings").push(models),
        );
        manager.refresh_settings(|| {
            let mut settings = DeviceSettings::default();
            for kind in [OnnxModelKind::Eidos, OnnxModelKind::Thymos, OnnxModelKind::Kokoro] {
                settings.set_keep_onnx_model_loaded(kind, true);
            }
            Ok(settings)
        }).expect("settings");
        Self { manager, available, warnings, freed: Arc::default() }
    }
    fn load(&self, kind: OnnxModelKind) -> Result<Arc<Model>, ()> {
        Ok(Arc::new(Model { kind, freed: self.freed.clone(), available: self.available.clone() }))
    }
    fn cache(&self, kind: OnnxModelKind) -> Arc<OnnxSessionCache<u64, Model>> {
        OnnxSessionCache::new(kind, self.manager.clone())
    }
}

#[test]
fn all_three_models_remain_loaded_in_independent_caches() {
    let h = Harness::new();
    let kinds = [OnnxModelKind::Eidos, OnnxModelKind::Thymos, OnnxModelKind::Kokoro];
    let caches = kinds.map(|kind| h.cache(kind));
    for (kind, cache) in kinds.into_iter().zip(&caches) {
        drop(cache.acquire(1, || h.load(kind)).expect("load"));
    }
    assert!(caches.iter().all(|cache| cache.is_loaded()));
    for cache in &caches {
        drop(cache.acquire(1, || -> Result<Arc<Model>, ()> { panic!("a retained model must not reload") }).expect("reuse"));
    }
    h.manager.invalidate(OnnxModelKind::Eidos);
    assert!(!caches[0].is_loaded());
    assert!(caches[1].is_loaded());
    assert!(caches[2].is_loaded());
    assert_eq!(*h.freed.lock().expect("freed"), vec![OnnxModelKind::Eidos]);
}

#[test]
fn independent_settings_release_only_the_selected_idle_model() {
    let h = Harness::new();
    let kinds = [OnnxModelKind::Eidos, OnnxModelKind::Thymos, OnnxModelKind::Kokoro];
    let caches = kinds.map(|kind| h.cache(kind));
    for (kind, cache) in kinds.into_iter().zip(&caches) {
        drop(cache.acquire(1, || h.load(kind)).expect("load"));
    }
    h.manager.refresh_settings(|| {
        let mut settings = DeviceSettings::default();
        settings.embedding.keep_model_loaded = true;
        settings.onnx.thymos_keep_model_loaded = false;
        settings.onnx.kokoro_keep_model_loaded = true;
        Ok(settings)
    }).expect("settings");
    assert!(caches[0].is_loaded());
    assert!(!caches[1].is_loaded());
    assert!(caches[2].is_loaded());
}

#[test]
fn memory_pressure_releases_idle_models_in_completion_order_until_headroom_recovers() {
    let h = Harness::new();
    let eidos = h.cache(OnnxModelKind::Eidos);
    let thymos = h.cache(OnnxModelKind::Thymos);
    let kokoro = h.cache(OnnxModelKind::Kokoro);
    let long = eidos.acquire(1, || h.load(OnnxModelKind::Eidos)).expect("long");
    drop(thymos.acquire(1, || h.load(OnnxModelKind::Thymos)).expect("short"));
    drop(long);
    drop(kokoro.acquire(1, || h.load(OnnxModelKind::Kokoro)).expect("newest"));
    h.available.store(5, Ordering::SeqCst);
    h.manager.check();
    assert_eq!(*h.freed.lock().expect("freed"), vec![OnnxModelKind::Thymos, OnnxModelKind::Eidos]);
    assert!(kokoro.is_loaded());
    assert_eq!(*h.warnings.lock().expect("warnings"), vec![vec![OnnxModelKind::Thymos, OnnxModelKind::Eidos]]);
}

#[test]
fn a_native_warning_never_releases_an_active_session_and_is_applied_when_it_finishes() {
    let h = Harness::new();
    let cache = h.cache(OnnxModelKind::Kokoro);
    let active = cache.acquire(1, || h.load(OnnxModelKind::Kokoro)).expect("active");
    h.manager.memory_warning();
    h.manager.check();
    assert!(h.freed.lock().expect("freed").is_empty());
    assert!(cache.is_loaded());
    drop(active);
    assert!(!cache.is_loaded());
    assert_eq!(*h.warnings.lock().expect("warnings"), vec![vec![OnnxModelKind::Kokoro]]);
    drop(cache.acquire(1, || h.load(OnnxModelKind::Kokoro)).expect("reload on demand"));
    assert!(cache.is_loaded());
}

#[test]
fn disabling_retention_keeps_active_work_and_releases_the_last_operation() {
    let h = Harness::new();
    let cache = h.cache(OnnxModelKind::Eidos);
    let active = cache.acquire(1, || h.load(OnnxModelKind::Eidos)).expect("active");
    h.manager.refresh_settings(|| Ok(DeviceSettings::default())).expect("release");
    let overlapping = cache.acquire(1, || -> Result<Arc<Model>, ()> { panic!("overlapping operations share one model") }).expect("overlap");
    drop(active);
    assert!(cache.is_loaded());
    drop(overlapping);
    assert!(!cache.is_loaded());
    assert_eq!(h.freed.lock().expect("freed").len(), 1);
}

#[test]
fn concurrent_requests_load_one_session_and_do_not_hold_other_model_caches() {
    let h = Harness::new();
    let cache = h.cache(OnnxModelKind::Eidos);
    let loads = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(8));
    let mut threads = Vec::new();
    for _ in 0..8 {
        let cache = cache.clone(); let barrier = barrier.clone(); let loads = loads.clone();
        let freed = h.freed.clone(); let available = h.available.clone();
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            let model = cache.acquire(1, || {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(Arc::new(Model { kind: OnnxModelKind::Eidos, freed, available }))
            }).expect("acquire");
            barrier.wait();
            drop(model);
        }));
    }
    for thread in threads { thread.join().expect("thread"); }
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    let thymos = h.cache(OnnxModelKind::Thymos);
    let _held = cache.state.lock().expect("busy Eidos");
    drop(thymos.acquire(1, || h.load(OnnxModelKind::Thymos)).expect("Thymos is independent"));
    assert!(thymos.is_loaded());
}

#[test]
fn failed_loads_retry_and_invalidation_does_not_kill_existing_work() {
    let h = Harness::new();
    let cache = h.cache(OnnxModelKind::Thymos);
    assert!(cache.acquire(1, || Err::<Arc<Model>, _>("load failed")).is_err());
    let old = cache.acquire(1, || h.load(OnnxModelKind::Thymos)).expect("retry");
    h.manager.invalidate(OnnxModelKind::Thymos);
    let new = cache.acquire(2, || h.load(OnnxModelKind::Thymos)).expect("replacement");
    assert!(h.freed.lock().expect("freed").is_empty());
    drop(old);
    assert_eq!(h.freed.lock().expect("freed").len(), 1);
    drop(new);
    assert!(cache.is_loaded());
}

#[test]
fn unavailable_memory_metrics_do_not_evict_retained_models() {
    let manager = OnnxModelManager::with_probe(|| None, |_| panic!("no pressure evidence"));
    let cache = OnnxSessionCache::new(OnnxModelKind::Thymos, manager.clone());
    drop(cache.acquire(1, || Ok::<_, ()>(Arc::new(42))).expect("model"));
    manager.check();
    assert!(cache.is_loaded());
}

#[test]
fn warnings_during_a_load_release_it_after_the_operation_and_empty_caches_do_not_latch_warnings() {
    let h = Harness::new();
    let eidos = h.cache(OnnxModelKind::Eidos);
    let kokoro = h.cache(OnnxModelKind::Kokoro);
    let active = eidos.acquire(1, || {
        h.manager.memory_warning();
        h.manager.check();
        h.load(OnnxModelKind::Eidos)
    }).expect("load under pressure");
    assert!(h.freed.lock().expect("freed").is_empty());
    drop(active);
    assert!(!eidos.is_loaded());
    drop(kokoro.acquire(1, || h.load(OnnxModelKind::Kokoro)).expect("new work after pressure"));
    assert!(kokoro.is_loaded());
}

#[test]
fn a_warning_racing_with_another_trim_is_retried() {
    let h = Harness::new();
    let cache = h.cache(OnnxModelKind::Thymos);
    drop(cache.acquire(1, || h.load(OnnxModelKind::Thymos)).expect("load"));
    let trim = h.manager.trim_lock.lock().expect("other trim");
    h.manager.memory_warning();
    h.manager.check();
    assert!(cache.is_loaded());
    drop(trim);
    h.manager.check();
    assert!(!cache.is_loaded());
}
