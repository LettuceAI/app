use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Inherit {
    Inherit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LorebookSelection {
    Inherit(Inherit),
    Explicit(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StarterMessage {
    pub role: lettuce_characters::StarterRole,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatTemplate {
    pub key: String,
    pub name: String,
    pub messages: Vec<StarterMessage>,
    pub prompt: Option<String>,
    pub lorebooks: LorebookSelection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Starter {
    #[serde(flatten)]
    pub template: ChatTemplate,
    pub scene: Option<String>,
}
