use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Syntax error: could not parse the program")]
    SyntaxError,
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    #[error("Initialization error: {0}")]
    InitializationError(String),
}
