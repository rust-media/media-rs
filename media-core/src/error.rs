use std::borrow::Cow;

use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("Failed: {0}")]
    Failed(Cow<'static, str>),
    #[error("Invalid: {0}")]
    Invalid(Cow<'static, str>),
    #[error("Again: {0}")]
    Again(Cow<'static, str>),
    #[error("Canceled: {0}")]
    Canceled(Cow<'static, str>),
    #[error("Creation failed: {0}")]
    CreationFailed(Cow<'static, str>),
    #[error("Invalid parameter: {0} {1}")]
    InvalidParameter(Cow<'static, str>, Cow<'static, str>),
    #[error("Not implemented")]
    NotImplemented,
    #[error("Not found: {0}")]
    NotFound(Cow<'static, str>),
    #[error("Unsupported: {0}")]
    Unsupported(Cow<'static, str>),
    #[error("Initialization failed: {0}")]
    InitializationFailed(Cow<'static, str>),
    #[error("Open failed: {0}")]
    OpenFailed(Cow<'static, str>),
    #[error("Close failed: {0}")]
    CloseFailed(Cow<'static, str>),
    #[error("Start failed: {0}")]
    StartFailed(Cow<'static, str>),
    #[error("Stop failed: {0}")]
    StopFailed(Cow<'static, str>),
    #[error("Not running: {0}")]
    NotRunning(Cow<'static, str>),
    #[error("Get failed: {0}")]
    GetFailed(Cow<'static, str>),
    #[error("Set failed: {0}")]
    SetFailed(Cow<'static, str>),
    #[error("Read failed: {0}")]
    ReadFailed(Cow<'static, str>),
    #[error("Write failed: {0}")]
    WriteFailed(Cow<'static, str>),
}

#[macro_export]
macro_rules! invalid_error {
    ($param:literal) => {
        $crate::error::Error::Invalid($param.into())
    };
    ($param:expr) => {
        $crate::error::Error::Invalid(format!("{:?}", $param).into())
    };
}

#[macro_export]
macro_rules! failed_error {
    ($param:literal) => {
        $crate::error::Error::Failed($param.into())
    };
    ($param:expr) => {
        $crate::error::Error::Failed(format!("{:?}", $param).into())
    };
}

#[macro_export]
macro_rules! invalid_param_error {
    ($param:expr) => {
        $crate::error::Error::InvalidParameter(stringify!($param).into(), format!("{:?}", $param).into())
    };
}

#[macro_export]
macro_rules! none_param_error {
    ($param:expr) => {
        $crate::error::Error::InvalidParameter(stringify!($param).into(), stringify!(None).into())
    };
}

#[macro_export]
macro_rules! not_found_error {
    ($param:literal) => {
        $crate::error::Error::NotFound($param.into())
    };
    ($param:expr) => {
        $crate::error::Error::NotFound(format!("{:?}", $param).into())
    };
}

#[macro_export]
macro_rules! unsupported_error {
    ($param:literal) => {
        $crate::error::Error::Unsupported($param.into())
    };
    ($param:expr) => {
        $crate::error::Error::Unsupported(format!("{:?}", $param).into())
    };
}
