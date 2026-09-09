use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

const MAX_ESPEAK_INPUT_BYTES: usize = 64 * 1024;
const MAX_ESPEAK_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct EspeakNgProcess {
    executable: PathBuf,
    data_directory: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EspeakNgError {
    #[error("eSpeak NG configuration is invalid")]
    InvalidConfiguration,
    #[error("eSpeak NG is unavailable")]
    Unavailable,
    #[error("eSpeak NG execution failed")]
    Execution,
    #[error("eSpeak NG output is invalid")]
    InvalidOutput,
}

pub trait EspeakPhonemizer: Send + Sync {
    fn phonemize(&self, input: &str, language: &str) -> Result<String, EspeakNgError>;
}

impl EspeakNgProcess {
    #[must_use]
    pub fn from_path() -> Self {
        Self {
            executable: PathBuf::from("espeak-ng"),
            data_directory: None,
        }
    }

    pub fn with_managed_paths(
        executable: impl AsRef<Path>,
        data_directory: Option<impl AsRef<Path>>,
    ) -> Result<Self, EspeakNgError> {
        let executable = executable.as_ref();
        if !executable.is_absolute() || !executable.is_file() {
            return Err(EspeakNgError::InvalidConfiguration);
        }
        let data_directory = data_directory
            .map(|path| path.as_ref().to_path_buf())
            .transpose_directory()?;
        Ok(Self {
            executable: executable.to_path_buf(),
            data_directory,
        })
    }

    fn execute(&self, input: &str, language: &str) -> Result<String, EspeakNgError> {
        if input.len() > MAX_ESPEAK_INPUT_BYTES || input.contains('\0') || !valid_language(language)
        {
            return Err(EspeakNgError::InvalidConfiguration);
        }
        let mut command = Command::new(&self.executable);
        command.args(["--ipa", "--stdin", "-q", "-v", language]);
        if let Some(directory) = &self.data_directory {
            command.arg("--path").arg(directory);
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        #[cfg(target_os = "linux")]
        if self.executable.is_absolute()
            && let Some(directory) = self.executable.parent()
        {
            command.env("LD_LIBRARY_PATH", directory);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    EspeakNgError::Unavailable
                } else {
                    EspeakNgError::Execution
                }
            })?;
        let stdout = child.stdout.take().ok_or(EspeakNgError::Execution)?;
        let stderr = child.stderr.take().ok_or(EspeakNgError::Execution)?;
        let stdout_reader = thread::spawn(move || read_bounded(stdout, MAX_ESPEAK_OUTPUT_BYTES));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_ESPEAK_OUTPUT_BYTES));
        let mut payload = input.as_bytes().to_vec();
        if !payload.ends_with(b"\n") {
            payload.push(b'\n');
        }
        if child
            .stdin
            .take()
            .ok_or(EspeakNgError::Execution)?
            .write_all(&payload)
            .is_err()
        {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(EspeakNgError::Execution);
        }
        let status = child.wait().map_err(|_| EspeakNgError::Execution)?;
        let (stdout, stdout_exceeded) = stdout_reader
            .join()
            .map_err(|_| EspeakNgError::Execution)?
            .map_err(|_| EspeakNgError::Execution)?;
        let (_, stderr_exceeded) = stderr_reader
            .join()
            .map_err(|_| EspeakNgError::Execution)?
            .map_err(|_| EspeakNgError::Execution)?;
        if !status.success() || stderr_exceeded {
            return Err(EspeakNgError::Execution);
        }
        if stdout_exceeded {
            return Err(EspeakNgError::InvalidOutput);
        }
        String::from_utf8(stdout).map_err(|_| EspeakNgError::InvalidOutput)
    }
}

fn read_bounded(
    mut reader: impl Read,
    limit: usize,
) -> Result<(Vec<u8>, bool), std::io::Error> {
    let mut retained = Vec::with_capacity(limit.min(8192));
    let mut exceeded = false;
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok((retained, exceeded));
        }
        let remaining = limit.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..count.min(remaining)]);
        exceeded |= count > remaining;
    }
}

impl EspeakPhonemizer for EspeakNgProcess {
    fn phonemize(&self, input: &str, language: &str) -> Result<String, EspeakNgError> {
        self.execute(input, language)
    }
}

trait OptionalDirectory {
    fn transpose_directory(self) -> Result<Option<PathBuf>, EspeakNgError>;
}

impl OptionalDirectory for Option<PathBuf> {
    fn transpose_directory(self) -> Result<Option<PathBuf>, EspeakNgError> {
        if self.as_ref().is_some_and(|path| !path.is_dir()) {
            return Err(EspeakNgError::InvalidConfiguration);
        }
        Ok(self)
    }
}

fn valid_language(language: &str) -> bool {
    !language.is_empty()
        && language.len() <= 16
        && language
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn rejects_unmanaged_paths_and_unsafe_language_values() {
        assert!(matches!(
            EspeakNgProcess::with_managed_paths("relative/espeak-ng", None::<&Path>),
            Err(EspeakNgError::InvalidConfiguration)
        ));
        assert_eq!(
            EspeakPhonemizer::phonemize(&EspeakNgProcess::from_path(), "hello", "en-US\0--help"),
            Err(EspeakNgError::InvalidConfiguration)
        );
    }

    #[test]
    fn drains_output_while_retaining_only_the_limit() {
        let input = vec![7_u8; 16_385];
        let (output, exceeded) = read_bounded(Cursor::new(input), 16_384).expect("read output");

        assert_eq!(output.len(), 16_384);
        assert!(exceeded);
    }
}
