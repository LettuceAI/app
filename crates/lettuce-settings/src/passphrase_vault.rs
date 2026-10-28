use std::{collections::BTreeMap, fmt, sync::Mutex};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

const VAULT_VERSION: u32 = 1;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_LANES: u32 = 4;
const MAX_VAULT_BYTES: usize = 64 * 1024 * 1024;
const MAX_PASSPHRASE_BYTES: usize = 1024;

/// Storage for the vault's single ciphertext file.
///
/// `write` must replace the previous contents atomically, so a crash leaves
/// either the old or the new vault and never a partial one. `create_new` must
/// publish the first vault atomically and fail with
/// [`std::io::ErrorKind::AlreadyExists`] when a vault file already exists, so
/// two concurrent creates can never overwrite each other.
pub trait VaultFile: Send + Sync {
    fn read(&self) -> std::io::Result<Option<Vec<u8>>>;
    fn write(&self, bytes: &[u8]) -> std::io::Result<()>;
    fn create_new(&self, bytes: &[u8]) -> std::io::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PassphraseVaultError {
    #[error("a passphrase vault already exists")]
    AlreadyExists,
    #[error("no passphrase vault exists")]
    Missing,
    #[error("the passphrase is empty, too long or contains control characters: {reason}")]
    InvalidPassphrase { reason: &'static str },
    #[error("the passphrase does not unlock the vault")]
    WrongPassphrase,
    #[error("the passphrase vault is corrupt: {reason}")]
    Corrupt { reason: &'static str },
    #[error("the passphrase vault could not be read or written: {reason}")]
    Io { reason: &'static str },
}

/// A password-protected local vault for devices without an OS credential
/// store: an Argon2id key derived from the passphrase seals one ciphertext
/// file with XChaCha20-Poly1305 under a fresh nonce on every write.
///
/// Only the derived key is kept, and only for the unlocked session; the
/// passphrase itself is never stored.
pub struct PassphraseVault {
    file: Box<dyn VaultFile>,
    kdf: VaultKdf,
    key: Zeroizing<[u8; 32]>,
    lock: Mutex<()>,
}

impl fmt::Debug for PassphraseVault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PassphraseVault")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultKdf {
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
    salt: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SealedVault {
    version: u32,
    kdf: VaultKdf,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
struct Entries(BTreeMap<String, Vec<u8>>);

impl Drop for Entries {
    fn drop(&mut self) {
        for value in self.0.values_mut() {
            value.zeroize();
        }
    }
}

impl PassphraseVault {
    /// Creates an empty vault; an existing vault is never overwritten.
    pub fn create(
        file: Box<dyn VaultFile>,
        passphrase: &str,
    ) -> Result<Self, PassphraseVaultError> {
        validate_passphrase(passphrase)?;
        let mut salt = vec![0_u8; SALT_BYTES];
        OsRng.fill_bytes(&mut salt);
        let kdf = VaultKdf {
            memory_kib: ARGON2_MEMORY_KIB,
            iterations: ARGON2_ITERATIONS,
            lanes: ARGON2_LANES,
            salt,
        };
        let key = derive_key(passphrase, &kdf)?;
        let vault = Self {
            file,
            kdf,
            key,
            lock: Mutex::new(()),
        };
        let bytes = vault.sealed_bytes(&Entries::default())?;
        match vault.file.create_new(&bytes) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(PassphraseVaultError::AlreadyExists);
            }
            Err(error) => {
                tracing::warn!(%error, "creating the passphrase vault file failed");
                return Err(PassphraseVaultError::Io {
                    reason: "creating the vault file failed",
                });
            }
        }
        Ok(vault)
    }

    pub fn unlock(
        file: Box<dyn VaultFile>,
        passphrase: &str,
    ) -> Result<Self, PassphraseVaultError> {
        validate_passphrase(passphrase)?;
        let sealed = read_sealed(file.as_ref())?.ok_or(PassphraseVaultError::Missing)?;
        let key = derive_key(passphrase, &sealed.kdf)?;
        let vault = Self {
            file,
            kdf: sealed.kdf.clone(),
            key,
            lock: Mutex::new(()),
        };
        vault.open(&sealed)?;
        Ok(vault)
    }

    pub(crate) fn get(
        &self,
        name: &str,
    ) -> Result<Option<Zeroizing<Vec<u8>>>, PassphraseVaultError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| PassphraseVaultError::Corrupt {
                reason: "vault lock is poisoned",
            })?;
        let mut entries = self.entries()?;
        Ok(entries.0.remove(name).map(Zeroizing::new))
    }

    pub(crate) fn set(&self, name: &str, value: &[u8]) -> Result<(), PassphraseVaultError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| PassphraseVaultError::Corrupt {
                reason: "vault lock is poisoned",
            })?;
        let mut entries = self.entries()?;
        if let Some(mut replaced) = entries.0.insert(name.to_owned(), value.to_vec()) {
            replaced.zeroize();
        }
        self.seal(&entries)
    }

    pub(crate) fn remove(&self, name: &str) -> Result<(), PassphraseVaultError> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| PassphraseVaultError::Corrupt {
                reason: "vault lock is poisoned",
            })?;
        let mut entries = self.entries()?;
        if entries.0.remove(name).is_some() {
            self.seal(&entries)?;
        }
        Ok(())
    }

    fn entries(&self) -> Result<Entries, PassphraseVaultError> {
        let sealed = read_sealed(self.file.as_ref())?.ok_or(PassphraseVaultError::Missing)?;
        if sealed.kdf != self.kdf {
            return Err(PassphraseVaultError::Corrupt {
                reason: "vault key derivation settings differ from the unlocked settings",
            });
        }
        self.open(&sealed)
    }

    fn open(&self, sealed: &SealedVault) -> Result<Entries, PassphraseVaultError> {
        if sealed.nonce.len() != NONCE_BYTES {
            return Err(PassphraseVaultError::Corrupt {
                reason: "vault nonce has the wrong length",
            });
        }
        let aad = associated_data(&sealed.kdf)?;
        let plaintext = Zeroizing::new(
            XChaCha20Poly1305::new(self.key.as_ref().into())
                .decrypt(
                    XNonce::from_slice(&sealed.nonce),
                    Payload {
                        msg: &sealed.ciphertext,
                        aad: &aad,
                    },
                )
                .map_err(|_| PassphraseVaultError::WrongPassphrase)?,
        );
        serde_json::from_slice::<Entries>(&plaintext).map_err(|error| {
            tracing::warn!(%error, "passphrase vault entries could not be decoded");
            PassphraseVaultError::Corrupt {
                reason: "vault entries are not valid JSON",
            }
        })
    }

    fn seal(&self, entries: &Entries) -> Result<(), PassphraseVaultError> {
        let bytes = self.sealed_bytes(entries)?;
        self.file.write(&bytes).map_err(|error| {
            tracing::warn!(%error, "writing the passphrase vault file failed");
            PassphraseVaultError::Io {
                reason: "writing the vault file failed",
            }
        })
    }

    fn sealed_bytes(&self, entries: &Entries) -> Result<Vec<u8>, PassphraseVaultError> {
        let plaintext = Zeroizing::new(serde_json::to_vec(entries).map_err(|error| {
            tracing::warn!(%error, "passphrase vault entries could not be serialized");
            PassphraseVaultError::Corrupt {
                reason: "vault entries could not be serialized",
            }
        })?);
        let mut nonce = vec![0_u8; NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);
        let aad = associated_data(&self.kdf)?;
        let ciphertext = XChaCha20Poly1305::new(self.key.as_ref().into())
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| PassphraseVaultError::Corrupt {
                reason: "vault encryption failed",
            })?;
        let bytes = serde_json::to_vec(&SealedVault {
            version: VAULT_VERSION,
            kdf: self.kdf.clone(),
            nonce,
            ciphertext,
        })
        .map_err(|error| {
            tracing::warn!(%error, "the sealed passphrase vault could not be serialized");
            PassphraseVaultError::Corrupt {
                reason: "sealed vault could not be serialized",
            }
        })?;
        if bytes.len() > MAX_VAULT_BYTES {
            return Err(PassphraseVaultError::Corrupt {
                reason: "sealed vault exceeds the size limit",
            });
        }
        Ok(bytes)
    }
}

fn read_sealed(file: &dyn VaultFile) -> Result<Option<SealedVault>, PassphraseVaultError> {
    let Some(bytes) = file.read().map_err(|error| {
        tracing::warn!(%error, "reading the passphrase vault file failed");
        PassphraseVaultError::Io {
            reason: "reading the vault file failed",
        }
    })?
    else {
        return Ok(None);
    };
    if bytes.len() > MAX_VAULT_BYTES {
        return Err(PassphraseVaultError::Corrupt {
            reason: "vault file exceeds the size limit",
        });
    }
    let sealed: SealedVault = serde_json::from_slice(&bytes).map_err(|error| {
        tracing::warn!(%error, "the passphrase vault file could not be decoded");
        PassphraseVaultError::Corrupt {
            reason: "vault file is not valid JSON",
        }
    })?;
    if sealed.version != VAULT_VERSION {
        return Err(PassphraseVaultError::Corrupt {
            reason: "vault version is not supported",
        });
    }
    if sealed.kdf.salt.len() != SALT_BYTES {
        return Err(PassphraseVaultError::Corrupt {
            reason: "vault salt has the wrong length",
        });
    }
    if !(8 * 1024..=1024 * 1024).contains(&sealed.kdf.memory_kib) {
        return Err(PassphraseVaultError::Corrupt {
            reason: "vault key derivation memory is out of range",
        });
    }
    if !(1..=16).contains(&sealed.kdf.iterations) {
        return Err(PassphraseVaultError::Corrupt {
            reason: "vault key derivation iterations are out of range",
        });
    }
    if !(1..=16).contains(&sealed.kdf.lanes) {
        return Err(PassphraseVaultError::Corrupt {
            reason: "vault key derivation lanes are out of range",
        });
    }
    Ok(Some(sealed))
}

fn associated_data(kdf: &VaultKdf) -> Result<Vec<u8>, PassphraseVaultError> {
    serde_json::to_vec(&(VAULT_VERSION, kdf)).map_err(|error| {
        tracing::warn!(%error, "passphrase vault associated data could not be serialized");
        PassphraseVaultError::Corrupt {
            reason: "vault associated data could not be serialized",
        }
    })
}

fn validate_passphrase(passphrase: &str) -> Result<(), PassphraseVaultError> {
    if passphrase.is_empty() {
        Err(PassphraseVaultError::InvalidPassphrase {
            reason: "passphrase is empty",
        })
    } else if passphrase.len() > MAX_PASSPHRASE_BYTES {
        Err(PassphraseVaultError::InvalidPassphrase {
            reason: "passphrase is too long",
        })
    } else if passphrase.chars().any(char::is_control) {
        Err(PassphraseVaultError::InvalidPassphrase {
            reason: "passphrase contains control characters",
        })
    } else {
        Ok(())
    }
}

fn derive_key(
    passphrase: &str,
    kdf: &VaultKdf,
) -> Result<Zeroizing<[u8; 32]>, PassphraseVaultError> {
    let params =
        Params::new(kdf.memory_kib, kdf.iterations, kdf.lanes, Some(32)).map_err(|error| {
            tracing::warn!(%error, "passphrase vault key derivation parameters are invalid");
            PassphraseVaultError::Corrupt {
                reason: "key derivation parameters are invalid",
            }
        })?;
    let mut key = Zeroizing::new([0_u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), &kdf.salt, key.as_mut())
        .map_err(|error| {
            tracing::warn!(%error, "passphrase vault key derivation failed");
            PassphraseVaultError::Corrupt {
                reason: "key derivation failed",
            }
        })?;
    Ok(key)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use super::*;

    #[derive(Default, Clone)]
    pub(crate) struct MemoryFile(pub(crate) Arc<Mutex<Option<Vec<u8>>>>);

    impl VaultFile for MemoryFile {
        fn read(&self) -> std::io::Result<Option<Vec<u8>>> {
            Ok(self.0.lock().expect("file lock").clone())
        }

        fn write(&self, bytes: &[u8]) -> std::io::Result<()> {
            *self.0.lock().expect("file lock") = Some(bytes.to_vec());
            Ok(())
        }

        fn create_new(&self, bytes: &[u8]) -> std::io::Result<()> {
            let mut file = self.0.lock().expect("file lock");
            if file.is_some() {
                return Err(std::io::ErrorKind::AlreadyExists.into());
            }
            *file = Some(bytes.to_vec());
            Ok(())
        }
    }

    #[test]
    fn vault_seals_entries_and_reopens_only_with_the_passphrase() {
        let file = MemoryFile::default();
        let vault =
            PassphraseVault::create(Box::new(file.clone()), "correct horse").expect("create");
        vault.set("ref-1", b"sk-canary").expect("set");
        let first = file.0.lock().expect("file").clone().expect("sealed");
        assert!(!String::from_utf8_lossy(&first).contains("sk-canary"));
        vault.set("ref-2", b"other").expect("set");
        let second: SealedVault =
            serde_json::from_slice(&file.0.lock().expect("file").clone().expect("sealed"))
                .expect("sealed format");
        let first: SealedVault = serde_json::from_slice(&first).expect("sealed format");
        assert_ne!(first.nonce, second.nonce);
        assert_eq!(
            PassphraseVault::create(Box::new(file.clone()), "correct horse").map(|_| ()),
            Err(PassphraseVaultError::AlreadyExists)
        );
        assert_eq!(
            PassphraseVault::unlock(Box::new(file.clone()), "wrong horse").map(|_| ()),
            Err(PassphraseVaultError::WrongPassphrase)
        );
        let reopened =
            PassphraseVault::unlock(Box::new(file.clone()), "correct horse").expect("unlock");
        assert_eq!(
            reopened
                .get("ref-1")
                .expect("get")
                .as_deref()
                .map(Vec::as_slice),
            Some(&b"sk-canary"[..])
        );
        reopened.remove("ref-1").expect("remove");
        assert!(reopened.get("ref-1").expect("get").is_none());
        assert_eq!(
            PassphraseVault::unlock(Box::new(MemoryFile::default()), "correct horse").map(|_| ()),
            Err(PassphraseVaultError::Missing)
        );
    }
}
