use crate::espeak::{EspeakNgError, MAX_ESPEAK_OUTPUT_BYTES};

/// The clauses `espeak_TextToPhonemes` returns, in the shape the command line
/// prints them: each clause on its own line.
#[derive(Debug, Default)]
pub(crate) struct ClauseOutput {
    text: String,
}

impl ClauseOutput {
    pub(crate) fn push(&mut self, clause: &str) -> Result<(), EspeakNgError> {
        if self.text.len() + clause.len() + 1 > MAX_ESPEAK_OUTPUT_BYTES {
            return Err(EspeakNgError::InvalidOutput {
                reason: "clause output exceeds the byte limit",
            });
        }
        self.text.push_str(clause);
        self.text.push('\n');
        Ok(())
    }

    pub(crate) fn finish(self) -> String {
        self.text
    }
}

#[cfg(target_os = "android")]
pub use android::EspeakNgLibrary;

#[cfg(target_os = "android")]
mod android {
    use std::{
        ffi::{CStr, CString, c_char, c_int, c_void},
        os::unix::ffi::OsStrExt,
        path::{Path, PathBuf},
        sync::{Mutex, PoisonError},
    };

    use libloading::Library;

    use super::ClauseOutput;
    use crate::espeak::{EspeakNgError, EspeakPhonemizer, validate_request};

    const LIBRARY_NAME: &str = "libttsespeak.so";
    const AUDIO_OUTPUT_SYNCHRONOUS: c_int = 2;
    const ESPEAK_INITIALIZE_PHONEME_IPA: c_int = 0x0002;
    const ESPEAK_INITIALIZE_DONT_EXIT: c_int = 0x8000;
    const ESPEAK_CHARS_UTF8: c_int = 1;
    const ESPEAK_PHONEMES_IPA: c_int = 0x02;
    const EE_OK: c_int = 0;

    type Initialize = unsafe extern "C" fn(c_int, c_int, *const c_char, c_int) -> c_int;
    type SetVoiceByName = unsafe extern "C" fn(*const c_char) -> c_int;
    type TextToPhonemes = unsafe extern "C" fn(*mut *const c_void, c_int, c_int) -> *const c_char;

    static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

    /// eSpeak NG loaded from the app's `libttsespeak.so`, reading the
    /// `espeak-ng-data` folder at `data_directory`. eSpeak keeps one global
    /// state, so the first successful call initializes it for the process
    /// and every call is serialized; a call with another data folder after
    /// that is refused.
    #[derive(Debug, Clone)]
    pub struct EspeakNgLibrary {
        data_directory: PathBuf,
    }

    impl EspeakNgLibrary {
        #[must_use]
        pub fn new(data_directory: impl Into<PathBuf>) -> Self {
            Self {
                data_directory: data_directory.into(),
            }
        }
    }

    impl EspeakPhonemizer for EspeakNgLibrary {
        fn phonemize(&self, input: &str, language: &str) -> Result<String, EspeakNgError> {
            validate_request(input, language)?;
            let mut engine = ENGINE.lock().unwrap_or_else(PoisonError::into_inner);
            if engine
                .as_ref()
                .is_some_and(|engine| engine.data_directory != self.data_directory)
            {
                return Err(EspeakNgError::InvalidConfiguration);
            }
            if engine.is_none() {
                *engine = Some(Engine::load(&self.data_directory)?);
            }
            engine
                .as_mut()
                .ok_or(EspeakNgError::Unavailable)?
                .phonemize(input, language)
        }
    }

    struct Engine {
        _library: Library,
        data_directory: PathBuf,
        set_voice: SetVoiceByName,
        text_to_phonemes: TextToPhonemes,
    }

    impl Engine {
        fn load(data_directory: &Path) -> Result<Self, EspeakNgError> {
            if !data_directory.is_dir() {
                return Err(EspeakNgError::Unavailable);
            }
            let data_root = data_directory
                .parent()
                .ok_or(EspeakNgError::InvalidConfiguration)?;
            let data_root = CString::new(data_root.as_os_str().as_bytes())
                .map_err(|_| EspeakNgError::InvalidConfiguration)?;
            let library =
                unsafe { Library::new(LIBRARY_NAME) }.map_err(|_| EspeakNgError::Unavailable)?;
            let initialize = *unsafe { library.get::<Initialize>(b"espeak_Initialize\0") }
                .map_err(|_| EspeakNgError::Unavailable)?;
            let set_voice = *unsafe { library.get::<SetVoiceByName>(b"espeak_SetVoiceByName\0") }
                .map_err(|_| EspeakNgError::Unavailable)?;
            let text_to_phonemes =
                *unsafe { library.get::<TextToPhonemes>(b"espeak_TextToPhonemes\0") }
                    .map_err(|_| EspeakNgError::Unavailable)?;
            let status = unsafe {
                initialize(
                    AUDIO_OUTPUT_SYNCHRONOUS,
                    0,
                    data_root.as_ptr(),
                    ESPEAK_INITIALIZE_PHONEME_IPA | ESPEAK_INITIALIZE_DONT_EXIT,
                )
            };
            if status < 0 {
                return Err(EspeakNgError::Unavailable);
            }
            Ok(Self {
                _library: library,
                data_directory: data_directory.to_path_buf(),
                set_voice,
                text_to_phonemes,
            })
        }

        fn phonemize(&mut self, input: &str, language: &str) -> Result<String, EspeakNgError> {
            let language =
                CString::new(language).map_err(|_| EspeakNgError::InvalidConfiguration)?;
            if unsafe { (self.set_voice)(language.as_ptr()) } != EE_OK {
                return Err(EspeakNgError::Execution);
            }
            let text = CString::new(input).map_err(|_| EspeakNgError::InvalidConfiguration)?;
            let mut cursor = text.as_ptr().cast::<c_void>();
            let mut output = ClauseOutput::default();
            while !cursor.is_null() {
                let clause = unsafe {
                    (self.text_to_phonemes)(&mut cursor, ESPEAK_CHARS_UTF8, ESPEAK_PHONEMES_IPA)
                };
                if clause.is_null() {
                    break;
                }
                let clause = unsafe { CStr::from_ptr(clause) }
                    .to_str()
                    .map_err(|_| EspeakNgError::InvalidOutput)?;
                output.push(clause)?;
            }
            Ok(output.finish())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clauses_become_one_line_each_like_the_command_line() {
        let mut output = ClauseOutput::default();
        for clause in ["həlˈoʊ", "", "wˈɜːld"] {
            output.push(clause).expect("clause");
        }
        assert_eq!(output.finish(), "həlˈoʊ\n\nwˈɜːld\n");
    }

    #[test]
    fn clause_output_is_bounded() {
        let mut output = ClauseOutput::default();
        let clause = "a".repeat(MAX_ESPEAK_OUTPUT_BYTES - 1);
        output.push(&clause).expect("clause at the limit");
        assert!(matches!(
            output.push(""),
            Err(EspeakNgError::InvalidOutput { .. })
        ));
    }
}
