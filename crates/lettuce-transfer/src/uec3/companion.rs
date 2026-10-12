use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmotionVector {
    pub warmth: f64,
    pub trust: f64,
    pub calm: f64,
    pub vulnerability: f64,
    pub longing: f64,
    pub hurt: f64,
    pub tension: f64,
    pub irritation: f64,
    pub affection_intensity: f64,
    pub reassurance_need: f64,
}

impl From<&lettuce_companions::EmotionVector> for EmotionVector {
    fn from(value: &lettuce_companions::EmotionVector) -> Self {
        Self {
            warmth: value.warmth,
            trust: value.trust,
            calm: value.calm,
            vulnerability: value.vulnerability,
            longing: value.longing,
            hurt: value.hurt,
            tension: value.tension,
            irritation: value.irritation,
            affection_intensity: value.affection_intensity,
            reassurance_need: value.reassurance_need,
        }
    }
}

impl From<EmotionVector> for lettuce_companions::EmotionVector {
    fn from(value: EmotionVector) -> Self {
        Self {
            warmth: value.warmth,
            trust: value.trust,
            calm: value.calm,
            vulnerability: value.vulnerability,
            longing: value.longing,
            hurt: value.hurt,
            tension: value.tension,
            irritation: value.irritation,
            affection_intensity: value.affection_intensity,
            reassurance_need: value.reassurance_need,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegulationStyle {
    pub suppression: f64,
    pub volatility: f64,
    pub recovery_speed: f64,
    pub conflict_avoidance: f64,
    pub reassurance_seeking: f64,
    pub protest_behavior: f64,
    pub emotional_transparency: f64,
    pub attachment_activation: f64,
    pub pride: f64,
}

impl From<&lettuce_companions::RegulationStyle> for RegulationStyle {
    fn from(value: &lettuce_companions::RegulationStyle) -> Self {
        Self {
            suppression: value.suppression,
            volatility: value.volatility,
            recovery_speed: value.recovery_speed,
            conflict_avoidance: value.conflict_avoidance,
            reassurance_seeking: value.reassurance_seeking,
            protest_behavior: value.protest_behavior,
            emotional_transparency: value.emotional_transparency,
            attachment_activation: value.attachment_activation,
            pride: value.pride,
        }
    }
}

impl From<RegulationStyle> for lettuce_companions::RegulationStyle {
    fn from(value: RegulationStyle) -> Self {
        Self {
            suppression: value.suppression,
            volatility: value.volatility,
            recovery_speed: value.recovery_speed,
            conflict_avoidance: value.conflict_avoidance,
            reassurance_seeking: value.reassurance_seeking,
            protest_behavior: value.protest_behavior,
            emotional_transparency: value.emotional_transparency,
            attachment_activation: value.attachment_activation,
            pride: value.pride,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipDefaults {
    pub closeness: f64,
    pub trust: f64,
    pub affection: f64,
    pub tension: f64,
}

impl From<&lettuce_companions::RelationshipDefaults> for RelationshipDefaults {
    fn from(value: &lettuce_companions::RelationshipDefaults) -> Self {
        Self {
            closeness: value.closeness,
            trust: value.trust,
            affection: value.affection,
            tension: value.tension,
        }
    }
}

impl From<RelationshipDefaults> for lettuce_companions::RelationshipDefaults {
    fn from(value: RelationshipDefaults) -> Self {
        Self {
            closeness: value.closeness,
            trust: value.trust,
            affection: value.affection,
            tension: value.tension,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoulIdentity {
    pub essence: String,
    pub traits: String,
    pub backstory: String,
    pub appearance: String,
    pub goals: String,
    pub likes: String,
    pub voice: String,
    pub relational_style: String,
    pub vulnerabilities: String,
    pub fears: String,
    pub habits: String,
    pub boundaries: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Companion {
    pub soul: SoulIdentity,
    pub baseline_affect: EmotionVector,
    pub regulation_style: RegulationStyle,
    pub relationship_defaults: RelationshipDefaults,
    pub authored_facts: Vec<AuthoredFact>,
    pub prompting: Prompting,
    pub time_awareness: bool,
    pub share_memory_across_chats: bool,
    pub share_soul_growth_across_chats: bool,
    pub scheduled_notes: Vec<ScheduledNote>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredFact {
    pub key: String,
    pub category: lettuce_companions::SoulCategory,
    pub value: String,
    pub policy: lettuce_companions::SoulFactPolicy,
    pub slot: String,
    pub locked: bool,
    pub weight: f64,
    pub valid_from: i64,
    pub valid_until: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompting {
    pub prompt: Option<String>,
    pub style_notes: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledNote {
    pub key: String,
    pub label: String,
    pub content: String,
    pub available_at: i64,
    pub expires_at: Option<i64>,
    pub recurrence: lettuce_companions::ScheduledNoteRecurrence,
    pub recurrence_window_ms: Option<u64>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub companion: CompanionState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionState {
    pub soul: Vec<LearnedFact>,
    pub memory: Memory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub items: Vec<MemoryItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnedFactKind {
    Authored,
    Add,
    Adjust,
    Consolidated,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnedFact {
    pub key: String,
    pub category: lettuce_companions::SoulCategory,
    pub value: String,
    pub kind: LearnedFactKind,
    pub policy: lettuce_companions::SoulFactPolicy,
    pub slot: String,
    pub confidence: f64,
    pub evidence_count: u32,
    pub weight: f64,
    pub valid_from: i64,
    pub valid_until: Option<i64>,
    pub locked: bool,
    pub created_at: i64,
    pub supersedes: Vec<String>,
    pub superseded_by: Option<String>,
    pub superseded_at: Option<i64>,
    pub source_memories: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedPrecision {
    #[serde(alias = "turn")]
    User,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    pub key: String,
    pub text: String,
    pub category: Option<lettuce_memory::MemoryCategory>,
    pub origin: lettuce_memory::MemoryOrigin,
    pub observed_at: Option<i64>,
    pub observed_precision: Option<ObservedPrecision>,
    pub supersedes: Vec<String>,
    pub superseded_by: Option<String>,
    pub superseded_at: Option<i64>,
    pub pinned: bool,
    pub cold: bool,
    pub importance: u16,
    pub persistence_importance: u16,
    pub prompt_importance: u16,
    pub volatility: u16,
    pub created_at: i64,
}
