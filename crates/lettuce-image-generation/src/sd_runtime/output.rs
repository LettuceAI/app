//! sd-server console output: the bounded log tail kept for OOM detection and
//! the step lines turned into progress events.

use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

pub const RUNTIME_LOG_TAIL_LINES: usize = 240;
const PROGRESS_THROTTLE: Duration = Duration::from_millis(100);

static RUNTIME_PROGRESS_LINE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\|\s*(\d+)/(\d+)\s*-\s*(\S+)").expect("valid sd.cpp progress pattern")
});

const OOM_SIGNATURES: [&str; 8] = [
    "out of memory",
    "outofdevicememory",
    "outofhostmemory",
    "out of device memory",
    "not enough memory",
    "failed to allocate",
    "memory allocation",
    "cuda error",
];

/// Local generation progress, the `sdcpp-generation-progress` phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum GenerationProgress {
    Starting,
    Loading {
        step: u32,
        steps: u32,
    },
    Sampling {
        step: u32,
        steps: u32,
    },
    Queued {
        #[serde(rename = "queuePosition")]
        queue_position: Option<u64>,
    },
    Generating,
    Retrying,
    Cancelled,
}

/// Receives local generation progress; the host forwards it to the UI.
pub trait GenerationProgressSink: Send + Sync {
    fn progress(&self, progress: GenerationProgress);
}

#[must_use]
pub fn oom_signature_present(lines: &[String]) -> bool {
    lines.iter().any(|line| {
        let line = line.to_ascii_lowercase();
        OOM_SIGNATURES
            .iter()
            .any(|signature| line.contains(signature))
    })
}

/// What one console segment means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputSegment {
    Blank,
    /// A plain line, logged as runtime output.
    Log(String),
    /// A step line; `log` is set for the final step, which is logged.
    Progress {
        progress: GenerationProgress,
        step: u32,
        steps: u32,
        log: Option<String>,
    },
}

/// Splits console bytes on `\r` and `\n`, as the engine redraws progress
/// bars in place.
#[derive(Debug, Default)]
pub struct OutputSplitter {
    pending: String,
}

impl OutputSplitter {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.pending.push_str(&String::from_utf8_lossy(bytes));
        let mut segments = Vec::new();
        while let Some(boundary) = self.pending.find(['\r', '\n']) {
            segments.push(self.pending[..boundary].to_owned());
            self.pending.drain(..=boundary);
        }
        segments
    }

    pub fn finish(self) -> Option<String> {
        (!self.pending.is_empty()).then_some(self.pending)
    }
}

/// The last console lines of the running server, shared by its output
/// streams.
#[derive(Debug)]
pub struct RuntimeOutput {
    tail: Mutex<VecDeque<String>>,
}

impl Default for RuntimeOutput {
    fn default() -> Self {
        Self {
            tail: Mutex::new(VecDeque::with_capacity(RUNTIME_LOG_TAIL_LINES)),
        }
    }
}

/// The progress throttle of one output stream; stdout and stderr are
/// throttled separately.
#[derive(Debug, Default)]
pub struct ProgressThrottle {
    last_emit: Option<Instant>,
}

impl RuntimeOutput {
    /// Records one segment in the tail and classifies it. A progress line
    /// yields an event unless one was sent in the last 100 ms, except for the
    /// final step.
    pub fn segment(
        &self,
        segment: &str,
        throttle: &mut ProgressThrottle,
        now: Instant,
    ) -> (OutputSegment, bool) {
        let segment = segment.replace("\u{1b}[K", "");
        let trimmed = segment.trim();
        if !trimmed.is_empty()
            && let Ok(mut tail) = self.tail.lock()
        {
            if tail.len() >= RUNTIME_LOG_TAIL_LINES {
                tail.pop_front();
            }
            tail.push_back(trimmed.to_owned());
        }
        let Some(captures) = RUNTIME_PROGRESS_LINE.captures(&segment) else {
            return if trimmed.is_empty() {
                (OutputSegment::Blank, false)
            } else {
                (OutputSegment::Log(trimmed.to_owned()), false)
            };
        };
        let (Ok(step), Ok(steps)) = (captures[1].parse::<u32>(), captures[2].parse::<u32>()) else {
            return (OutputSegment::Blank, false);
        };
        let progress = if captures[3].contains("B/s") {
            GenerationProgress::Loading { step, steps }
        } else {
            GenerationProgress::Sampling { step, steps }
        };
        let throttled = step < steps
            && throttle
                .last_emit
                .is_some_and(|last| now.duration_since(last) < PROGRESS_THROTTLE);
        if !throttled {
            throttle.last_emit = Some(now);
        }
        let emit = !throttled;
        (
            OutputSegment::Progress {
                progress,
                step,
                steps,
                log: (step == steps).then(|| trimmed.to_owned()),
            },
            emit,
        )
    }

    #[must_use]
    pub fn tail(&self) -> Vec<String> {
        self.tail
            .lock()
            .map(|tail| tail.iter().cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl RuntimeOutput {
        fn segment_for_test(&self, segment: &str, now: Instant) -> (OutputSegment, bool) {
            thread_local! {
                static THROTTLE: std::cell::RefCell<ProgressThrottle> =
                    std::cell::RefCell::new(ProgressThrottle::default());
            }
            THROTTLE.with(|throttle| self.segment(segment, &mut throttle.borrow_mut(), now))
        }
    }

    #[test]
    fn separate_streams_throttle_independently() {
        let output = RuntimeOutput::default();
        let now = Instant::now();
        let mut stdout = ProgressThrottle::default();
        let mut stderr = ProgressThrottle::default();
        assert!(output.segment("| 1/9 - 1it/s", &mut stdout, now).1);
        assert!(output.segment("| 2/9 - 1it/s", &mut stderr, now).1);
        assert!(!output.segment("| 3/9 - 1it/s", &mut stdout, now).1);
    }

    #[test]
    fn oom_signatures_match_engine_failure_output() {
        assert!(oom_signature_present(&[
            "ggml_vulkan: Device memory allocation of size 1073741824 failed.".to_owned()
        ]));
        assert!(oom_signature_present(&[
            "vk::Device::allocateMemory: ErrorOutOfDeviceMemory".to_owned()
        ]));
        assert!(oom_signature_present(&[
            "CUDA error: out of memory".to_owned()
        ]));
        assert!(!oom_signature_present(&[
            "sampling completed in 12.5s".to_owned()
        ]));
    }

    #[test]
    fn progress_lines_are_classified_and_throttled_except_the_last_step() {
        let output = RuntimeOutput::default();
        let start = Instant::now();
        let (segment, emit) =
            output.segment_for_test("  |=====>      | 3/20 - 1.52it/s\u{1b}[K", start);
        assert!(emit);
        assert_eq!(
            segment,
            OutputSegment::Progress {
                progress: GenerationProgress::Sampling { step: 3, steps: 20 },
                step: 3,
                steps: 20,
                log: None,
            }
        );
        let (_, emit) =
            output.segment_for_test("|====| 4/20 - 1.52it/s", start + Duration::from_millis(50));
        assert!(!emit);
        let (segment, emit) =
            output.segment_for_test("|====| 20/20 - 1.52it/s", start + Duration::from_millis(60));
        assert!(emit);
        assert!(matches!(
            segment,
            OutputSegment::Progress { log: Some(_), .. }
        ));
        let (segment, _) =
            output.segment_for_test("|==| 5/9 - 120.3MB/s", start + Duration::from_millis(500));
        assert!(matches!(
            segment,
            OutputSegment::Progress {
                progress: GenerationProgress::Loading { step: 5, steps: 9 },
                ..
            }
        ));
        assert_eq!(
            output.segment_for_test("  loading model  ", start).0,
            OutputSegment::Log("loading model".to_owned())
        );
        assert_eq!(
            output.segment_for_test("   ", start).0,
            OutputSegment::Blank
        );
        assert_eq!(output.tail().len(), 5);
    }

    #[test]
    fn the_tail_keeps_the_newest_lines_and_segments_split_on_carriage_returns() {
        let output = RuntimeOutput::default();
        for index in 0..300 {
            output.segment_for_test(&format!("line {index}"), Instant::now());
        }
        let tail = output.tail();
        assert_eq!(tail.len(), RUNTIME_LOG_TAIL_LINES);
        assert_eq!(tail[0], "line 60");
        let mut splitter = OutputSplitter::default();
        assert_eq!(splitter.push(b"a\rb\nc"), ["a", "b"]);
        assert_eq!(splitter.finish().as_deref(), Some("c"));
    }

    #[test]
    fn progress_serializes_as_the_legacy_event_payload() {
        assert_eq!(
            serde_json::to_value(GenerationProgress::Queued {
                queue_position: Some(2)
            })
            .expect("json"),
            serde_json::json!({"phase": "queued", "queuePosition": 2})
        );
        assert_eq!(
            serde_json::to_value(GenerationProgress::Sampling { step: 1, steps: 8 }).expect("json"),
            serde_json::json!({"phase": "sampling", "step": 1, "steps": 8})
        );
    }
}
