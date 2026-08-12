use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum AppError {
    Http(reqwest::Error),
    Json(serde_json::Error),
    Io(std::io::Error),
    Other(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Http(e) => write!(f, "{}", e),
            AppError::Json(e) => write!(f, "{}", e),
            AppError::Io(e) => write!(f, "{}", e),
            AppError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for AppError {}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        AppError::Http(value)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        AppError::Json(value)
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        AppError::Io(value)
    }
}

impl From<String> for AppError {
    fn from(value: String) -> Self {
        AppError::Other(value)
    }
}

impl From<&str> for AppError {
    fn from(value: &str) -> Self {
        AppError::Other(value.to_string())
    }
}
