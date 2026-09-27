use std::path::Path;

use specta_typescript::Typescript;
use tauri::Runtime;
use tauri_specta::{collect_commands, collect_events};

use crate::{commands, events::AppEvent};

/// Where the generated TypeScript bindings are committed.
pub const BINDINGS_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ui/src/api/generated/bindings.ts"
);

/// Every command and event the frontend can reach.
#[must_use]
pub fn specta_builder<R: Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .commands(collect_commands![
            commands::conversations::conversations_list,
            commands::conversations::conversations_latest_by_character,
            commands::conversations::conversations_latest_by_group,
            commands::conversations::conversation_open,
            commands::conversations::conversation_messages,
            commands::conversations::conversation_send,
            commands::conversations::generation_cancel,
            commands::conversations::conversation_launch_direct,
            commands::conversations::conversation_launch_group,
            commands::characters::characters_list,
            commands::jobs::jobs_list,
            commands::jobs::job_get,
            commands::jobs::job_cancel,
            commands::jobs::job_watch,
            commands::files::files_inspect,
            commands::files::assets_ingest,
            commands::app::app_status,
            commands::app::app_ui_state_update,
            commands::app::purge_notices_list,
            commands::app::purge_notice_dismiss,
        ])
        .events(collect_events![AppEvent])
}

/// Writes the TypeScript bindings to `path`, ending in exactly one newline.
pub fn export_bindings(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    specta_builder::<tauri::Wry>().export(Typescript::default(), path)?;
    let text = std::fs::read_to_string(path)?;
    std::fs::write(path, format!("{}\n", text.trim_end()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_bindings_are_current() {
        let directory =
            std::env::temp_dir().join(format!("lettuce-tauri-bindings-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("temporary directory");
        let generated = directory.join("bindings.ts");
        export_bindings(&generated).expect("export bindings");
        let generated_text = std::fs::read_to_string(&generated).expect("generated bindings");
        std::fs::remove_dir_all(&directory).expect("remove temporary directory");
        let committed = std::fs::read_to_string(BINDINGS_PATH).unwrap_or_default();
        assert!(
            generated_text == committed,
            "apps/ui/src/api/generated/bindings.ts is stale; run `cargo run -p lettuce-tauri --bin export-bindings`"
        );
    }
}
