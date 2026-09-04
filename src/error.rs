use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("protocol error: {message}")]
    Protocol {
        code: String,
        path: String,
        message: String,
    },
    #[error("validation error: {message}")]
    Validation {
        code: String,
        path: String,
        message: String,
    },
    #[error("recurrence error: {message}")]
    Recurrence {
        code: String,
        path: String,
        message: String,
    },
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &str {
        match self {
            Self::Protocol { code, .. }
            | Self::Validation { code, .. }
            | Self::Recurrence { code, .. } => code,
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::Protocol { path, .. }
            | Self::Validation { path, .. }
            | Self::Recurrence { path, .. } => path,
            Self::Internal(_) => "",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Protocol { message, .. }
            | Self::Validation { message, .. }
            | Self::Recurrence { message, .. } => message.clone(),
            Self::Internal(message) => message.clone(),
        }
    }

    pub fn protocol(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Protocol {
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn validation(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Validation {
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn recurrence(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Recurrence {
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireError {
    pub code: String,
    pub field_path: String,
    pub message: String,
}

impl From<&AppError> for WireError {
    fn from(error: &AppError) -> Self {
        Self {
            code: error.code().to_string(),
            field_path: error.path().to_string(),
            message: error.message(),
        }
    }
}
