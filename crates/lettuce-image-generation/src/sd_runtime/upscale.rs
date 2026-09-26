//! The upscaler library and one-shot upscaling through `sd-cli` (the server
//! stays as it is).

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use super::layout::{UPSCALER_EXTENSIONS, cli_executable_name};
use super::server::LocalDiffusionEngine;
use crate::diffusion_catalog;

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

    /// Whether an upscale can run, checked before the image is read.
    pub fn check_upscale_ready(&self) -> Result<(), String> {
        self.upscale_target().map(|_| ())
    }

    fn upscale_target(
        &self,
    ) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf), String> {
        let inventory = self.upscaler_inventory();
        let model = if inventory.recommended_installed {
            inventory.recommended_filename.clone()
        } else {
            inventory.models.first().cloned().ok_or_else(|| {
                "No upscaler model is installed. Install one in the Local Image Generation settings first."
                    .to_owned()
            })?
        };
        let model_path = self.paths().upscalers.join(&model);
        let active = self.effective_runtime().ok_or_else(|| {
            "No stable-diffusion.cpp engine build is installed. Install an engine in the Local Image Generation settings first."
                .to_owned()
        })?;
        let runtime_dir = self.paths().runtime_root(&active.release, &active.asset);
        let executable = runtime_dir.join(cli_executable_name());
        if !executable.is_file() {
            return Err(format!(
                "The stable-diffusion.cpp command-line tool is missing: {}",
                executable.display()
            ));
        }
        Ok((model_path, runtime_dir, executable))
    }

    /// Upscales one image with the recommended upscaler when installed, else
    /// the first one, using the active engine build's command-line tool.
    pub async fn upscale(&self, image: &[u8]) -> Result<Vec<u8>, String> {
        let (model_path, runtime_dir, executable) = self.upscale_target()?;
        let work_dir = &self.paths().upscale_scratch;
        std::fs::create_dir_all(work_dir)
            .map_err(|error| format!("Failed to prepare the upscale work directory: {error}"))?;
        let job_id = lettuce_types::OperationId::new();
        let input_path = work_dir.join(format!("{job_id}-in.png"));
        let output_path = work_dir.join(format!("{job_id}-out.png"));
        std::fs::write(&input_path, image)
            .map_err(|error| format!("Failed to stage the image to upscale: {error}"))?;
        let cleanup = || {
            std::fs::remove_file(&input_path).ok();
            std::fs::remove_file(&output_path).ok();
        };
        let mut command = Command::new(&executable);
        command
            .current_dir(&runtime_dir)
            .arg("-M")
            .arg("upscale")
            .arg("--upscale-model")
            .arg(&model_path)
            .arg("-i")
            .arg(&input_path)
            .arg("-o")
            .arg(&output_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Err(error) = super::server::library_path_env(
            &mut command,
            &runtime_dir,
            "Failed to configure stable-diffusion.cpp libraries",
        ) {
            cleanup();
            return Err(error);
        }
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                cleanup();
                return Err(format!("Failed to start the upscaler: {error}"));
            }
        };
        let output = match tokio::time::timeout(UPSCALE_TIMEOUT, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(error)) => {
                cleanup();
                return Err(format!("The upscaler failed to run: {error}"));
            }
            Err(_) => {
                cleanup();
                return Err("Upscaling timed out after ten minutes.".to_owned());
            }
        };
        if !output.status.success() || !output_path.is_file() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = stderr
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("no error output")
                .to_owned();
            cleanup();
            return Err(format!("Upscaling failed: {detail}"));
        }
        let upscaled = std::fs::read(&output_path)
            .map_err(|error| format!("Failed to read the upscaled image: {error}"));
        cleanup();
        upscaled
    }
}
