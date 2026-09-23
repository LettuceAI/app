//! Model files stored by absolute path, rewritten when their folder moves.

use crate::{ModelProfile, ModelRepositoryError, ProviderProtocol};

/// `path` with the folder `old_prefix` replaced by `new_prefix`; `None` when
/// `path` is not that folder or inside it.
#[must_use]
pub fn rewrite_path_prefix(path: &str, old_prefix: &str, new_prefix: &str) -> Option<String> {
    let separator = std::path::MAIN_SEPARATOR;
    let old = old_prefix.trim_end_matches(['/', '\\']);
    let new = new_prefix.trim_end_matches(['/', '\\']);
    if path == old {
        return Some(new.to_owned());
    }
    path.strip_prefix(&format!("{old}{separator}"))
        .map(|rest| format!("{new}{separator}{rest}"))
}

/// Rewrites every file path the profile stores (a local model's own file,
/// its llama.cpp projector and MTP draft model, its stable-diffusion.cpp
/// components and base LoRAs) that `relocate` maps; reports whether any
/// changed.
pub fn relocate_profile_paths(
    profile: &mut ModelProfile,
    protocol: ProviderProtocol,
    relocate: &dyn Fn(&str) -> Option<String>,
) -> bool {
    let mut changed = false;
    let mut apply = |path: &mut String| {
        if let Some(moved) = relocate(path)
            && moved != *path
        {
            *path = moved;
            changed = true;
        }
    };
    if matches!(
        protocol,
        ProviderProtocol::LlamaCpp | ProviderProtocol::StableDiffusion
    ) {
        apply(&mut profile.external_model_id);
    }
    let llama = &mut profile.config.llama_cpp;
    let diffusion = &mut profile.config.stable_diffusion;
    for path in [
        &mut llama.mmproj_path,
        &mut llama.mtp_model_path,
        &mut diffusion.cpp.text_encoder_path,
        &mut diffusion.cpp.vae_path,
        &mut diffusion.cpp.vision_encoder_path,
    ]
    .into_iter()
    .flatten()
    {
        apply(path);
    }
    for lora in diffusion.base_loras.iter_mut().flatten() {
        apply(&mut lora.path);
    }
    changed
}

/// Moves stored model paths in one transaction.
pub trait ModelPathRelocation: Send + Sync {
    /// Applies [`relocate_profile_paths`] to every profile and saves the
    /// changed ones; returns how many changed.
    fn relocate_model_paths(
        &self,
        relocate: &dyn Fn(&str) -> Option<String>,
        now: lettuce_types::TimestampMillis,
    ) -> Result<u32, ModelRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_paths_inside_the_old_folder_move() {
        let separator = std::path::MAIN_SEPARATOR;
        let old = format!("{separator}models{separator}");
        let inside = format!("{separator}models{separator}org--m{separator}m.gguf");
        assert_eq!(
            rewrite_path_prefix(&inside, &old, &format!("{separator}new")),
            Some(format!("{separator}new{separator}org--m{separator}m.gguf"))
        );
        assert_eq!(
            rewrite_path_prefix(&format!("{separator}models"), &old, "/x"),
            Some("/x".to_owned())
        );
        assert_eq!(
            rewrite_path_prefix(&format!("{separator}models2{separator}m.gguf"), &old, "/x"),
            None
        );
    }
}
