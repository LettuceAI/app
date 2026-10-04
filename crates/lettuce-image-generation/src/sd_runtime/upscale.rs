//! The upscaler library and one-shot upscaling through `sd-cli` (the server
//! stays as it is).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use lettuce_jobs::handle::CancellationToken;

use super::layout::{UPSCALER_EXTENSIONS, cli_executable_name};
use super::server::{LocalDiffusionEngine, no_runtime_installed};
use crate::{ImageError, ImageFailureKind, diffusion_catalog};

const UPSCALE_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpscalerInventory {
    pub models: Vec<String>,
    /// File stems, the names sd-server's hires fix accepts.
    pub hires_upscaler_names: Vec<String>,
    pub recommended_filename: String,
    pub recommended_bytes: u64,
    pub recommended_installed: bool,
}

impl LocalDiffusionEngine {
    fn upscaler_files(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.paths().upscalers) else {
            return Vec::new();
        };
        let mut files = entries
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| {
                Path::new(name)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        UPSCALER_EXTENSIONS
                            .iter()
                            .any(|allowed| extension.eq_ignore_ascii_case(allowed))
                    })
            })
            .collect::<Vec<_>>();
        files.sort();
        files
    }

    #[must_use]
    pub fn upscaler_inventory(&self) -> UpscalerInventory {
        let upscaler = &diffusion_catalog().upscaler;
        let models = self.upscaler_files();
        UpscalerInventory {
            hires_upscaler_names: models
                .iter()
                .filter_map(|name| {
                    Path::new(name)
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(str::to_owned)
                })
                .collect(),
            recommended_installed: models.contains(&upscaler.filename),
            recommended_filename: upscaler.filename.clone(),
            recommended_bytes: upscaler.bytes,
            models,
        }
    }

    /// Removes one upscaler file by its bare name.
    pub fn remove_upscaler(&self, filename: &str) -> Result<UpscalerInventory, String> {
        let name = Path::new(filename)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "Invalid upscaler file name.".to_owned())?;
        if name != filename {
            return Err("Invalid upscaler file name.".to_owned());
        }
        match std::fs::remove_file(self.paths().upscalers.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Failed to remove the upscaler model: {error}")),
        }
        Ok(self.upscaler_inventory())
    }

    /// Removes the scratch files an upscale a crash cut short left behind;
    /// call while no upscale runs.
    pub fn clear_upscale_scratch(&self) {
        let Ok(entries) = std::fs::read_dir(&self.paths().upscale_scratch) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.path().is_file() {
                std::fs::remove_file(entry.path()).ok();
            }
        }
    }

    /// Whether an upscale can run, checked before the image is read.
    pub fn check_upscale_ready(&self) -> Result<(), ImageError> {
        self.upscale_target().map(|_| ())
    }

    fn upscale_target(
        &self,
    ) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf), ImageError> {
        let paths = self.paths();
        let inventory = self.upscaler_inventory();
        let model = if inventory.recommended_installed {
            inventory.recommended_filename.clone()
        } else {
            inventory.models.first().cloned().ok_or_else(|| {
                ImageError::new(
                    ImageFailureKind::UpscalerMissing,
                    "No upscaler model is installed. Install one in the Local Image Generation settings first.",
                )
            })?
        };
        let model_path = paths.upscalers.join(&model);
        let active = self.effective_runtime().ok_or_else(no_runtime_installed)?;
        let runtime_dir = paths.runtime_root(&active.release, &active.asset);
        let executable = runtime_dir.join(cli_executable_name());
        if !executable.is_file() {
            return Err(ImageError::new(
                ImageFailureKind::RuntimeNotInstalled,
                format!(
                    "The stable-diffusion.cpp command-line tool is missing: {}",
                    executable.display()
                ),
            ));
        }
        Ok((model_path, runtime_dir, executable))
    }

    /// Upscales one image with the recommended upscaler when installed, else
    /// the first one, using the active engine build's command-line tool.
    /// Cancelling stops the tool and leaves no scratch file behind.
    pub async fn upscale(
        &self,
        image: &[u8],
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, ImageError> {
        let (model_path, runtime_dir, executable) = self.upscale_target()?;
        let Some(_call) = self.begin_call(cancellation, None).await else {
            return Err(ImageError::cancelled());
        };
        if self.stopping(cancellation) {
            return Err(ImageError::cancelled());
        }
        let work_dir = self.paths().upscale_scratch.clone();
        std::fs::create_dir_all(&work_dir).map_err(|error| {
            ImageError::storage(format!(
                "Failed to prepare the upscale work directory: {error}"
            ))
        })?;
        let job_id = lettuce_types::OperationId::new();
        let scratch = Scratch {
            input: work_dir.join(format!("{job_id}-in.png")),
            output: work_dir.join(format!("{job_id}-out.png")),
        };
        std::fs::write(&scratch.input, image).map_err(|error| {
            ImageError::storage(format!("Failed to stage the image to upscale: {error}"))
        })?;
        let mut command = Command::new(&executable);
        command
            .current_dir(&runtime_dir)
            .arg("-M")
            .arg("upscale")
            .arg("--upscale-model")
            .arg(&model_path)
            .arg("-i")
            .arg(&scratch.input)
            .arg("-o")
            .arg(&scratch.output)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        super::server::library_path_env(
            &mut command,
            &runtime_dir,
            "Failed to configure stable-diffusion.cpp libraries",
        )
        .map_err(|message| ImageError::new(ImageFailureKind::EngineFailed, message))?;
        let mut child = command.spawn().map_err(|error| {
            ImageError::new(
                ImageFailureKind::EngineFailed,
                format!("Failed to start the upscaler: {error}"),
            )
        })?;
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        enum Ended {
            Exited(std::io::Result<std::process::ExitStatus>),
            Cancelled,
            TimedOut,
        }
        let ended = tokio::select! {
            biased;
            () = cancellation.cancelled() => Ended::Cancelled,
            status = child.wait() => Ended::Exited(status),
            () = tokio::time::sleep(UPSCALE_TIMEOUT) => Ended::TimedOut,
        };
        let status = match ended {
            Ended::Exited(status) => status.map_err(|error| {
                ImageError::new(
                    ImageFailureKind::EngineFailed,
                    format!("The upscaler failed to run: {error}"),
                )
            })?,
            Ended::Cancelled => {
                child.kill().await.ok();
                child.wait().await.ok();
                return Err(ImageError::cancelled());
            }
            Ended::TimedOut => {
                child.kill().await.ok();
                child.wait().await.ok();
                return Err(ImageError::new(
                    ImageFailureKind::EngineTimedOut,
                    "Upscaling timed out after ten minutes.",
                ));
            }
        };
        let _ = stdout.await;
        let stderr = stderr.await.unwrap_or_default();
        if !status.success() || !scratch.output.is_file() {
            let detail = String::from_utf8_lossy(&stderr)
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("no error output")
                .to_owned();
            return Err(ImageError::new(
                ImageFailureKind::EngineFailed,
                format!("Upscaling failed: {detail}"),
            ));
        }
        std::fs::read(&scratch.output).map_err(|error| {
            ImageError::storage(format!("Failed to read the upscaled image: {error}"))
        })
    }
}

/// The files one upscale stages, removed when it ends however it ends.
struct Scratch {
    input: std::path::PathBuf,
    output: std::path::PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_file(&self.input).ok();
        std::fs::remove_file(&self.output).ok();
    }
}

fn drain<R>(stream: Option<R>) -> tokio::task::JoinHandle<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            tokio::io::AsyncReadExt::read_to_end(&mut stream, &mut bytes)
                .await
                .ok();
        }
        bytes
    })
}
