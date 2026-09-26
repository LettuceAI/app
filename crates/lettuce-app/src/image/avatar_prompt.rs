//! Avatar image prompts: the active avatar prompt document rendered with the
//! subject, the request and the two avatar entry conditions.

use lettuce_context::{PromptDocument, PromptEntryCondition, PromptRepository};

use crate::generation::built_in_prompts::{
    BuiltInPromptCatalog, BuiltInPromptId, active_built_in_prompt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvatarPromptRequest {
    Generation {
        subject_name: String,
        subject_description: String,
        avatar_request: String,
    },
    Edit {
        subject_name: String,
        subject_description: String,
        current_avatar_prompt: String,
        edit_request: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AvatarPromptError {
    #[error("the avatar prompt is unavailable")]
    Unavailable,
    #[error("the avatar prompt could not be rendered")]
    Render,
}

impl AvatarPromptRequest {
    const fn built_in(&self) -> BuiltInPromptId {
        match self {
            Self::Generation { .. } => BuiltInPromptId::AvatarGeneration,
            Self::Edit { .. } => BuiltInPromptId::AvatarEdit,
        }
    }

    /// What a local stable-diffusion.cpp model receives: the user's request
    /// as typed, without the avatar template.
    fn raw_request(&self) -> &str {
        match self {
            Self::Generation { avatar_request, .. } => avatar_request.trim(),
            Self::Edit { edit_request, .. } => edit_request.trim(),
        }
    }
}

/// The prompt an avatar generation or edit sends to `provider_kind`.
pub fn avatar_image_prompt<R: PromptRepository + ?Sized>(
    repository: &R,
    provider_kind: &str,
    request: &AvatarPromptRequest,
) -> Result<String, AvatarPromptError> {
    if provider_kind.eq_ignore_ascii_case(lettuce_image_generation::LOCAL_DIFFUSION_PROVIDER_KIND) {
        return Ok(request.raw_request().to_owned());
    }
    let document = active_built_in_prompt(repository, request.built_in())
        .map_err(|_| AvatarPromptError::Unavailable)?
        .ok_or(AvatarPromptError::Unavailable)?;
    render_avatar_prompt(&document, request)
}

impl crate::AppBackend {
    /// The prompt an avatar generation or edit sends to `provider_kind`.
    pub fn avatar_image_prompt(
        &self,
        provider_kind: &str,
        request: &AvatarPromptRequest,
    ) -> Result<String, AvatarPromptError> {
        avatar_image_prompt(self.database(), provider_kind, request)
    }
}

/// The enabled, non-blank entries whose avatar conditions hold, in document
/// order and joined by blank lines (else every enabled entry of the bundled
/// seed), then the nine placeholders replaced one after another with trimmed
/// values and blank runs collapsed.
pub(crate) fn render_avatar_prompt(
    document: &PromptDocument,
    request: &AvatarPromptRequest,
) -> Result<String, AvatarPromptError> {
    let values = request.values();
    let conditions = AvatarConditions {
        has_subject_description: !values.subject_description.is_empty(),
        has_current_description: !values.current_avatar_prompt.is_empty(),
    };
    let merged = merge_entries(
        document.entries.iter().map(|entry| {
            (
                entry.enabled,
                entry.content.as_str(),
                entry.conditions.as_ref(),
            )
        }),
        Some(conditions),
    );
    let template = if merged.trim().is_empty() {
        let catalog =
            BuiltInPromptCatalog::bundled().map_err(|_| AvatarPromptError::Unavailable)?;
        merge_entries(
            catalog
                .seed(request.built_in())
                .entries
                .iter()
                .map(|entry| {
                    (
                        entry.enabled,
                        entry.content.as_str(),
                        entry.conditions.as_ref(),
                    )
                }),
            None,
        )
    } else {
        merged
    };
    let mut prompt = template;
    for (placeholder, value) in [
        ("{{avatar_subject_name}}", values.subject_name),
        ("{{avatar_subject_description}}", values.subject_description),
        ("{{avatar_request}}", values.avatar_request),
        ("{{current_avatar_prompt}}", values.current_avatar_prompt),
        ("{{edit_request}}", values.edit_request),
        ("{{char.name}}", values.subject_name),
        ("{{char.desc}}", values.subject_description),
        ("{{persona.name}}", values.subject_name),
        ("{{persona.desc}}", values.subject_description),
    ] {
        prompt = prompt.replace(placeholder, value);
    }
    while prompt.contains("\n\n\n") {
        prompt = prompt.replace("\n\n\n", "\n\n");
    }
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(AvatarPromptError::Unavailable);
    }
    Ok(prompt.to_owned())
}

struct AvatarValues<'a> {
    subject_name: &'a str,
    subject_description: &'a str,
    avatar_request: &'a str,
    current_avatar_prompt: &'a str,
    edit_request: &'a str,
}

impl AvatarPromptRequest {
    fn values(&self) -> AvatarValues<'_> {
        match self {
            Self::Generation {
                subject_name,
                subject_description,
                avatar_request,
            } => AvatarValues {
                subject_name: subject_name.trim(),
                subject_description: subject_description.trim(),
                avatar_request: avatar_request.trim(),
                current_avatar_prompt: "",
                edit_request: "",
            },
            Self::Edit {
                subject_name,
                subject_description,
                current_avatar_prompt,
                edit_request,
            } => AvatarValues {
                subject_name: subject_name.trim(),
                subject_description: subject_description.trim(),
                avatar_request: "",
                current_avatar_prompt: current_avatar_prompt.trim(),
                edit_request: edit_request.trim(),
            },
        }
    }
}

#[derive(Clone, Copy)]
struct AvatarConditions {
    has_subject_description: bool,
    has_current_description: bool,
}

/// Only the two avatar conditions and the combinators are read; every other
/// condition passes.
fn avatar_condition_holds(condition: &PromptEntryCondition, context: AvatarConditions) -> bool {
    match condition {
        PromptEntryCondition::HasSubjectDescription { value } => {
            context.has_subject_description == *value
        }
        PromptEntryCondition::HasCurrentDescription { value } => {
            context.has_current_description == *value
        }
        PromptEntryCondition::All { conditions } => conditions
            .iter()
            .all(|child| avatar_condition_holds(child, context)),
        PromptEntryCondition::Any { conditions } => {
            !conditions.is_empty()
                && conditions
                    .iter()
                    .any(|child| avatar_condition_holds(child, context))
        }
        PromptEntryCondition::Not { condition } => !avatar_condition_holds(condition, context),
        _ => true,
    }
}

fn merge_entries<'a>(
    entries: impl Iterator<Item = (bool, &'a str, Option<&'a PromptEntryCondition>)>,
    conditions: Option<AvatarConditions>,
) -> String {
    entries
        .filter(|(enabled, content, condition)| {
            *enabled
                && !content.trim().is_empty()
                && match (condition, conditions) {
                    (Some(condition), Some(context)) => avatar_condition_holds(condition, context),
                    _ => true,
                }
        })
        .map(|(_, content, _)| content)
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::built_in_prompts::seed_document;

    fn generation(description: &str) -> AvatarPromptRequest {
        AvatarPromptRequest::Generation {
            subject_name: "  Mira ".into(),
            subject_description: description.into(),
            avatar_request: " silver hair, rainy harbor ".into(),
        }
    }

    #[test]
    fn generation_renders_the_bundled_entries_with_the_subject() {
        let document = seed_document(BuiltInPromptId::AvatarGeneration);
        let prompt =
            render_avatar_prompt(&document, &generation("A ship's navigator.")).expect("prompt");
        assert!(prompt.contains("Name: Mira\nA ship's navigator."));
        assert!(prompt.contains("# Avatar Request\nsilver hair, rainy harbor"));
        assert!(!prompt.contains("{{"));
        assert!(!prompt.contains("\n\n\n"));
        assert_eq!(prompt, prompt.trim());
    }

    #[test]
    fn edit_renders_the_current_prompt_and_request() {
        let document = seed_document(BuiltInPromptId::AvatarEdit);
        let prompt = render_avatar_prompt(
            &document,
            &AvatarPromptRequest::Edit {
                subject_name: "Mira".into(),
                subject_description: String::new(),
                current_avatar_prompt: "previous render".into(),
                edit_request: "add a scarf".into(),
            },
        )
        .expect("prompt");
        assert!(prompt.contains("previous render"));
        assert!(prompt.contains("add a scarf"));
        assert!(!prompt.contains("{{"));
    }

    #[test]
    fn entries_follow_the_legacy_subject_and_current_prompt_conditions() {
        let generation_document = seed_document(BuiltInPromptId::AvatarGeneration);
        let without_description =
            render_avatar_prompt(&generation_document, &generation("   ")).expect("prompt");
        assert!(!without_description.contains("# Avatar Subject"));
        assert!(!without_description.contains("Mira"));
        let edit_document = seed_document(BuiltInPromptId::AvatarEdit);
        let without_current = render_avatar_prompt(
            &edit_document,
            &AvatarPromptRequest::Edit {
                subject_name: "Mira".into(),
                subject_description: "Navigator".into(),
                current_avatar_prompt: " ".into(),
                edit_request: "add a scarf".into(),
            },
        )
        .expect("prompt");
        assert!(!without_current.contains("# Current Avatar Prompt"));
        assert!(without_current.contains("# Avatar Subject\nName: Mira\nNavigator"));
    }

    #[test]
    fn entries_are_read_like_legacy_templates() {
        let mut document = seed_document(BuiltInPromptId::AvatarGeneration);
        for entry in &mut document.entries {
            entry.system_prompt = true;
        }
        document.entries[0].enabled = false;
        document.entries[1].injection_position = lettuce_context::PromptEntryPosition::Conditional;
        document.entries[1].conditional_min_messages = Some(5);
        document.entries.swap(1, 2);
        document.entries[4].content =
            "{{char}} / {{user}} / {{#if x}}kept{{/if}} / {{char.name}}".into();
        let prompt = render_avatar_prompt(
            &document,
            &AvatarPromptRequest::Generation {
                subject_name: "Mira".into(),
                subject_description: "Loves {{char}} and {{user}}".into(),
                avatar_request: "harbor".into(),
            },
        )
        .expect("prompt");
        assert!(!prompt.contains("Generate a character avatar image directly"));
        assert!(
            prompt.find("# Avatar Request").expect("request")
                < prompt.find("# Avatar Subject").expect("subject")
        );
        assert!(prompt.contains("Name: Mira\nLoves {{char}} and {{user}}"));
        assert!(prompt.ends_with("{{char}} / {{user}} / {{#if x}}kept{{/if}} / Mira"));
    }

    #[test]
    fn a_template_with_nothing_selected_falls_back_to_the_bundled_text() {
        let mut document = seed_document(BuiltInPromptId::AvatarGeneration);
        for entry in &mut document.entries {
            entry.enabled = false;
        }
        let prompt = render_avatar_prompt(&document, &generation("")).expect("prompt");
        assert!(prompt.starts_with("Generate a character avatar image directly"));
        assert!(prompt.contains("Name: Mira"));
    }

    #[test]
    fn remote_models_use_the_active_bootstrapped_document() {
        let backend = crate::AppBackend::open_in_memory(lettuce_types::TimestampMillis::new(1))
            .expect("backend");
        let prompt = backend
            .avatar_image_prompt("openai", &generation("A ship's navigator."))
            .expect("prompt");
        assert_eq!(
            prompt,
            render_avatar_prompt(
                &seed_document(BuiltInPromptId::AvatarGeneration),
                &generation("A ship's navigator.")
            )
            .expect("seed prompt")
        );
    }

    #[test]
    fn local_diffusion_models_receive_the_request_as_typed() {
        let repository = lettuce_database::Database::open_in_memory().expect("database");
        assert_eq!(
            avatar_image_prompt(&repository, "sdcpp", &generation("")).expect("prompt"),
            "silver hair, rainy harbor"
        );
    }
}
