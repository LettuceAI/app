use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hint {
    pub name: String,
    pub provider: String,
    pub external_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Design {
    pub description: Option<String>,
    pub references: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaMedia {
    pub avatar: Option<String>,
    pub avatar_crop: Option<Crop>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRecommendation {
    pub file_name: String,
    pub sha256: Option<String>,
    pub strength: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LorebookBinding {
    pub book: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonaBehavior {
    pub image: Option<ImageRecommendation>,
    pub lorebooks: Vec<LorebookBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Persona {
    pub name: String,
    pub nickname: Option<String>,
    pub description: String,
    pub design: Design,
    pub media: PersonaMedia,
    pub behavior: PersonaBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Crop {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

impl From<lettuce_characters::Crop> for Crop {
    fn from(value: lettuce_characters::Crop) -> Self {
        Self {
            x: value.x,
            y: value.y,
            scale: value.scale,
        }
    }
}

impl From<Crop> for lettuce_characters::Crop {
    fn from(value: Crop) -> Self {
        Self {
            x: value.x,
            y: value.y,
            scale: value.scale,
        }
    }
}
