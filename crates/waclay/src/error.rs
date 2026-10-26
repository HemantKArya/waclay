//! Unified error type for every engine backend and the Canonical ABI layer.

/// Errors produced anywhere in waclay: engine backends, component decoding,
/// the Canonical ABI, WASI glue and user host code.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A backend engine refused or failed during an operation.
    #[error("engine `{engine}`: {msg}")]
    Engine { engine: &'static str, msg: String },

    /// The binary handed to us is not a valid wasm module / component.
    #[error("malformed binary: {0}")]
    MalformedBinary(String),

    /// Feature not (yet) supported by this version of waclay.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// A required import was not provided by the [`Linker`](crate::Linker).
    #[error("missing import `{module}` `{name}`")]
    MissingImport { module: String, name: String },

    /// Signature / layout mismatch between host and guest.
    #[error("type mismatch: {0}")]
    TypeMismatch(String),

    /// A wasm trap surfaced from engine or host code.
    #[error("wasm trap: {0}")]
    Trap(String),

    /// A component value did not match the expected WIT type.
    #[error("invalid value: {0}")]
    InvalidValue(String),

    /// Resource handle problems (use-after-drop, foreign table, ...).
    #[error("resource error: {0}")]
    Resource(String),

    /// User host callback returned an error; routed through as a trap.
    #[error("host error: {0}")]
    Host(String),
}

impl Error {
    /// Build a trap error (used by host functions that must abort a wasm call).
    pub fn trap(msg: impl Into<String>) -> Error {
        Error::Trap(msg.into())
    }

    /// Build an engine-flavoured error.
    pub fn engine(engine: &'static str, err: impl std::fmt::Display) -> Error {
        Error::Engine {
            engine,
            msg: err.to_string(),
        }
    }

    /// Build an invalid-value error.
    pub fn value(msg: impl Into<String>) -> Error {
        Error::InvalidValue(msg.into())
    }

    /// Build an unsupported-feature error.
    pub fn unsupported(msg: impl Into<String>) -> Error {
        Error::Unsupported(msg.into())
    }
}

/// Convenience `Result` alias used across waclay.
pub type Result<T> = std::result::Result<T, Error>;

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::value(format!("invalid utf-8 string: {e}"))
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Error::value(format!("invalid utf-8 string: {e}"))
    }
}
