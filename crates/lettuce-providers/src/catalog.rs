use crate::descriptor::ProviderDescriptor;

/// Every remote chat provider this crate can execute, in the legacy
/// catalog order. Local llama.cpp and image-only providers live in their
/// own crates.
pub fn provider_descriptors() -> &'static [&'static ProviderDescriptor] {
    &[
        &crate::chutes::DESCRIPTOR,
        &crate::openai::DESCRIPTOR,
        &crate::cerebras::DESCRIPTOR,
        &crate::anthropic::DESCRIPTOR,
        &crate::openrouter::DESCRIPTOR,
        &crate::literouter::DESCRIPTOR,
        &crate::pollinations::DESCRIPTOR,
        &crate::mistral::DESCRIPTOR,
        &crate::deepseek::DESCRIPTOR,
        &crate::nanogpt::DESCRIPTOR,
        &crate::xai::DESCRIPTOR,
        &crate::gemini::DESCRIPTOR,
        &crate::gemini_express::DESCRIPTOR,
        &crate::zai::DESCRIPTOR,
        &crate::moonshot::DESCRIPTOR,
        &crate::featherless::DESCRIPTOR,
        &crate::qwen::DESCRIPTOR,
        &crate::nvidia::DESCRIPTOR,
        &crate::anannas::DESCRIPTOR,
        &crate::groq::DESCRIPTOR,
        &crate::ollama::DESCRIPTOR,
        &crate::lmstudio::DESCRIPTOR,
        &crate::intenserp::DESCRIPTOR,
        &crate::custom::DESCRIPTOR,
        &crate::custom_anthropic::DESCRIPTOR,
    ]
}

/// Resolves a canonical kind or one of its legacy aliases.
pub fn provider_descriptor(kind: &str) -> Option<&'static ProviderDescriptor> {
    let kind = kind.trim();
    provider_descriptors().iter().copied().find(|descriptor| {
        descriptor.kind.eq_ignore_ascii_case(kind)
            || descriptor
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(kind))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_kinds_and_aliases_are_unique_and_resolvable() {
        let mut seen = HashSet::new();
        for descriptor in provider_descriptors() {
            assert!(seen.insert(descriptor.kind), "{}", descriptor.kind);
            for alias in descriptor.aliases {
                assert!(seen.insert(alias), "{alias}");
                assert_eq!(
                    provider_descriptor(alias).map(|d| d.kind),
                    Some(descriptor.kind)
                );
            }
            assert_eq!(
                provider_descriptor(descriptor.kind).map(|d| d.kind),
                Some(descriptor.kind)
            );
        }
        assert_eq!(provider_descriptors().len(), 25);
        assert!(provider_descriptor("lettuce-host").is_none());
        assert!(provider_descriptor("lettuce-engine").is_none());
        assert!(provider_descriptor("").is_none());
    }
}
