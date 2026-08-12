use tauri::AppHandle;

use crate::chat_manager::prompting::prompt_engine;
use crate::chat_manager::prompting::prompts::{
    get_template, APP_AVATAR_EDIT_TEMPLATE_ID, APP_AVATAR_GENERATION_TEMPLATE_ID,
};

fn substitute(
    template: &str,
    name: &str,
    description: &str,
    avatar_request: &str,
    current_avatar_prompt: &str,
    edit_request: &str,
) -> String {
    let pairs: &[(&str, &str)] = &[
        ("{{avatar_subject_name}}", name),
        ("{{avatar_subject_description}}", description),
        ("{{avatar_request}}", avatar_request),
        ("{{current_avatar_prompt}}", current_avatar_prompt),
        ("{{edit_request}}", edit_request),
        ("{{char.name}}", name),
        ("{{char.desc}}", description),
        ("{{persona.name}}", name),
        ("{{persona.desc}}", description),
    ];
    let mut out = template.to_string();
    for (k, v) in pairs {
        out = out.replace(k, v);
    }
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out.trim().to_string()
}

fn resolve_template(app: &AppHandle, id: &str, fallback: String) -> String {
    match get_template(app, id) {
        Ok(Some(t)) if !t.content.trim().is_empty() => t.content,
        _ => fallback,
    }
}

pub fn build_generation_prompt(
    app: &AppHandle,
    name: &str,
    description: &str,
    avatar_request: &str,
) -> String {
    let template = resolve_template(
        app,
        APP_AVATAR_GENERATION_TEMPLATE_ID,
        prompt_engine::default_avatar_generation_prompt(),
    );
    substitute(&template, name, description, avatar_request, "", "")
}

pub fn build_edit_prompt(
    app: &AppHandle,
    name: &str,
    description: &str,
    current_avatar_prompt: &str,
    edit_request: &str,
) -> String {
    let template = resolve_template(
        app,
        APP_AVATAR_EDIT_TEMPLATE_ID,
        prompt_engine::default_avatar_edit_prompt(),
    );
    substitute(
        &template,
        name,
        description,
        "",
        current_avatar_prompt,
        edit_request,
    )
}
