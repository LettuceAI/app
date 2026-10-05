//! Dictation: the shell's microphone records into a scratch WAV file, which
//! becomes an audio asset when the recording stops and is then transcribed.
//! No audio crosses the API; the UI sees a capture id, the input level and
//! the transcription job.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use lettuce_contracts::{
    self as dto, ApiError, ApiErrorCode, ApiEvent, SpeechFailure,
};
use lettuce_media::{AssetKind, AssetOrigin, AssetProvenanceV1, IngestRequest, RetentionClass};
use lettuce_types::{AssetId, ConversationId, RequestId, TimestampMillis};

use super::errors::speech_error;
use super::transcribe::admit_transcription;
use super::whisper::{engine_options, resolve_model};
use crate::api::ApiContext;
use crate::api::error::{IntoApiError, api_error, parse_id};
use crate::{CaptureFormat, CaptureSession, CaptureSink, MicrophoneError};

/// After a level event, the next one waits this long: a coalescing delay,
/// not a timer; nothing runs while no sample arrives.
const LEVEL_COALESCE: Duration = Duration::from_millis(50);
const RECORDING_LIFETIME_MS: i64 = 24 * 60 * 60 * 1000;
const ENDED_CAPTURES_KEPT: usize = 64;
const WAV_HEADER_BYTES: u64 = 44;
const MAX_WAV_DATA_BYTES: u64 = u32::MAX as u64 - 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    Stopped,
    Cancelled,
}

struct Active {
    id: String,
    recorder: Arc<Recorder>,
    session: Option<Box<dyn CaptureSession>>,
    level_task: Option<tokio::task::JoinHandle<()>>,
    sealed_bytes: Option<u64>,
}

#[derive(Default)]
struct Inner {
    active: Option<Active>,
    starting: bool,
    stopping: bool,
    ended: VecDeque<(String, Ended)>,
}

/// The capture in progress, if any, and the last captures that ended.
#[derive(Default)]
pub(crate) struct DictationState {
    inner: Mutex<Inner>,
}

fn lock(state: &DictationState) -> MutexGuard<'_, Inner> {
    state
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The scratch WAV a capture writes: 16-bit PCM in the capture's format,
/// its sizes patched into the header when it ends.
struct WavSpool {
    writer: BufWriter<File>,
    data_bytes: u64,
}

impl WavSpool {
    fn create(path: &Path, format: CaptureFormat) -> std::io::Result<Self> {
        let mut writer = BufWriter::new(File::create(path)?);
        let block_align = u32::from(format.channels) * 2;
        let byte_rate = format.sample_rate_hz.saturating_mul(block_align);
        writer.write_all(b"RIFF")?;
        writer.write_all(&0_u32.to_le_bytes())?;
        writer.write_all(b"WAVEfmt ")?;
        writer.write_all(&16_u32.to_le_bytes())?;
        writer.write_all(&1_u16.to_le_bytes())?;
        writer.write_all(&format.channels.to_le_bytes())?;
        writer.write_all(&format.sample_rate_hz.to_le_bytes())?;
        writer.write_all(&byte_rate.to_le_bytes())?;
        writer.write_all(&(block_align as u16).to_le_bytes())?;
        writer.write_all(&16_u16.to_le_bytes())?;
        writer.write_all(b"data")?;
        writer.write_all(&0_u32.to_le_bytes())?;
        Ok(Self {
            writer,
            data_bytes: 0,
        })
    }

    fn append(&mut self, samples: &[f32]) -> std::io::Result<()> {
        let bytes = u64::try_from(samples.len()).unwrap_or(u64::MAX).saturating_mul(2);
        if self.data_bytes.saturating_add(bytes) > MAX_WAV_DATA_BYTES {
            return Err(std::io::Error::other("the recording is too long"));
        }
        for sample in samples {
            let value = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
            self.writer.write_all(&value.to_le_bytes())?;
        }
        self.data_bytes += bytes;
        Ok(())
    }

    fn finish(mut self) -> std::io::Result<u64> {
        self.writer.flush()?;
        let data = u32::try_from(self.data_bytes).unwrap_or(u32::MAX);
        let file = self.writer.get_mut();
        file.seek(SeekFrom::Start(4))?;
        file.write_all(&data.saturating_add(WAV_HEADER_BYTES as u32 - 8).to_le_bytes())?;
        file.seek(SeekFrom::Start(40))?;
        file.write_all(&data.to_le_bytes())?;
        file.sync_all()?;
        Ok(self.data_bytes)
    }
}

const SAMPLE_BLOCK_SIZE: usize = 4_096;
const QUEUED_SAMPLE_BLOCKS: usize = 8;

trait SpoolWriter: Send {
    fn append(&mut self, samples: &[f32]) -> std::io::Result<()>;
    fn finish(self: Box<Self>) -> std::io::Result<u64>;
}
impl SpoolWriter for WavSpool {
    fn append(&mut self, samples: &[f32]) -> std::io::Result<()> { WavSpool::append(self, samples) }
    fn finish(self: Box<Self>) -> std::io::Result<u64> { WavSpool::finish(*self) }
}
enum WriteBlock { Samples(Vec<f32>), Finish }

/// Receives samples without waiting for disk I/O. Queue overload refuses
/// the recording explicitly rather than reporting silently truncated audio.
struct Recorder {
    sender: std::sync::mpsc::SyncSender<WriteBlock>,
    writer: Mutex<Option<std::thread::JoinHandle<std::io::Result<u64>>>>,
    stopped: AtomicBool,
    level: AtomicU32,
    changed: tokio::sync::Notify,
    failed: Arc<AtomicBool>,
}

impl CaptureSink for Recorder {
    fn push(&self, samples: &[f32]) {
        if self.stopped.load(Ordering::Acquire) || self.failed.load(Ordering::Acquire) { return; }
        let peak = samples.iter().fold(0.0_f32, |peak, sample| peak.max(sample.abs())).min(1.0);
        for samples in samples.chunks(SAMPLE_BLOCK_SIZE) {
            if self.sender.try_send(WriteBlock::Samples(samples.to_vec())).is_err() {
                self.failed.store(true, Ordering::Release);
                break;
            }
        }
        self.level.store(peak.to_bits(), Ordering::Release);
        self.changed.notify_one();
    }
}

impl Recorder {
    fn new(mut spool: Box<dyn SpoolWriter>) -> Result<Self, ApiError> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(QUEUED_SAMPLE_BLOCKS);
        let failed = Arc::new(AtomicBool::new(false));
        let writer_failed = failed.clone();
        let writer = std::thread::Builder::new().name("dictation-wav-writer".into()).spawn(move || {
            while let Ok(block) = receiver.recv() {
                match block {
                    WriteBlock::Samples(samples) => {
                        if let Err(error) = spool.append(&samples) {
                            writer_failed.store(true, Ordering::Release);
                            return Err(error);
                        }
                    }
                    WriteBlock::Finish => break,
                }
            }
            spool.finish()
        }).map_err(unavailable)?;
        Ok(Self { sender, writer: Mutex::new(Some(writer)), stopped: AtomicBool::new(false), level: AtomicU32::new(0), changed: tokio::sync::Notify::new(), failed })
    }

    fn level_permille(&self) -> u16 {
        let level = f32::from_bits(self.level.load(Ordering::Acquire));
        (level.clamp(0.0, 1.0) * 1000.0).round() as u16
    }

    fn finish(&self) -> Result<u64, ApiError> {
        self.stopped.store(true, Ordering::Release);
        let writer = self.writer.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take()
            .ok_or_else(|| api_error(ApiErrorCode::Internal, "the recording was closed"))?;
        // Called on the stop worker after the native session stops, never
        // from the audio callback. Drain queued samples before patching WAV.
        let _ = self.sender.send(WriteBlock::Finish);
        let bytes = writer.join().map_err(|_| unavailable("the recording writer stopped unexpectedly"))?.map_err(unavailable)?;
        if self.failed.load(Ordering::Acquire) { return Err(unavailable("the recording writer fell behind or failed")); }
        Ok(bytes)
    }
}

fn unavailable(error: impl std::fmt::Display) -> ApiError {
    api_error(ApiErrorCode::Unavailable, error.to_string())
}

fn microphone_error(error: MicrophoneError) -> ApiError {
    match error {
        MicrophoneError::PermissionDenied => speech_error(
            ApiErrorCode::Unavailable,
            SpeechFailure::MicrophonePermissionDenied,
            error.to_string(),
        ),
        MicrophoneError::NoInputDevice => speech_error(
            ApiErrorCode::Unavailable,
            SpeechFailure::NoMicrophone,
            error.to_string(),
        ),
        MicrophoneError::Unsupported => api_error(ApiErrorCode::Unsupported, error.to_string()),
        MicrophoneError::Failed => unavailable(error),
    }
}

fn scratch_path(context: &ApiContext, capture_id: &str) -> Result<PathBuf, ApiError> {
    let folder = context
        .app_folder()
        .ok_or_else(|| unavailable("no app data folder is open"))?;
    let root = crate::dictation_scratch_root(folder);
    std::fs::create_dir_all(&root).map_err(unavailable)?;
    Ok(root.join(format!("{capture_id}.wav")))
}

fn remember_ended(inner: &mut Inner, id: String, how: Ended) {
    inner.ended.push_back((id, how));
    while inner.ended.len() > ENDED_CAPTURES_KEPT {
        inner.ended.pop_front();
    }
}

fn no_such_capture(inner: &Inner, capture_id: &str) -> ApiError {
    if inner.ended.iter().any(|(id, _)| id == capture_id) {
        api_error(ApiErrorCode::Conflict, "the dictation has already ended")
    } else {
        api_error(ApiErrorCode::NotFound, "no such dictation")
    }
}

/// Starts recording from the microphone. Refused without a Whisper model,
/// while another dictation records, and when the microphone is not allowed
/// or missing.
pub async fn dictation_start(
    context: &ApiContext,
    request: dto::DictationStartRequest,
) -> Result<dto::DictationStarted, ApiError> {
    if context.shutdown_token().is_cancelled() {
        return Err(api_error(ApiErrorCode::Unavailable, "the application is shutting down"));
    }
    if let Some(conversation_id) = &request.conversation_id {
        parse_id::<ConversationId>(conversation_id, "conversation_id")?;
    }
    let capture_id = RequestId::new().to_string();
    {
        let mut inner = lock(context.speech_state().dictation());
        if inner.active.is_some() || inner.starting || inner.stopping {
            return Err(api_error(
                ApiErrorCode::Conflict,
                "a dictation is already recording",
            ));
        }
        inner.starting = true;
    }
    let started = start_capture(context, &capture_id).await;
    let mut inner = lock(context.speech_state().dictation());
    inner.starting = false;
    let (recorder, session) = started?;
    let level_context = context.clone();
    let level_recorder = Arc::clone(&recorder);
    let level_id = capture_id.clone();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let level_task = tokio::spawn(async move {
        if ready_rx.await.is_err() {
            return;
        }
        loop {
            tokio::select! {
                biased;
                () = level_context.shutdown_token().cancelled() => {
                    let _ = level_context.blocking(move |context| {
                        if let Ok(mut active) = take_active(context, &level_id, Ended::Cancelled) {
                            active.level_task.take();
                            discard_capture(context, active)?;
                        }
                        Ok(())
                    }).await;
                    return;
                }
                () = level_recorder.changed.notified() => {}
            }
            level_context.emit(ApiEvent::DictationLevel {
                capture_id: level_id.clone(),
                level: level_recorder.level_permille(),
            });
            tokio::time::sleep(LEVEL_COALESCE).await;
        }
    });
    inner.active = Some(Active {
        id: capture_id.clone(),
        recorder,
        session: Some(session),
        level_task: Some(level_task),
        sealed_bytes: None,
    });
    let _ = ready_tx.send(());
    Ok(dto::DictationStarted { capture_id })
}

async fn start_capture(
    context: &ApiContext,
    capture_id: &str,
) -> Result<(Arc<Recorder>, Box<dyn CaptureSession>), ApiError> {
    let capture_id = capture_id.to_owned();
    context
        .blocking(move |context| {
            resolve_model(context, None)?;
            let microphone = context.speech().microphone().ok_or_else(|| {
                api_error(
                    ApiErrorCode::Unsupported,
                    "this device cannot record from a microphone",
                )
            })?;
            let prepared = microphone.prepare().map_err(microphone_error)?;
            let path = scratch_path(context, &capture_id)?;
            let spool = WavSpool::create(&path, prepared.format()).map_err(unavailable)?;
            let recorder = Arc::new(Recorder::new(Box::new(spool))?);
            match prepared.start(Arc::clone(&recorder) as Arc<dyn CaptureSink>) {
                Ok(session) => Ok((recorder, session)),
                Err(error) => {
                    let _ = recorder.finish();
                    std::fs::remove_file(&path).ok();
                    Err(microphone_error(error))
                }
            }
        })
        .await
}

fn take_active(context: &ApiContext, capture_id: &str, how: Ended) -> Result<Active, ApiError> {
    let mut inner = lock(context.speech_state().dictation());
    match inner.active.take() {
        Some(active) if active.id == capture_id => {
            remember_ended(&mut inner, active.id.clone(), how);
            inner.stopping = how == Ended::Stopped;
            Ok(active)
        }
        other => {
            inner.active = other;
            Err(no_such_capture(&inner, capture_id))
        }
    }
}

/// Ends the recording, stores it as an audio asset and transcribes it. A
/// recording with no sound is refused.
pub async fn dictation_stop(
    context: &ApiContext,
    request: dto::DictationStopRequest,
) -> Result<dto::JobAccepted, ApiError> {
    let capture_id = request.capture_id.clone();
    {
        let inner = lock(context.speech_state().dictation());
        if inner.active.as_ref().is_none_or(|active| active.id != capture_id) {
            return Err(no_such_capture(&inner, &capture_id));
        }
    }
    let model_id = request.model_id.clone();
    let model = context
        .blocking(move |context| resolve_model(context, model_id.as_deref()))
        .await?;
    let mut active = take_active(context, &capture_id, Ended::Stopped)?;
    if let Some(task) = active.level_task.take() {
        task.abort();
        let _ = task.await;
    }
    context
        .blocking(move |context| {
            let path = match scratch_path(context, &capture_id) {
                Ok(path) => path,
                Err(error) => {
                    let mut inner = lock(context.speech_state().dictation());
                    inner.ended.retain(|(id, _)| id != &capture_id);
                    inner.active = Some(active);
                    inner.stopping = false;
                    return Err(error);
                }
            };
            if active.sealed_bytes.is_none() {
                let stopped = active.session.take().map_or(Ok(()), |session| session.stop()).map_err(microphone_error);
                let written = active.recorder.finish();
                match stopped.and(written) {
                    Ok(bytes) if bytes > 0 => active.sealed_bytes = Some(bytes),
                    result => {
                        lock(context.speech_state().dictation()).stopping = false;
                        std::fs::remove_file(&path).ok();
                        return Err(result.err().unwrap_or_else(|| speech_error(ApiErrorCode::InvalidInput, SpeechFailure::NoAudioCaptured, "nothing was recorded")));
                    }
                }
            }
            let audio = match ingest_recording(context, &path) {
                Ok(audio) => audio,
                Err(error) => {
                    // Keep a sealed capture for stop retry; do not call finish again.
                    let mut inner = lock(context.speech_state().dictation());
                    inner.ended.retain(|(id, _)| id != &capture_id);
                    inner.active = Some(active);
                    inner.stopping = false;
                    return Err(error);
                }
            };
            std::fs::remove_file(&path).ok();
            lock(context.speech_state().dictation()).stopping = false;
            admit_transcription(context, RequestId::new(), audio, model, engine_options(&request.options)).map_err(|mut error| {
                error.details = Some(dto::ApiErrorDetails::CapturedAudio { audio: context.asset_ref(audio) });
                error
            })
        })
        .await
}

fn ingest_recording(context: &ApiContext, path: &Path) -> Result<AssetId, ApiError> {
    let media = context
        .media()
        .ok_or_else(|| unavailable("no media store is open"))?;
    let file = File::open(path).map_err(unavailable)?;
    let expires_at = TimestampMillis::new(context.now().get().saturating_add(RECORDING_LIFETIME_MS));
    media
        .ingest(
            file,
            IngestRequest::new(
                AssetKind::OtherAudio,
                AssetOrigin::Upload,
                RetentionClass::Temporary { expires_at },
                AssetProvenanceV1::default(),
            ),
        )
        .map(|ingested| ingested.asset.id)
        .map_err(IntoApiError::into_api_error)
}

/// Throws the recording away.
pub async fn dictation_cancel(
    context: &ApiContext,
    request: dto::DictationCancelRequest,
) -> Result<(), ApiError> {
    let mut active = take_active(context, &request.capture_id, Ended::Cancelled)?;
    if let Some(task) = active.level_task.take() {
        task.abort();
        let _ = task.await;
    }
    context.blocking(move |context| discard_capture(context, active)).await
}

fn discard_capture(context: &ApiContext, mut active: Active) -> Result<(), ApiError> {
    if let Some(session) = active.session.take()
        && let Err(error) = session.stop()
    {
        tracing::debug!(%error, "the cancelled capture did not stop cleanly");
    }
    if active.sealed_bytes.is_none() { let _ = active.recorder.finish(); }
    std::fs::remove_file(scratch_path(context, &active.id)?).ok();
    Ok(())
}

/// Removes the recordings a stopped process left half written.
pub(crate) fn sweep_scratch(context: &ApiContext) {
    let Some(folder) = context.app_folder() else {
        return;
    };
    let root = crate::dictation_scratch_root(folder);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.path().extension().is_some_and(|extension| extension == "wav")
            && let Err(error) = std::fs::remove_file(entry.path())
        {
            tracing::warn!(%error, "a leftover dictation recording could not be removed");
        }
    }
}

#[cfg(test)]
mod recorder_tests {
    use super::*;
    struct SlowWriter {
        entered: std::sync::mpsc::Sender<std::thread::ThreadId>,
        release: std::sync::mpsc::Receiver<()>,
        first: bool,
        bytes: u64,
    }
    impl SpoolWriter for SlowWriter {
        fn append(&mut self, samples: &[f32]) -> std::io::Result<()> {
            if self.first {
                self.first = false;
                self.entered.send(std::thread::current().id()).expect("entered");
                self.release.recv().expect("release slow writer");
            }
            self.bytes += samples.len() as u64 * 2;
            Ok(())
        }
        fn finish(self: Box<Self>) -> std::io::Result<u64> { Ok(self.bytes) }
    }
    #[test]
    fn slow_writer_never_blocks_the_callback_and_overload_is_a_failure() {
        let (entered, entered_rx) = std::sync::mpsc::channel();
        let (release, release_rx) = std::sync::mpsc::channel();
        let recorder = Recorder::new(Box::new(SlowWriter { entered, release: release_rx, first: true, bytes: 0 })).expect("writer");
        recorder.push(&[0.25]);
        let writer_thread = entered_rx.recv_timeout(Duration::from_secs(1)).expect("writer started");
        assert_ne!(writer_thread, std::thread::current().id());
        // The writer cannot advance until release; push must still return.
        for _ in 0..QUEUED_SAMPLE_BLOCKS { recorder.push(&[0.25; SAMPLE_BLOCK_SIZE]); }
        assert!(!recorder.failed.load(Ordering::Acquire));
        recorder.push(&[0.25]);
        assert!(recorder.failed.load(Ordering::Acquire));
        release.send(()).expect("release");
        assert_eq!(recorder.finish().expect_err("no silently truncated success").code, ApiErrorCode::Unavailable);
    }
}
