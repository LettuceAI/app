use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ChatParameterProfile {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub context_length: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub repetition_penalty: Option<f64>,
    pub reasoning_mode: Option<ReasoningMode>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub reasoning_budget_tokens: Option<u32>,
    pub prompt_caching: Option<PromptCaching>,
}

impl ChatParameterProfile {
    pub fn validate(&self) -> Result<(), ParameterValidationError> {
        validate_temperature(self.temperature)?;
        validate_probability("top_p", self.top_p)?;
        validate_positive("top_k", self.top_k)?;
        validate_positive("max_output_tokens", self.max_output_tokens)?;
        validate_positive("context_length", self.context_length)?;
        validate_penalty("frequency_penalty", self.frequency_penalty)?;
        validate_penalty("presence_penalty", self.presence_penalty)?;
        if let Some(value) = self.repetition_penalty {
            if !value.is_finite() || value <= 0.0 {
                return Err(ParameterValidationError::InvalidValue("repetition_penalty"));
            }
        }
        validate_positive("reasoning_budget_tokens", self.reasoning_budget_tokens)?;
        if self.reasoning_mode == Some(ReasoningMode::Disabled)
            && (self.reasoning_effort.is_some() || self.reasoning_budget_tokens.is_some())
        {
            return Err(ParameterValidationError::InvalidValue("reasoning"));
        }
        if let Some(cache) = self.prompt_caching {
            cache.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptCaching {
    Disabled,
    Enabled { ttl_seconds: Option<u32> },
}

impl PromptCaching {
    fn validate(self) -> Result<(), ParameterValidationError> {
        if matches!(
            self,
            Self::Enabled {
                ttl_seconds: Some(0)
            }
        ) {
            return Err(ParameterValidationError::InvalidValue("prompt_cache_ttl"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ParameterOverride<T> {
    #[default]
    Inherit,
    Set(T),
    Clear,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatParameterOverrides {
    #[serde(default)]
    pub temperature: ParameterOverride<f64>,
    #[serde(default)]
    pub top_p: ParameterOverride<f64>,
    #[serde(default)]
    pub top_k: ParameterOverride<u32>,
    #[serde(default)]
    pub max_output_tokens: ParameterOverride<u32>,
    #[serde(default)]
    pub context_length: ParameterOverride<u32>,
    #[serde(default)]
    pub frequency_penalty: ParameterOverride<f64>,
    #[serde(default)]
    pub presence_penalty: ParameterOverride<f64>,
    #[serde(default)]
    pub repetition_penalty: ParameterOverride<f64>,
    #[serde(default)]
    pub reasoning_mode: ParameterOverride<ReasoningMode>,
    #[serde(default)]
    pub reasoning_effort: ParameterOverride<ReasoningEffort>,
    #[serde(default)]
    pub reasoning_budget_tokens: ParameterOverride<u32>,
}

impl Default for ChatParameterOverrides {
    fn default() -> Self {
        Self {
            temperature: ParameterOverride::Inherit,
            top_p: ParameterOverride::Inherit,
            top_k: ParameterOverride::Inherit,
            max_output_tokens: ParameterOverride::Inherit,
            context_length: ParameterOverride::Inherit,
            frequency_penalty: ParameterOverride::Inherit,
            presence_penalty: ParameterOverride::Inherit,
            repetition_penalty: ParameterOverride::Inherit,
            reasoning_mode: ParameterOverride::Inherit,
            reasoning_effort: ParameterOverride::Inherit,
            reasoning_budget_tokens: ParameterOverride::Inherit,
        }
    }
}

impl ChatParameterOverrides {
    pub fn validate(&self) -> Result<(), ParameterValidationError> {
        validate_override("temperature", &self.temperature, validate_temperature_value)?;
        validate_override("top_p", &self.top_p, validate_probability_value)?;
        validate_override("top_k", &self.top_k, validate_positive_value)?;
        validate_override(
            "max_output_tokens",
            &self.max_output_tokens,
            validate_positive_value,
        )?;
        validate_override(
            "context_length",
            &self.context_length,
            validate_positive_value,
        )?;
        validate_override(
            "frequency_penalty",
            &self.frequency_penalty,
            validate_penalty_value,
        )?;
        validate_override(
            "presence_penalty",
            &self.presence_penalty,
            validate_penalty_value,
        )?;
        validate_override(
            "repetition_penalty",
            &self.repetition_penalty,
            validate_repetition_value,
        )?;
        validate_override(
            "reasoning_budget_tokens",
            &self.reasoning_budget_tokens,
            validate_positive_value,
        )?;
        Ok(())
    }
}

fn validate_override<T, F>(
    field: &'static str,
    value: &ParameterOverride<T>,
    validate: F,
) -> Result<(), ParameterValidationError>
where
    F: Fn(&T) -> Result<(), ParameterValidationError>,
{
    if let ParameterOverride::Set(value) = value {
        validate(value).map_err(|_| ParameterValidationError::InvalidValue(field))?;
    }
    Ok(())
}

fn validate_temperature(value: Option<f64>) -> Result<(), ParameterValidationError> {
    if let Some(value) = value {
        validate_temperature_value(&value)
    } else {
        Ok(())
    }
}

fn validate_temperature_value(value: &f64) -> Result<(), ParameterValidationError> {
    if value.is_finite() && (0.0..=2.0).contains(value) {
        Ok(())
    } else {
        Err(ParameterValidationError::InvalidValue("temperature"))
    }
}

fn validate_probability(
    field: &'static str,
    value: Option<f64>,
) -> Result<(), ParameterValidationError> {
    if let Some(value) = value {
        validate_probability_value(&value)
            .map_err(|_| ParameterValidationError::InvalidValue(field))
    } else {
        Ok(())
    }
}

fn validate_probability_value(value: &f64) -> Result<(), ParameterValidationError> {
    if value.is_finite() && (0.0..=1.0).contains(value) {
        Ok(())
    } else {
        Err(ParameterValidationError::InvalidValue("probability"))
    }
}

fn validate_positive<T: Copy + PartialEq + From<u8>>(
    _field: &'static str,
    value: Option<T>,
) -> Result<(), ParameterValidationError> {
    if value.is_some_and(|value| value == T::from(0)) {
        Err(ParameterValidationError::InvalidValue("positive"))
    } else {
        Ok(())
    }
}

fn validate_positive_value<T: Copy + PartialEq + From<u8>>(
    value: &T,
) -> Result<(), ParameterValidationError> {
    if *value == T::from(0) {
        Err(ParameterValidationError::InvalidValue("positive"))
    } else {
        Ok(())
    }
}

fn validate_penalty(
    field: &'static str,
    value: Option<f64>,
) -> Result<(), ParameterValidationError> {
    if let Some(value) = value {
        validate_penalty_value(&value).map_err(|_| ParameterValidationError::InvalidValue(field))
    } else {
        Ok(())
    }
}

fn validate_penalty_value(value: &f64) -> Result<(), ParameterValidationError> {
    if value.is_finite() && (-2.0..=2.0).contains(value) {
        Ok(())
    } else {
        Err(ParameterValidationError::InvalidValue("penalty"))
    }
}

fn validate_repetition_value(value: &f64) -> Result<(), ParameterValidationError> {
    if value.is_finite() && *value > 0.0 {
        Ok(())
    } else {
        Err(ParameterValidationError::InvalidValue("repetition_penalty"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParameterValidationError {
    #[error("parameter value for {0} is invalid")]
    InvalidValue(&'static str),
}
