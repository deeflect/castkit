use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub code: String,
    pub path: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub ok: bool,
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            ok: true,
            errors: Vec::new(),
        }
    }

    pub fn from_errors(errors: Vec<ValidationError>) -> Self {
        Self {
            ok: errors.is_empty(),
            errors,
        }
    }
}
