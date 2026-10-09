use super::tests::{Reply, harness_in};
use lettuce_types::TimestampMillis;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

#[derive(Debug)]
struct Clock(AtomicI64);
impl lettuce_jobs::Clock for Clock {
    fn now(&self) -> TimestampMillis {
        TimestampMillis::new(self.0.load(Ordering::SeqCst))
    }
}

#[tokio::test]
async fn usage_flush_emits_an_event_after_persistence() {
    let clock = Arc::new(Clock(AtomicI64::new(1000)));
    let h = harness_in(
        Reply::Text("hello"),
        clock.clone(),
        None,
        None,
        Arc::new(super::NoModels),
    );
    clock.0.store(4000, Ordering::SeqCst);
    h.context.flush_app_usage();
    assert!(h.events.events().iter().any(|event| serde_json::to_value(event).expect("event")["type"] == "app_usage_changed"));
    assert_eq!(
        lettuce_usage::AppUsageRepository::app_usage_days(h.context.backend().database())
            .expect("days")
            .iter()
            .map(|day| day.active_ms)
            .sum::<u64>(),
        3000
    );
}

#[tokio::test]
async fn usage_read_includes_the_current_stretch_without_a_write() {
    let clock = Arc::new(Clock(AtomicI64::new(1000)));
    let h = harness_in(
        Reply::Text("hello"),
        clock.clone(),
        None,
        None,
        Arc::new(super::NoModels),
    );
    clock.0.store(4000, Ordering::SeqCst);
    let read = super::app_usage_days(&h.context).await.expect("read");
    assert_eq!(read.days.iter().map(|day| day.active_ms).sum::<u64>(), 3000);
    assert!(
        lettuce_usage::AppUsageRepository::app_usage_days(h.context.backend().database())
            .expect("persisted")
            .is_empty()
    );
    assert_eq!(
        super::app_usage_days(&h.context).await.expect("repeat"),
        read
    );
    h.context.flush_app_usage();
    assert_eq!(
        super::app_usage_days(&h.context)
            .await
            .expect("after flush"),
        read
    );
}
