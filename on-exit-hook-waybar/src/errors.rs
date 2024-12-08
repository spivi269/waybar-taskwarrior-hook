use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TaskHookWaybarError {
    #[error("Failed to determine cache directory")]
    SetLogger(#[from] log::SetLoggerError),
    #[error("File error: {0}")]
    File(#[from] std::io::Error),
    #[error("Error: No processes found")]
    ProcessNotFound,
    #[error("Process error: {0}")]
    Proc(#[from] procfs::ProcError),
    #[error("Signal out of bounds: {0}")]
    InvalidRTSignal(#[from] InvalidRTSignalError),
    #[error("Json processing error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum InvalidRTSignalError {
    #[error("Signal below minimum: {context}")]
    BelowMinError { context: String },
    #[error("Signal above maximum: {context}")]
    AboveMaxError { context: String },
}
