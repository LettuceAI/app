use std::sync::Arc;

use lettuce_app::MicrophoneCapture;

pub(crate) fn capture() -> Option<Arc<dyn MicrophoneCapture>> {
    #[cfg(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos",
        target_os = "android"
    ))]
    {
        Some(Arc::new(desktop::NativeMicrophone))
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos",
        target_os = "android"
    )))]
    {
        None
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "macos",
    target_os = "android"
))]
mod desktop {
    use std::sync::{Arc, mpsc};
    use std::thread::JoinHandle;

    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{Device, SampleFormat, StreamConfig};
    use lettuce_app::{
        CaptureFormat, CaptureSession, CaptureSink, MicrophoneCapture, MicrophoneError,
        PreparedCapture,
    };

    pub(super) struct NativeMicrophone;

    struct Prepared {
        device: Device,
        config: StreamConfig,
        sample_format: SampleFormat,
    }

    impl MicrophoneCapture for NativeMicrophone {
        fn prepare(&self) -> Result<Box<dyn PreparedCapture>, MicrophoneError> {
            #[cfg(target_os = "android")]
            super::android_record_audio_permission()?;
            let device = cpal::default_host()
                .default_input_device()
                .ok_or(MicrophoneError::NoInputDevice)?;
            let config = device.default_input_config().map_err(|error| {
                tracing::warn!(%error, "the microphone format could not be selected");
                MicrophoneError::Failed
            })?;
            Ok(Box::new(Prepared {
                device,
                sample_format: config.sample_format(),
                config: config.into(),
            }))
        }
    }

    struct Session {
        stop: Option<mpsc::Sender<()>>,
        worker: Option<JoinHandle<Result<(), MicrophoneError>>>,
    }

    impl Session {
        fn finish(&mut self) -> Result<(), MicrophoneError> {
            if let Some(stop) = self.stop.take() {
                let _ = stop.send(());
            }
            self.worker.take().map_or(Ok(()), |worker| {
                worker.join().map_err(|_| MicrophoneError::Failed)?
            })
        }
    }

    impl CaptureSession for Session {
        fn stop(mut self: Box<Self>) -> Result<(), MicrophoneError> {
            self.finish()
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            let _ = self.finish();
        }
    }

    impl PreparedCapture for Prepared {
        fn format(&self) -> CaptureFormat {
            CaptureFormat {
                sample_rate_hz: self.config.sample_rate.0,
                channels: self.config.channels,
            }
        }

        fn start(
            self: Box<Self>,
            sink: Arc<dyn CaptureSink>,
        ) -> Result<Box<dyn CaptureSession>, MicrophoneError> {
            let (ready_tx, ready_rx) = mpsc::sync_channel(1);
            let (stop_tx, stop_rx) = mpsc::channel();
            let worker = std::thread::Builder::new()
                .name("microphone-capture".into())
                .spawn(move || {
                    let failed = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let build = build_stream(&self, sink, Arc::clone(&failed));
                    let stream = match build {
                        Ok(stream) => stream,
                        Err(error) => {
                            let _ = ready_tx.send(Err(error));
                            return Err(error);
                        }
                    };
                    if let Err(error) = stream.play() {
                        tracing::warn!(%error, "the microphone could not start");
                        let _ = ready_tx.send(Err(MicrophoneError::Failed));
                        return Err(MicrophoneError::Failed);
                    }
                    let _ = ready_tx.send(Ok(()));
                    let _ = stop_rx.recv();
                    drop(stream);
                    if failed.load(std::sync::atomic::Ordering::Acquire) {
                        Err(MicrophoneError::Failed)
                    } else {
                        Ok(())
                    }
                })
                .map_err(|_| MicrophoneError::Failed)?;
            let mut session = Session {
                stop: Some(stop_tx),
                worker: Some(worker),
            };
            match ready_rx.recv() {
                Ok(Ok(())) => Ok(Box::new(session)),
                _ => {
                    let _ = session.finish();
                    Err(MicrophoneError::Failed)
                }
            }
        }
    }

    fn build_stream(
        prepared: &Prepared,
        sink: Arc<dyn CaptureSink>,
        failed: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<cpal::Stream, MicrophoneError> {
        let on_error = move |error| {
            tracing::warn!(%error, "the microphone stream failed");
            failed.store(true, std::sync::atomic::Ordering::Release);
        };
        let built = match prepared.sample_format {
            SampleFormat::F32 => prepared.device.build_input_stream(
                &prepared.config,
                move |data: &[f32], _| sink.push(data),
                on_error,
                None,
            ),
            SampleFormat::I16 => prepared.device.build_input_stream(
                &prepared.config,
                move |data: &[i16], _| {
                    let samples = data
                        .iter()
                        .map(|value| f32::from(*value) / 32768.0)
                        .collect::<Vec<_>>();
                    sink.push(&samples);
                },
                on_error,
                None,
            ),
            SampleFormat::U16 => prepared.device.build_input_stream(
                &prepared.config,
                move |data: &[u16], _| {
                    let samples = data
                        .iter()
                        .map(|value| (f32::from(*value) - 32768.0) / 32768.0)
                        .collect::<Vec<_>>();
                    sink.push(&samples);
                },
                on_error,
                None,
            ),
            _ => return Err(MicrophoneError::Unsupported),
        };
        built.map_err(|error| {
            tracing::warn!(%error, "the microphone stream could not be opened");
            MicrophoneError::Failed
        })
    }
}

#[cfg(target_os = "android")]
fn android_record_audio_permission() -> Result<(), lettuce_app::MicrophoneError> {
    use jni::objects::{JObject, JValue};
    use lettuce_app::MicrophoneError;
    let context = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(context.vm().cast()) }
        .map_err(|_| MicrophoneError::Failed)?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|_| MicrophoneError::Failed)?;
    let activity = unsafe { JObject::from_raw(context.context().cast()) };
    let permission = env
        .new_string("android.permission.RECORD_AUDIO")
        .map_err(|_| MicrophoneError::Failed)?;
    let granted = env
        .call_method(
            &activity,
            "checkSelfPermission",
            "(Ljava/lang/String;)I",
            &[JValue::Object(permission.as_ref())],
        )
        .and_then(|value| value.i())
        .map_err(|_| MicrophoneError::Failed)?;
    if granted == 0 {
        Ok(())
    } else {
        Err(MicrophoneError::PermissionDenied)
    }
}
