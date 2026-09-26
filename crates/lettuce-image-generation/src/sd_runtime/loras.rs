//! LoRA files as sd-server loads them: library-relative paths, with FLUX.2
//! Klein tensor aliases rewritten into a compatibility cache.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use lettuce_models::StableDiffusionLora;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::payload::EngineLora;

pub const LORA_COMPAT_CACHE_DIR: &str = ".sdcpp-compat";
const LORA_COMPAT_CACHE_VERSION: &str = "flux-klein-v1";
const MAX_SAFETENSORS_HEADER_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoraPathError {
    #[error("LoRA must be inside the LettuceAI LoRA library: {0}")]
    OutsideLibrary(String),
    #[error("Invalid LoRA library path: {0}")]
    InvalidPath(String),
    #[error("LoRA is not installed in the LettuceAI library: {0}")]
    NotInstalled(String),
    #[error("{0}")]
    Compatibility(String),
}

/// The file size and modification time (seconds) LoRA files are
/// fingerprinted by.
pub fn lora_file_fingerprint(path: &Path) -> Result<(u64, u64), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("Failed to inspect the LoRA file: {error}"))?;
    let modified_at = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |value| value.as_secs());
    Ok((metadata.len(), modified_at))
}

/// Resolves each LoRA below the library root, rewriting FLUX.2 Klein aliases
/// when the profile is a Klein model.
pub fn normalize_loras(
    root: &Path,
    loras: &[StableDiffusionLora],
    profile_id: Option<&str>,
) -> Result<Vec<EngineLora>, LoraPathError> {
    let rewrite_flux_klein_aliases = profile_id.is_some_and(|id| id.starts_with("flux-2-klein-"));
    loras
        .iter()
        .map(|lora| {
            let requested = PathBuf::from(&lora.path);
            let relative = if requested.is_absolute() {
                requested
                    .strip_prefix(root)
                    .map_err(|_| LoraPathError::OutsideLibrary(root.display().to_string()))?
            } else {
                requested.as_path()
            };
            if relative.as_os_str().is_empty()
                || relative.components().any(|component| {
                    matches!(
                        component,
                        Component::ParentDir | Component::RootDir | Component::Prefix(_)
                    )
                })
            {
                return Err(LoraPathError::InvalidPath(lora.path.clone()));
            }
            let full_path = root.join(relative);
            if !full_path.is_file() {
                return Err(LoraPathError::NotInstalled(full_path.display().to_string()));
            }
            let runtime_relative = if rewrite_flux_klein_aliases {
                prepare_sdcpp_compatible_lora(root, relative, &full_path)
                    .map_err(LoraPathError::Compatibility)?
            } else {
                relative.to_path_buf()
            };
            if runtime_relative != relative {
                tracing::info!(
                    component = "sdcpp",
                    lora = %relative.display(),
                    "using sd.cpp-compatible tensor names for LoRA"
                );
            }
            Ok(EngineLora {
                path: runtime_relative
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
                multiplier: lora.multiplier,
                is_high_noise: lora.is_high_noise,
            })
        })
        .collect()
}

#[must_use]
pub fn rewrite_flux_klein_lora_tensor_name(name: &str) -> Option<String> {
    let rewritten = if name.starts_with("single_transformer_blocks.") {
        name.replace(".attn.to_qkv_mlp_proj.", ".attn.to_q.")
            .replace(".attn.to_out.", ".proj_out.")
    } else if name.starts_with("transformer_blocks.") {
        name.replace(".ff.linear_in.", ".ff.net.0.proj.")
            .replace(".ff.linear_out.", ".ff.net.2.")
            .replace(".ff_context.linear_in.", ".ff_context.net.0.proj.")
            .replace(".ff_context.linear_out.", ".ff_context.net.2.")
    } else {
        name.to_owned()
    };
    (rewritten != name).then_some(rewritten)
}

/// Rewrites tensor keys, keeping `__metadata__`; nothing is rewritten when a
/// rewritten name would collide with an existing key.
#[must_use]
pub fn rewrite_flux_klein_lora_header(header: Map<String, Value>) -> (Map<String, Value>, usize) {
    let has_collision = header.keys().any(|name| {
        rewrite_flux_klein_lora_tensor_name(name)
            .is_some_and(|converted| header.contains_key(&converted))
    });
    if has_collision {
        return (header, 0);
    }
    let mut rewritten = Map::with_capacity(header.len());
    let mut count = 0;
    for (name, value) in header {
        if name == "__metadata__" {
            rewritten.insert(name, value);
        } else if let Some(converted) = rewrite_flux_klein_lora_tensor_name(&name) {
            count += 1;
            rewritten.insert(converted, value);
        } else {
            rewritten.insert(name, value);
        }
    }
    (rewritten, count)
}

fn lora_compat_cache_path(root: &Path, relative: &Path, source: &Path) -> Result<PathBuf, String> {
    let (bytes, modified_at) = lora_file_fingerprint(source)?;
    let mut hasher = Sha256::new();
    hasher.update(LORA_COMPAT_CACHE_VERSION.as_bytes());
    hasher.update(relative.to_string_lossy().as_bytes());
    hasher.update(bytes.to_le_bytes());
    hasher.update(modified_at.to_le_bytes());
    let digest = format!("{:x}", hasher.finalize());
    let stem = relative
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("lora");
    Ok(root
        .join(LORA_COMPAT_CACHE_DIR)
        .join(format!("{stem}-{digest}.safetensors")))
}

/// The library-relative file sd-server should load: the LoRA itself, or a
/// cached copy with rewritten tensor names.
pub fn prepare_sdcpp_compatible_lora(
    root: &Path,
    relative: &Path,
    source: &Path,
) -> Result<PathBuf, String> {
    if !source
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("safetensors"))
    {
        return Ok(relative.to_path_buf());
    }
    let mut input = std::fs::File::open(source)
        .map_err(|error| format!("Failed to inspect the LoRA for sd.cpp compatibility: {error}"))?;
    let mut length_bytes = [0_u8; 8];
    input
        .read_exact(&mut length_bytes)
        .map_err(|error| format!("Failed to read the LoRA compatibility header: {error}"))?;
    let header_length = u64::from_le_bytes(length_bytes);
    let file_length = input
        .metadata()
        .map_err(|error| format!("Failed to inspect the LoRA file: {error}"))?
        .len();
    if header_length == 0
        || header_length > MAX_SAFETENSORS_HEADER_BYTES
        || header_length + 8 > file_length
    {
        return Err("The LoRA has an invalid safetensors compatibility header.".to_owned());
    }
    let mut header_bytes = vec![
        0_u8;
        usize::try_from(header_length).map_err(|_| {
            "The LoRA has an invalid safetensors compatibility header.".to_owned()
        })?
    ];
    input
        .read_exact(&mut header_bytes)
        .map_err(|error| format!("Failed to read the LoRA compatibility metadata: {error}"))?;
    let header = serde_json::from_slice::<Value>(&header_bytes)
        .map_err(|error| format!("Failed to parse the LoRA compatibility metadata: {error}"))?
        .as_object()
        .cloned()
        .ok_or_else(|| "The LoRA safetensors header is not an object.".to_owned())?;
    let (header, rewritten_count) = rewrite_flux_klein_lora_header(header);
    if rewritten_count == 0 {
        return Ok(relative.to_path_buf());
    }

    let destination = lora_compat_cache_path(root, relative, source)?;
    let serialized = serde_json::to_vec(&header)
        .map_err(|error| format!("Failed to serialize the LoRA compatibility metadata: {error}"))?;
    let padded_length = serialized
        .len()
        .checked_add(7)
        .map(|length| length / 8 * 8)
        .ok_or_else(|| "The LoRA compatibility header is too large.".to_owned())?;
    let expected_length = 8 + padded_length as u64 + file_length - 8 - header_length;
    if destination
        .metadata()
        .is_ok_and(|metadata| metadata.len() == expected_length)
    {
        return destination
            .strip_prefix(root)
            .map(Path::to_path_buf)
            .map_err(|_| "The LoRA compatibility cache is outside the library.".to_owned());
    }
    let cache_root = destination
        .parent()
        .ok_or_else(|| "The LoRA compatibility cache path is invalid.".to_owned())?;
    std::fs::create_dir_all(cache_root)
        .map_err(|error| format!("Failed to create the LoRA compatibility cache: {error}"))?;
    if destination.exists() {
        std::fs::remove_file(&destination)
            .map_err(|error| format!("Failed to replace the LoRA compatibility cache: {error}"))?;
    }
    let temporary = destination.with_extension(format!("{}.tmp", std::process::id()));
    let write_result = (|| -> Result<(), String> {
        let mut output = std::fs::File::create(&temporary)
            .map_err(|error| format!("Failed to create the LoRA compatibility cache: {error}"))?;
        output
            .write_all(&(padded_length as u64).to_le_bytes())
            .and_then(|()| output.write_all(&serialized))
            .and_then(|()| output.write_all(&vec![b' '; padded_length - serialized.len()]))
            .map_err(|error| format!("Failed to write the LoRA compatibility header: {error}"))?;
        input
            .seek(SeekFrom::Start(8 + header_length))
            .and_then(|_| std::io::copy(&mut input, &mut output))
            .map_err(|error| format!("Failed to copy the LoRA tensor data: {error}"))?;
        output
            .sync_all()
            .map_err(|error| format!("Failed to finish the LoRA compatibility cache: {error}"))?;
        std::fs::rename(&temporary, &destination)
            .map_err(|error| format!("Failed to activate the LoRA compatibility cache: {error}"))?;
        Ok(())
    })();
    if write_result.is_err() {
        std::fs::remove_file(&temporary).ok();
    }
    write_result?;
    destination
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .map_err(|_| "The LoRA compatibility cache is outside the library.".to_owned())
}

#[cfg(test)]
mod tests {
    use lettuce_types::OperationId;

    use super::*;

    fn write_safetensors(path: &Path, header: &Value, payload: &[u8]) {
        let serialized = serde_json::to_vec(header).expect("header");
        let padded_length = serialized.len().div_ceil(8) * 8;
        let mut file = std::fs::File::create(path).expect("file");
        file.write_all(&(padded_length as u64).to_le_bytes())
            .expect("length");
        file.write_all(&serialized).expect("header");
        file.write_all(&vec![b' '; padded_length - serialized.len()])
            .expect("padding");
        file.write_all(payload).expect("payload");
    }

    #[test]
    fn flux_klein_lora_aliases_are_rewritten_for_sdcpp() {
        let cases = [
            (
                "single_transformer_blocks.3.attn.to_qkv_mlp_proj.lora_A.default.weight",
                "single_transformer_blocks.3.attn.to_q.lora_A.default.weight",
            ),
            (
                "single_transformer_blocks.3.attn.to_out.lora_B.default.weight",
                "single_transformer_blocks.3.proj_out.lora_B.default.weight",
            ),
            (
                "transformer_blocks.2.ff.linear_in.lora_A.default.weight",
                "transformer_blocks.2.ff.net.0.proj.lora_A.default.weight",
            ),
            (
                "transformer_blocks.2.ff.linear_out.lora_B.default.weight",
                "transformer_blocks.2.ff.net.2.lora_B.default.weight",
            ),
            (
                "transformer_blocks.2.ff_context.linear_in.lora_A.default.weight",
                "transformer_blocks.2.ff_context.net.0.proj.lora_A.default.weight",
            ),
            (
                "transformer_blocks.2.ff_context.linear_out.lora_B.default.weight",
                "transformer_blocks.2.ff_context.net.2.lora_B.default.weight",
            ),
        ];
        for (source, expected) in cases {
            assert_eq!(
                rewrite_flux_klein_lora_tensor_name(source).as_deref(),
                Some(expected)
            );
        }
        assert_eq!(
            rewrite_flux_klein_lora_tensor_name(
                "transformer_blocks.2.attn.to_q.lora_A.default.weight"
            ),
            None
        );
    }

    #[test]
    fn flux_klein_lora_header_preserves_metadata_and_rewrites_tensor_keys() {
        let header = serde_json::json!({
            "__metadata__": {"ss_base_model_version": "flux.2-klein-4b"},
            "single_transformer_blocks.0.attn.to_out.lora_A.default.weight": {
                "dtype": "F16", "shape": [4, 4], "data_offsets": [0, 32]
            },
            "transformer_blocks.0.attn.to_q.lora_A.default.weight": {
                "dtype": "F16", "shape": [4, 4], "data_offsets": [32, 64]
            }
        })
        .as_object()
        .expect("object")
        .clone();
        let (rewritten, count) = rewrite_flux_klein_lora_header(header);
        assert_eq!(count, 1);
        assert!(rewritten.contains_key("__metadata__"));
        assert!(
            rewritten.contains_key("single_transformer_blocks.0.proj_out.lora_A.default.weight")
        );
        assert!(rewritten.contains_key("transformer_blocks.0.attn.to_q.lora_A.default.weight"));
    }

    #[test]
    fn flux_klein_lora_cache_preserves_tensor_payload_and_library_paths_are_confined() {
        let root = std::env::temp_dir().join(format!("sd-loras-{}", OperationId::new()));
        std::fs::create_dir_all(root.join("styles")).expect("root");
        let payload = [1_u8, 2, 3, 4, 5, 6, 7, 8];
        write_safetensors(
            &root.join("styles/style.safetensors"),
            &serde_json::json!({
                "single_transformer_blocks.0.attn.to_out.lora_A.default.weight": {
                    "dtype": "U8", "shape": [8], "data_offsets": [0, 8]
                }
            }),
            &payload,
        );
        let lora = |path: &str| StableDiffusionLora {
            path: path.to_owned(),
            multiplier: 0.8,
            is_high_noise: true,
            keywords: Vec::new(),
        };
        let klein = normalize_loras(
            &root,
            &[lora("styles/style.safetensors")],
            Some("flux-2-klein-4b"),
        )
        .expect("klein");
        assert!(klein[0].path.starts_with(".sdcpp-compat/style-"));
        assert!(klein[0].is_high_noise);
        let cached = std::fs::read(root.join(&klein[0].path)).expect("cached");
        let header_length =
            usize::try_from(u64::from_le_bytes(cached[..8].try_into().expect("length")))
                .expect("usize");
        let header: Value = serde_json::from_slice(&cached[8..8 + header_length]).expect("header");
        assert!(
            header
                .get("single_transformer_blocks.0.proj_out.lora_A.default.weight")
                .is_some()
        );
        assert_eq!(&cached[8 + header_length..], &payload);
        let again = normalize_loras(
            &root,
            &[lora("styles/style.safetensors")],
            Some("flux-2-klein-4b"),
        )
        .expect("cached again");
        assert_eq!(again, klein);

        let absolute = root.join("styles/style.safetensors");
        let plain = normalize_loras(
            &root,
            &[lora(absolute.to_str().expect("path"))],
            Some("z-image"),
        )
        .expect("plain");
        assert_eq!(plain[0].path, "styles/style.safetensors");
        assert!(matches!(
            normalize_loras(&root, &[lora("../outside.safetensors")], None),
            Err(LoraPathError::InvalidPath(_))
        ));
        assert!(matches!(
            normalize_loras(&root, &[lora("/elsewhere/a.safetensors")], None),
            Err(LoraPathError::OutsideLibrary(_))
        ));
        assert!(matches!(
            normalize_loras(&root, &[lora("missing.safetensors")], None),
            Err(LoraPathError::NotInstalled(_))
        ));
        std::fs::remove_dir_all(root).ok();
    }
}
