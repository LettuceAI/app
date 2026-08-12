use std::str::FromStr;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolCategory {
    Write,
    Set,
    Edit,
    Image,
    Read,
    Flow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    WriteDefinition,
    WriteScene,
    WriteLoreEntry,
    SetName,
    SetModel,
    SetPrompt,
    SetAvatarGradient,
    AttachLorebooks,
    EditScene,
    EditLoreEntry,
    DeleteScene,
    DeleteLoreEntry,
    DeletePersona,
    DeleteLorebook,
    ReorderLoreEntries,
    GenerateImage,
    EditAvatarImage,
    UseUploadedImage,
    ListModels,
    ListPrompts,
    ListPersonas,
    ListLorebooks,
    ListLoreEntries,
    ShowPreview,
    RequestConfirmation,
}

impl Tool {
    pub fn as_str(self) -> &'static str {
        use Tool::*;
        match self {
            WriteDefinition => "write_definition",
            WriteScene => "write_scene",
            WriteLoreEntry => "write_lore_entry",
            SetName => "set_name",
            SetModel => "set_model",
            SetPrompt => "set_prompt",
            SetAvatarGradient => "set_avatar_gradient",
            AttachLorebooks => "attach_lorebooks",
            EditScene => "edit_scene",
            EditLoreEntry => "edit_lore_entry",
            DeleteScene => "delete_scene",
            DeleteLoreEntry => "delete_lore_entry",
            DeletePersona => "delete_persona",
            DeleteLorebook => "delete_lorebook",
            ReorderLoreEntries => "reorder_lore_entries",
            GenerateImage => "generate_image",
            EditAvatarImage => "edit_avatar_image",
            UseUploadedImage => "use_uploaded_image",
            ListModels => "list_models",
            ListPrompts => "list_prompts",
            ListPersonas => "list_personas",
            ListLorebooks => "list_lorebooks",
            ListLoreEntries => "list_lore_entries",
            ShowPreview => "show_preview",
            RequestConfirmation => "request_confirmation",
        }
    }

    #[allow(dead_code)]
    pub fn category(self) -> ToolCategory {
        use Tool::*;
        match self {
            WriteDefinition | WriteScene | WriteLoreEntry => ToolCategory::Write,
            SetName | SetModel | SetPrompt | SetAvatarGradient | AttachLorebooks => {
                ToolCategory::Set
            }
            EditScene | EditLoreEntry | DeleteScene | DeleteLoreEntry | DeletePersona
            | DeleteLorebook | ReorderLoreEntries => ToolCategory::Edit,
            GenerateImage | EditAvatarImage | UseUploadedImage => ToolCategory::Image,
            ListModels | ListPrompts | ListPersonas | ListLorebooks | ListLoreEntries => {
                ToolCategory::Read
            }
            ShowPreview | RequestConfirmation => ToolCategory::Flow,
        }
    }
}

impl FromStr for Tool {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        use Tool::*;
        let normalized = s.trim().to_ascii_lowercase();
        let v = match normalized.as_str() {
            "write_definition" => WriteDefinition,
            "write_scene" => WriteScene,
            "write_lore_entry" => WriteLoreEntry,
            "set_name" => SetName,
            "set_model" => SetModel,
            "set_prompt" => SetPrompt,
            "set_avatar_gradient" => SetAvatarGradient,
            "attach_lorebooks" => AttachLorebooks,
            "edit_scene" => EditScene,
            "edit_lore_entry" => EditLoreEntry,
            "delete_scene" => DeleteScene,
            "delete_lore_entry" => DeleteLoreEntry,
            "delete_persona" => DeletePersona,
            "delete_lorebook" => DeleteLorebook,
            "reorder_lore_entries" => ReorderLoreEntries,
            "generate_image" => GenerateImage,
            "edit_avatar_image" => EditAvatarImage,
            "use_uploaded_image" | "use_image" => UseUploadedImage,
            "list_models" => ListModels,
            "list_prompts" => ListPrompts,
            "list_personas" => ListPersonas,
            "list_lorebooks" => ListLorebooks,
            "list_lore_entries" => ListLoreEntries,
            "show_preview" | "preview" => ShowPreview,
            "request_confirmation" | "confirm" => RequestConfirmation,
            _ => return Err(()),
        };
        Ok(v)
    }
}

pub fn allowed_tools(target: super::state::TargetKind) -> &'static [Tool] {
    use super::state::TargetKind::*;
    match target {
        Character => &[
            Tool::WriteDefinition,
            Tool::WriteScene,
            Tool::SetName,
            Tool::SetModel,
            Tool::SetPrompt,
            Tool::SetAvatarGradient,
            Tool::AttachLorebooks,
            Tool::EditScene,
            Tool::DeleteScene,
            Tool::GenerateImage,
            Tool::EditAvatarImage,
            Tool::UseUploadedImage,
            Tool::ListModels,
            Tool::ListPrompts,
            Tool::ListLorebooks,
            Tool::ShowPreview,
            Tool::RequestConfirmation,
        ],
        Persona => &[
            Tool::WriteDefinition,
            Tool::SetName,
            Tool::DeletePersona,
            Tool::GenerateImage,
            Tool::EditAvatarImage,
            Tool::UseUploadedImage,
            Tool::ListPersonas,
            Tool::ShowPreview,
            Tool::RequestConfirmation,
        ],
        Lorebook => &[
            Tool::WriteLoreEntry,
            Tool::SetName,
            Tool::EditLoreEntry,
            Tool::DeleteLoreEntry,
            Tool::DeleteLorebook,
            Tool::ReorderLoreEntries,
            Tool::ListLorebooks,
            Tool::ListLoreEntries,
            Tool::ShowPreview,
            Tool::RequestConfirmation,
        ],
    }
}
