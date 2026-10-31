use std::sync::Arc;

use lettuce_app::MicrophoneCapture;

pub(crate) fn capture<R: tauri::Runtime>(_handle: &tauri::AppHandle<R>) -> Option<Arc<dyn MicrophoneCapture>> {
    #[cfg(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos",
        target_os = "android"
    ))]
    {
        Some(Arc::new(desktop::NativeMicrophone {
            #[cfg(target_os = "android")]
            permission: {
                let handle = _handle.clone();
                Arc::new(move || crate::host::request_microphone_permission(&handle))
            },
        }))
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

    pub(super) struct NativeMicrophone {
        #[cfg(target_os = "android")]
        pub(super) permission: Arc<dyn Fn() -> Result<(), MicrophoneError> + Send + Sync>,
    }

    struct Prepared {
        device: Device,
        config: StreamConfig,
        sample_format: SampleFormat,
    }

    impl MicrophoneCapture for NativeMicrophone {
        fn devices(&self) -> Result<Vec<lettuce_contracts::MicrophoneDevice>, MicrophoneError> {
            let mut names = std::collections::BTreeMap::<String, usize>::new();
            for device in cpal::default_host().input_devices().map_err(|_| MicrophoneError::Failed)? {
                let name = device.name().map_err(|_| MicrophoneError::Failed)?;
                *names.entry(name).or_default() += 1;
            }
            Ok(names.into_iter().map(|(name, count)| lettuce_contracts::MicrophoneDevice {
                id: format!("input:{name}"), label: name, selectable: count == 1,
            }).collect())
        }

        fn prepare(&self) -> Result<Box<dyn PreparedCapture>, MicrophoneError> {
            self.prepare_device(None)
        }

        fn prepare_device(&self, id: Option<&str>) -> Result<Box<dyn PreparedCapture>, MicrophoneError> {
            #[cfg(target_os = "android")]
            (self.permission)()?;
            let host = cpal::default_host();
            let device = if let Some(id) = id {
                let id = id.strip_prefix("input:").ok_or(MicrophoneError::NoInputDevice)?;
                let mut matched = None;
                for device in host.input_devices().map_err(|_| MicrophoneError::Failed)? {
                    if device.name().map_err(|_| MicrophoneError::Failed)? == id {
                        if matched.is_some() {
                            return Err(MicrophoneError::NoInputDevice);
                        }
                        matched = Some(device);
                    }
                }
                matched.ok_or(MicrophoneError::NoInputDevice)?
            } else {
                host.default_input_device().ok_or(MicrophoneError::NoInputDevice)?
            };
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
