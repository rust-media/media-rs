use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum MediaError {
    #[error("Failed: {0}")]
    Failed(String),
    #[error("Invalid: {0}")]
    Invalid(String),
    #[error("Again: {0}")]
    Again(String),
    #[error("Canceled: {0}")]
    Canceled(String),
    #[error("Creation failed: {0}")]
    CreationFailed(String),
    #[error("Invalid parameter: {0} {1}")]
    InvalidParameter(String, String),
    #[error("Not implemented")]
    NotImplemented,
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Unsupported: {0}")]
    Unsupported(String),
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
}

#[macro_export]
macro_rules! invalid_param_error {
    ($param:expr) => {
        MediaError::InvalidParameter(stringify!($param).to_string(), format!("{:?}", $param).to_string())
    };
}

#[macro_export]
macro_rules! none_param_error {
    ($param:expr) => {
        MediaError::InvalidParameter(stringify!($param).to_string(), stringify!(None).to_string())
    };
}

#[macro_export]
macro_rules! not_found_error {
    ($param:expr) => {
        MediaError::NotFound(format!("{:?}", $param).to_string())
    };
}

#[macro_export]
macro_rules! unsupported_error {
    ($param:expr) => {
        MediaError::Unsupported(stringify!($param).to_string())
    };
}
