use crate::descriptor::ProviderDescriptor;

/// Every remote chat provider this crate can execute, in catalog order.
/// Local llama.cpp and image-only providers live in their own crates.
pub fn provider_descriptors() -> &'static [&'static ProviderDescriptor] {
    &[
        &crate::providers::chutes::DESCRIPTOR,
        &crate::providers::openai::DESCRIPTOR,
        &crate::providers::cerebras::DESCRIPTOR,
        &crate::providers::anthropic::DESCRIPTOR,
        &crate::providers::openrouter::DESCRIPTOR,
        &crate::providers::literouter::DESCRIPTOR,
        &crate::providers::pollinations::DESCRIPTOR,
        &crate::providers::mistral::DESCRIPTOR,
        &crate::providers::deepseek::DESCRIPTOR,
        &crate::providers::nanogpt::DESCRIPTOR,
        &crate::providers::xai::DESCRIPTOR,
        &crate::providers::gemini::DESCRIPTOR,
        &crate::providers::gemini_express::DESCRIPTOR,
        &crate::providers::zai::DESCRIPTOR,
        &crate::providers::moonshot::DESCRIPTOR,
        &crate::providers::featherless::DESCRIPTOR,
        &crate::providers::qwen::DESCRIPTOR,
        &crate::providers::nvidia::DESCRIPTOR,
        &crate::providers::anannas::DESCRIPTOR,
        &crate::providers::groq::DESCRIPTOR,
        &crate::providers::ollama::DESCRIPTOR,
        &crate::providers::lmstudio::DESCRIPTOR,
        &crate::providers::intenserp::DESCRIPTOR,
        &crate::providers::custom::DESCRIPTOR,
        &crate::providers::custom_anthropic::DESCRIPTOR,
    ]
}

/// Resolves a canonical kind or one of its aliases.
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
