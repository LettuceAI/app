use std::sync::Arc;

/// The interleaved format a capture delivers samples in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureFormat {
    pub sample_rate_hz: u32,
    pub channels: u16,
}

/// Where a capture delivers its audio: interleaved `f32` samples as they
/// arrive. Called from the audio thread, so it never blocks for long.
pub trait CaptureSink: Send + Sync {
    fn push(&self, samples: &[f32]);
}

/// Why a capture could not start or finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MicrophoneError {
    #[error("the microphone permission was denied")]
    PermissionDenied,
    #[error("no microphone is available")]
    NoInputDevice,
    #[error("microphone capture is not supported here")]
    Unsupported,
    #[error("the microphone stopped working")]
    Failed,
}

/// A running capture; dropping it without `stop` ends the capture.
pub trait CaptureSession: Send {
    /// Ends the capture; every sample the device delivered has reached the
    /// sink when this returns.
    fn stop(self: Box<Self>) -> Result<(), MicrophoneError>;
}

/// A microphone that is ready to record: permission granted, device and
/// format chosen, nothing captured yet.
pub trait PreparedCapture: Send {
    fn format(&self) -> CaptureFormat;

    fn start(
        self: Box<Self>,
        sink: Arc<dyn CaptureSink>,
    ) -> Result<Box<dyn CaptureSession>, MicrophoneError>;
}

/// Records from the device's default microphone. The host implements it
/// for its platform (native capture in the shell); the application only
/// sees samples. `prepare` may wait for the user to answer a permission
/// prompt, so it is called off the async executor.
pub trait MicrophoneCapture: Send + Sync {
    fn prepare(&self) -> Result<Box<dyn PreparedCapture>, MicrophoneError>;
}
