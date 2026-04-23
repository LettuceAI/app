pub mod memory;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::chat_manager::types::{Character, Session};
use crate::embedding::emotion::{EmotionClassification, EmotionLabelScore};
use crate::utils::log_warn;

const DECAY_MINUTES: f64 = 45.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for EmotionVector {
    fn default() -> Self {
        Self {
            warmth: 0.0,
            trust: 0.0,
            calm: 0.0,
            vulnerability: 0.0,
            longing: 0.0,
            hurt: 0.0,
            tension: 0.0,
            irritation: 0.0,
            affection_intensity: 0.0,
            reassurance_need: 0.0,
        }
    }
}

impl EmotionVector {
    fn clamp(mut self) -> Self {
        self.warmth = clamp01(self.warmth);
        self.trust = clamp01(self.trust);
        self.calm = clamp01(self.calm);
        self.vulnerability = clamp01(self.vulnerability);
        self.longing = clamp01(self.longing);
        self.hurt = clamp01(self.hurt);
        self.tension = clamp01(self.tension);
        self.irritation = clamp01(self.irritation);
        self.affection_intensity = clamp01(self.affection_intensity);
        self.reassurance_need = clamp01(self.reassurance_need);
        self
    }

    fn scaled(&self, scale: f64) -> Self {
        Self {
            warmth: self.warmth * scale,
            trust: self.trust * scale,
            calm: self.calm * scale,
            vulnerability: self.vulnerability * scale,
            longing: self.longing * scale,
            hurt: self.hurt * scale,
            tension: self.tension * scale,
            irritation: self.irritation * scale,
            affection_intensity: self.affection_intensity * scale,
            reassurance_need: self.reassurance_need * scale,
        }
        .clamp_signed()
    }

    fn add(&self, other: &Self) -> Self {
        Self {
            warmth: self.warmth + other.warmth,
            trust: self.trust + other.trust,
            calm: self.calm + other.calm,
            vulnerability: self.vulnerability + other.vulnerability,
            longing: self.longing + other.longing,
            hurt: self.hurt + other.hurt,
            tension: self.tension + other.tension,
            irritation: self.irritation + other.irritation,
            affection_intensity: self.affection_intensity + other.affection_intensity,
            reassurance_need: self.reassurance_need + other.reassurance_need,
        }
        .clamp()
    }

    fn lerp(&self, target: &Self, weight: f64) -> Self {
        let w = clamp01(weight);
        Self {
            warmth: self.warmth * (1.0 - w) + target.warmth * w,
            trust: self.trust * (1.0 - w) + target.trust * w,
            calm: self.calm * (1.0 - w) + target.calm * w,
            vulnerability: self.vulnerability * (1.0 - w) + target.vulnerability * w,
            longing: self.longing * (1.0 - w) + target.longing * w,
            hurt: self.hurt * (1.0 - w) + target.hurt * w,
            tension: self.tension * (1.0 - w) + target.tension * w,
            irritation: self.irritation * (1.0 - w) + target.irritation * w,
            affection_intensity: self.affection_intensity * (1.0 - w)
                + target.affection_intensity * w,
            reassurance_need: self.reassurance_need * (1.0 - w) + target.reassurance_need * w,
        }
        .clamp_signed()
    }

    fn subtract_positive(&self, other: &Self) -> Self {
        Self {
            warmth: (self.warmth - other.warmth).max(0.0),
            trust: (self.trust - other.trust).max(0.0),
            calm: (self.calm - other.calm).max(0.0),
            vulnerability: (self.vulnerability - other.vulnerability).max(0.0),
            longing: (self.longing - other.longing).max(0.0),
            hurt: (self.hurt - other.hurt).max(0.0),
            tension: (self.tension - other.tension).max(0.0),
            irritation: (self.irritation - other.irritation).max(0.0),
            affection_intensity: (self.affection_intensity - other.affection_intensity).max(0.0),
            reassurance_need: (self.reassurance_need - other.reassurance_need).max(0.0),
        }
        .clamp()
    }

    fn clamp_signed(mut self) -> Self {
        self.warmth = clamp_signed(self.warmth);
        self.trust = clamp_signed(self.trust);
        self.calm = clamp_signed(self.calm);
        self.vulnerability = clamp_signed(self.vulnerability);
        self.longing = clamp_signed(self.longing);
        self.hurt = clamp_signed(self.hurt);
        self.tension = clamp_signed(self.tension);
        self.irritation = clamp_signed(self.irritation);
        self.affection_intensity = clamp_signed(self.affection_intensity);
        self.reassurance_need = clamp_signed(self.reassurance_need);
        self
    }

    fn decay_toward(&self, baseline: &Self, elapsed_minutes: f64, recovery_speed: f64) -> Self {
        let decay_strength = (elapsed_minutes / DECAY_MINUTES) * (0.35 + recovery_speed * 0.85);
        let factor = (-decay_strength).exp();
        Self {
            warmth: baseline.warmth + (self.warmth - baseline.warmth) * factor,
            trust: baseline.trust + (self.trust - baseline.trust) * factor,
            calm: baseline.calm + (self.calm - baseline.calm) * factor,
            vulnerability: baseline.vulnerability
                + (self.vulnerability - baseline.vulnerability) * factor,
            longing: baseline.longing + (self.longing - baseline.longing) * factor,
            hurt: baseline.hurt + (self.hurt - baseline.hurt) * factor,
            tension: baseline.tension + (self.tension - baseline.tension) * factor,
            irritation: baseline.irritation + (self.irritation - baseline.irritation) * factor,
            affection_intensity: baseline.affection_intensity
                + (self.affection_intensity - baseline.affection_intensity) * factor,
            reassurance_need: baseline.reassurance_need
                + (self.reassurance_need - baseline.reassurance_need) * factor,
        }
        .clamp()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for RegulationStyle {
    fn default() -> Self {
        Self {
            suppression: 0.35,
            volatility: 0.25,
            recovery_speed: 0.55,
            conflict_avoidance: 0.45,
            reassurance_seeking: 0.4,
            protest_behavior: 0.2,
            emotional_transparency: 0.55,
            attachment_activation: 0.45,
            pride: 0.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmotionalState {
    pub felt: EmotionVector,
    pub expressed: EmotionVector,
    pub blocked: EmotionVector,
    pub momentum: EmotionVector,
    pub active_drivers: Vec<String>,
    pub confidence: f64,
    pub updated_at: u64,
}

impl Default for EmotionalState {
    fn default() -> Self {
        Self {
            felt: EmotionVector::default(),
            expressed: EmotionVector::default(),
            blocked: EmotionVector::default(),
            momentum: EmotionVector::default(),
            active_drivers: Vec::new(),
            confidence: 0.5,
            updated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationshipState {
    pub closeness: f64,
    pub trust: f64,
    pub affection: f64,
    pub tension: f64,
    pub stability: f64,
    pub interaction_count: u32,
    pub last_interaction_at: u64,
}

impl Default for RelationshipState {
    fn default() -> Self {
        Self {
            closeness: 0.2,
            trust: 0.3,
            affection: 0.15,
            tension: 0.0,
            stability: 0.5,
            interaction_count: 0,
            last_interaction_at: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionSessionState {
    pub emotional_state: EmotionalState,
    pub relationship_state: RelationshipState,
    pub active_signals: Vec<String>,
    pub updated_at: u64,
}

impl Default for CompanionSessionState {
    fn default() -> Self {
        Self {
            emotional_state: EmotionalState::default(),
            relationship_state: RelationshipState::default(),
            active_signals: Vec::new(),
            updated_at: 0,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CompanionPromptingConfig {
    #[serde(default)]
    prompt_template_id: Option<String>,
    #[serde(default)]
    style_notes: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelationshipDefaults {
    #[serde(default = "default_closeness")]
    closeness: f64,
    #[serde(default = "default_trust")]
    trust: f64,
    #[serde(default = "default_affection")]
    affection: f64,
    #[serde(default)]
    tension: f64,
}

impl Default for RelationshipDefaults {
    fn default() -> Self {
        Self {
            closeness: default_closeness(),
            trust: default_trust(),
            affection: default_affection(),
            tension: 0.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SoulConfig {
    #[serde(default)]
    essence: String,
    #[serde(default)]
    voice: String,
    #[serde(default)]
    relational_style: String,
    #[serde(default)]
    vulnerabilities: String,
    #[serde(default)]
    habits: String,
    #[serde(default)]
    boundaries: String,
    #[serde(default)]
    baseline_affect: EmotionVector,
    #[serde(default)]
    regulation_style: RegulationStyle,
}

impl Default for SoulConfig {
    fn default() -> Self {
        Self {
            essence: String::new(),
            voice: String::new(),
            relational_style: String::new(),
            vulnerabilities: String::new(),
            habits: String::new(),
            boundaries: String::new(),
            baseline_affect: EmotionVector {
                warmth: 0.45,
                trust: 0.35,
                calm: 0.65,
                vulnerability: 0.2,
                longing: 0.15,
                hurt: 0.05,
                tension: 0.1,
                irritation: 0.05,
                affection_intensity: 0.25,
                reassurance_need: 0.15,
            },
            regulation_style: RegulationStyle::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CompanionMemoryConfig {
    #[serde(default = "default_memory_enabled")]
    enabled: bool,
    #[serde(default = "default_memory_retrieval_limit")]
    retrieval_limit: u32,
    #[serde(default = "default_memory_max_entries")]
    max_entries: u32,
    #[serde(default = "default_memory_prioritize_relationship")]
    prioritize_relationship: bool,
    #[serde(default = "default_memory_prioritize_episodic")]
    prioritize_episodic: bool,
    #[serde(default = "default_memory_use_emotional_snapshots")]
    use_emotional_snapshots: bool,
}

fn default_memory_enabled() -> bool {
    true
}

fn default_memory_retrieval_limit() -> u32 {
    8
}

fn default_memory_max_entries() -> u32 {
    120
}

fn default_memory_prioritize_relationship() -> bool {
    true
}

fn default_memory_prioritize_episodic() -> bool {
    true
}

fn default_memory_use_emotional_snapshots() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct CompanionConfig {
    #[serde(default)]
    soul: SoulConfig,
    #[serde(default)]
    relationship_defaults: RelationshipDefaults,
    #[serde(default)]
    memory: CompanionMemoryConfig,
    #[serde(default)]
    prompting: CompanionPromptingConfig,
}

#[derive(Debug, Clone)]
struct SignalBundle {
    signals: Vec<String>,
    delta: EmotionVector,
    relationship_delta: RelationshipDelta,
    confidence: f64,
}

#[derive(Debug, Clone, Default)]
struct RelationshipDelta {
    closeness: f64,
    trust: f64,
    affection: f64,
    tension: f64,
    stability: f64,
}

pub fn is_companion_mode(session: &Session, character: &Character) -> bool {
    session.mode.eq_ignore_ascii_case("companion")
        || character.mode.eq_ignore_ascii_case("companion")
}

pub fn companion_prompt_template_id(character: &Character) -> Option<String> {
    character
        .companion
        .as_ref()
        .and_then(|value| serde_json::from_value::<CompanionConfig>(value.clone()).ok())
        .and_then(|config| config.prompting.prompt_template_id)
        .filter(|value| !value.trim().is_empty())
}

pub async fn update_state_for_user_message(
    app: &AppHandle,
    session: &mut Session,
    character: &Character,
    user_message: &str,
    now: u64,
) -> bool {
    if !is_companion_mode(session, character) {
        return false;
    }

    let config = companion_config(character);
    let mut state = current_state(session, &config);
    let baseline = config.soul.baseline_affect.clone();
    let regulation = config.soul.regulation_style.clone();
    let elapsed_minutes = elapsed_minutes(state.updated_at, now);

    state.emotional_state.felt = state.emotional_state.felt.decay_toward(
        &baseline,
        elapsed_minutes,
        regulation.recovery_speed,
    );
    state.emotional_state.expressed = state.emotional_state.expressed.decay_toward(
        &baseline,
        elapsed_minutes,
        regulation.recovery_speed,
    );
    state.emotional_state.blocked = state.emotional_state.blocked.decay_toward(
        &EmotionVector::default(),
        elapsed_minutes,
        regulation.recovery_speed,
    );
    state.relationship_state.tension = clamp01(
        state.relationship_state.tension * (-elapsed_minutes / (DECAY_MINUTES * 2.0)).exp(),
    );
    state.relationship_state.stability = clamp01(
        state.relationship_state.stability
            + ((0.55 - state.relationship_state.tension) * 0.02)
            + (elapsed_minutes / 180.0).min(0.05),
    );

    let bundle = detect_signals(app, user_message).await;
    let volatility = 0.75 + regulation.volatility * 0.9;
    let delta = bundle.delta.scaled(volatility);
    let felt = state.emotional_state.felt.add(&delta).clamp();
    let expressed = regulate_expressed(&felt, &regulation);
    let blocked = felt.subtract_positive(&expressed);

    state.emotional_state.momentum = state.emotional_state.momentum.lerp(&delta, 0.45);
    state.emotional_state.felt = felt;
    state.emotional_state.expressed = expressed;
    state.emotional_state.blocked = blocked;
    state.emotional_state.active_drivers = bundle.signals.clone();
    state.emotional_state.confidence = bundle.confidence;
    state.emotional_state.updated_at = now;

    state.relationship_state.closeness =
        clamp01(state.relationship_state.closeness + bundle.relationship_delta.closeness + 0.004);
    state.relationship_state.trust =
        clamp01(state.relationship_state.trust + bundle.relationship_delta.trust);
    state.relationship_state.affection =
        clamp01(state.relationship_state.affection + bundle.relationship_delta.affection + 0.003);
    state.relationship_state.tension =
        clamp01(state.relationship_state.tension + bundle.relationship_delta.tension);
    state.relationship_state.stability =
        clamp01(state.relationship_state.stability + bundle.relationship_delta.stability);
    state.relationship_state.interaction_count += 1;
    state.relationship_state.last_interaction_at = now;
    state.active_signals = bundle.signals;
    state.updated_at = now;

    session.companion_state = serde_json::to_value(state).ok();
    true
}

pub fn render_prompt_state(session: &Session, character: &Character) -> Option<String> {
    if !is_companion_mode(session, character) {
        return None;
    }

    let config = companion_config(character);
    let state = current_state(session, &config);
    let soul = &config.soul;
    let regulation = soul.regulation_style.clone();

    let expressed = describe_top_dimensions(&state.emotional_state.expressed, 3);
    let blocked = describe_top_dimensions(&state.emotional_state.blocked, 2);
    let rel = &state.relationship_state;

    let mut lines = vec![
        format!(
            "Relationship trend: closeness {:.0}%, trust {:.0}%, affection {:.0}%, tension {:.0}%.",
            rel.closeness * 100.0,
            rel.trust * 100.0,
            rel.affection * 100.0,
            rel.tension * 100.0,
        ),
        format!(
            "Expressed tone right now: {}.",
            if expressed.is_empty() {
                "steady and low-intensity"
            } else {
                expressed.as_str()
            }
        ),
    ];

    push_soul_line(&mut lines, "Soul essence", &soul.essence);
    push_soul_line(&mut lines, "Companion voice", &soul.voice);
    push_soul_line(&mut lines, "Relational style", &soul.relational_style);
    push_soul_line(&mut lines, "Vulnerabilities", &soul.vulnerabilities);
    push_soul_line(&mut lines, "Habits", &soul.habits);
    push_soul_line(&mut lines, "Boundaries", &soul.boundaries);
    push_soul_line(
        &mut lines,
        "Companion style notes",
        &config.prompting.style_notes,
    );

    if !blocked.is_empty() {
        lines.push(format!("More strongly felt than shown: {}.", blocked));
    }

    if !state.active_signals.is_empty() {
        lines.push(format!(
            "Recent drivers: {}.",
            state.active_signals.join(", ")
        ));
    }

    if regulation.suppression >= 0.6 {
        lines.push(
            "Regulation: tends to hide direct hurt and avoids blunt emotional disclosure."
                .to_string(),
        );
    } else if regulation.emotional_transparency >= 0.65 {
        lines.push("Regulation: relatively emotionally direct when trust is present.".to_string());
    }

    if regulation.reassurance_seeking >= 0.6 && regulation.pride < 0.45 {
        lines.push("When unsettled, may seek reassurance more openly.".to_string());
    } else if regulation.pride >= 0.55 {
        lines.push("When unsettled, may avoid asking directly for reassurance.".to_string());
    }

    Some(lines.join("\n"))
}

fn push_soul_line(lines: &mut Vec<String>, label: &str, value: &str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        lines.push(format!("{}: {}.", label, trimmed));
    }
}

fn current_state(session: &Session, config: &CompanionConfig) -> CompanionSessionState {
    if let Some(raw) = &session.companion_state {
        if let Ok(parsed) = serde_json::from_value::<CompanionSessionState>(raw.clone()) {
            return parsed;
        }
    }

    CompanionSessionState {
        emotional_state: EmotionalState {
            felt: config.soul.baseline_affect.clone(),
            expressed: regulate_expressed(
                &config.soul.baseline_affect,
                &config.soul.regulation_style,
            ),
            blocked: EmotionVector::default(),
            momentum: EmotionVector::default(),
            active_drivers: Vec::new(),
            confidence: 0.5,
            updated_at: 0,
        },
        relationship_state: RelationshipState {
            closeness: config.relationship_defaults.closeness,
            trust: config.relationship_defaults.trust,
            affection: config.relationship_defaults.affection,
            tension: config.relationship_defaults.tension,
            stability: 0.5,
            interaction_count: 0,
            last_interaction_at: 0,
        },
        active_signals: Vec::new(),
        updated_at: 0,
    }
}

fn companion_config(character: &Character) -> CompanionConfig {
    character
        .companion
        .as_ref()
        .and_then(|value| serde_json::from_value::<CompanionConfig>(value.clone()).ok())
        .unwrap_or_default()
}

async fn detect_signals(app: &AppHandle, message: &str) -> SignalBundle {
    match crate::embedding::emotion::classify_text(app, message).await {
        Ok(Some(classification)) => signals_from_classification(&classification),
        Ok(None) => SignalBundle {
            signals: Vec::new(),
            delta: EmotionVector::default(),
            relationship_delta: RelationshipDelta {
                stability: 0.01,
                ..RelationshipDelta::default()
            },
            confidence: 0.2,
        },
        Err(err) => {
            log_warn(
                app,
                "companion_emotion",
                format!(
                    "emotion classifier unavailable; using neutral update: {}",
                    err
                ),
            );
            SignalBundle {
                signals: Vec::new(),
                delta: EmotionVector::default(),
                relationship_delta: RelationshipDelta {
                    stability: 0.01,
                    ..RelationshipDelta::default()
                },
                confidence: 0.2,
            }
        }
    }
}

fn signals_from_classification(classification: &EmotionClassification) -> SignalBundle {
    let mut signals = Vec::new();
    let mut delta = EmotionVector::default();
    let mut rel = RelationshipDelta::default();
    let mut applied_score = 0.0_f64;

    for item in classification.labels.iter().take(8) {
        if item.score < label_threshold(item.label.as_str()) {
            continue;
        }
        applied_score = applied_score.max(item.score as f64);
        apply_emotion_label(item, &mut signals, &mut delta, &mut rel);
    }

    if signals.is_empty() {
        rel.stability += 0.01;
    }

    let confidence = if signals.is_empty() {
        0.25
    } else {
        clamp01((classification.confidence * 0.75) + (applied_score * 0.25))
    };

    SignalBundle {
        signals,
        delta: delta.clamp_signed(),
        relationship_delta: rel,
        confidence,
    }
}

fn apply_emotion_label(
    item: &EmotionLabelScore,
    signals: &mut Vec<String>,
    delta: &mut EmotionVector,
    rel: &mut RelationshipDelta,
) {
    let score = item.score as f64;
    let label = item.label.as_str();

    match label {
        "love" => {
            push_signal(signals, "emotion:love");
            delta.warmth += 0.10 * score;
            delta.affection_intensity += 0.15 * score;
            delta.longing += 0.06 * score;
            delta.trust += 0.04 * score;
            rel.closeness += 0.035 * score;
            rel.affection += 0.055 * score;
        }
        "caring" => {
            push_signal(signals, "emotion:caring");
            delta.warmth += 0.11 * score;
            delta.trust += 0.05 * score;
            delta.calm += 0.04 * score;
            rel.closeness += 0.025 * score;
            rel.trust += 0.025 * score;
        }
        "gratitude" | "admiration" | "approval" => {
            push_signal(signals, "emotion:appreciation");
            delta.warmth += 0.08 * score;
            delta.trust += 0.07 * score;
            delta.calm += 0.035 * score;
            rel.trust += 0.03 * score;
            rel.stability += 0.025 * score;
        }
        "joy" | "amusement" | "excitement" | "optimism" => {
            push_signal(signals, "emotion:positive");
            delta.warmth += 0.07 * score;
            delta.calm += 0.035 * score;
            delta.affection_intensity += 0.04 * score;
            rel.closeness += 0.018 * score;
            rel.stability += 0.015 * score;
        }
        "desire" => {
            push_signal(signals, "emotion:desire");
            delta.longing += 0.12 * score;
            delta.affection_intensity += 0.08 * score;
            delta.vulnerability += 0.035 * score;
            rel.closeness += 0.025 * score;
            rel.affection += 0.03 * score;
        }
        "relief" => {
            push_signal(signals, "emotion:relief");
            delta.calm += 0.08 * score;
            delta.trust += 0.04 * score;
            delta.tension -= 0.05 * score;
            delta.hurt -= 0.035 * score;
            rel.stability += 0.03 * score;
            rel.tension -= 0.025 * score;
        }
        "remorse" => {
            push_signal(signals, "emotion:remorse");
            delta.warmth += 0.04 * score;
            delta.trust += 0.035 * score;
            delta.hurt -= 0.06 * score;
            delta.tension -= 0.05 * score;
            rel.trust += 0.025 * score;
            rel.tension -= 0.025 * score;
            rel.stability += 0.02 * score;
        }
        "sadness" | "grief" | "disappointment" => {
            push_signal(signals, "emotion:distress");
            delta.warmth += 0.035 * score;
            delta.vulnerability += 0.10 * score;
            delta.reassurance_need += 0.09 * score;
            delta.hurt += 0.045 * score;
            delta.calm -= 0.035 * score;
            rel.closeness += 0.012 * score;
        }
        "fear" | "nervousness" => {
            push_signal(signals, "emotion:anxiety");
            delta.vulnerability += 0.09 * score;
            delta.reassurance_need += 0.10 * score;
            delta.tension += 0.04 * score;
            delta.calm -= 0.06 * score;
            rel.stability -= 0.015 * score;
        }
        "anger" | "annoyance" | "disapproval" | "disgust" => {
            push_signal(signals, "emotion:conflict");
            delta.hurt += 0.08 * score;
            delta.irritation += 0.10 * score;
            delta.tension += 0.12 * score;
            delta.calm -= 0.08 * score;
            delta.warmth -= 0.06 * score;
            delta.trust -= 0.045 * score;
            rel.tension += 0.07 * score;
            rel.trust -= 0.035 * score;
            rel.stability -= 0.035 * score;
        }
        "embarrassment" => {
            push_signal(signals, "emotion:embarrassment");
            delta.vulnerability += 0.07 * score;
            delta.reassurance_need += 0.045 * score;
            delta.tension += 0.02 * score;
            delta.warmth += 0.02 * score;
        }
        "confusion" => {
            push_signal(signals, "emotion:uncertainty");
            delta.tension += 0.025 * score;
            delta.reassurance_need += 0.035 * score;
            delta.calm -= 0.025 * score;
        }
        "curiosity" | "realization" | "surprise" => {
            push_signal(signals, "emotion:engagement");
            delta.warmth += 0.025 * score;
            delta.vulnerability += 0.02 * score;
            rel.closeness += 0.01 * score;
        }
        "pride" => {
            push_signal(signals, "emotion:pride");
            delta.calm += 0.035 * score;
            delta.warmth += 0.025 * score;
            rel.stability += 0.015 * score;
        }
        "neutral" => {
            push_signal(signals, "emotion:neutral");
            rel.stability += 0.01 * score;
        }
        _ => {}
    }
}

fn label_threshold(label: &str) -> f32 {
    match label {
        "neutral" => 0.55,
        "love" | "caring" | "gratitude" | "remorse" | "anger" | "sadness" | "fear" => 0.18,
        _ => 0.22,
    }
}

fn regulate_expressed(felt: &EmotionVector, regulation: &RegulationStyle) -> EmotionVector {
    EmotionVector {
        warmth: felt.warmth * (0.6 + regulation.attachment_activation * 0.35),
        trust: felt.trust * 0.85,
        calm: felt.calm * (0.55 + regulation.recovery_speed * 0.4),
        vulnerability: felt.vulnerability
            * (1.0 - regulation.suppression)
            * regulation.emotional_transparency,
        longing: felt.longing
            * (0.35 + regulation.attachment_activation * 0.65)
            * (1.0 - regulation.suppression * 0.25),
        hurt: felt.hurt * (1.0 - regulation.suppression) * regulation.emotional_transparency,
        tension: felt.tension * (1.0 - regulation.recovery_speed * 0.15),
        irritation: felt.irritation * (1.0 - regulation.conflict_avoidance * 0.35),
        affection_intensity: felt.affection_intensity
            * (0.65 + regulation.emotional_transparency * 0.3),
        reassurance_need: felt.reassurance_need
            * regulation.reassurance_seeking
            * (1.0 - regulation.pride * 0.4),
    }
    .clamp()
}

fn describe_top_dimensions(vector: &EmotionVector, count: usize) -> String {
    let mut items = vec![
        ("warmth", vector.warmth),
        ("trust", vector.trust),
        ("calm", vector.calm),
        ("vulnerability", vector.vulnerability),
        ("longing", vector.longing),
        ("hurt", vector.hurt),
        ("tension", vector.tension),
        ("irritation", vector.irritation),
        ("affection", vector.affection_intensity),
        ("reassurance need", vector.reassurance_need),
    ];
    items.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let described = items
        .into_iter()
        .filter(|(_, value)| *value >= 0.08)
        .take(count)
        .map(|(label, value)| format!("{} ({:.0}%)", label, value * 100.0))
        .collect::<Vec<_>>();

    described.join(", ")
}

fn push_signal(signals: &mut Vec<String>, label: &str) {
    if !signals.iter().any(|existing| existing == label) {
        signals.push(label.to_string());
    }
}

fn elapsed_minutes(previous: u64, now: u64) -> f64 {
    if previous == 0 || now <= previous {
        0.0
    } else {
        (now - previous) as f64 / 60000.0
    }
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn clamp_signed(value: f64) -> f64 {
    value.clamp(-1.0, 1.0)
}

fn default_closeness() -> f64 {
    0.2
}

fn default_trust() -> f64 {
    0.3
}

fn default_affection() -> f64 {
    0.15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifier_labels_map_to_companion_signals() {
        let bundle = signals_from_classification(&EmotionClassification {
            confidence: 0.8,
            labels: vec![
                EmotionLabelScore {
                    label: "love".into(),
                    score: 0.82,
                },
                EmotionLabelScore {
                    label: "gratitude".into(),
                    score: 0.62,
                },
            ],
        });

        assert!(bundle.signals.iter().any(|signal| signal == "emotion:love"));
        assert!(bundle.delta.longing > 0.04);
        assert!(bundle.relationship_delta.affection > 0.04);
    }

    #[test]
    fn signed_deltas_can_reduce_negative_state() {
        let delta = EmotionVector {
            hurt: -0.4,
            tension: -0.3,
            ..EmotionVector::default()
        }
        .scaled(0.5);

        let state = EmotionVector {
            hurt: 0.5,
            tension: 0.4,
            ..EmotionVector::default()
        }
        .add(&delta);

        assert!(state.hurt < 0.5);
        assert!(state.tension < 0.4);
    }
}
