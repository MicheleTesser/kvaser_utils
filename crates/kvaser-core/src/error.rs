use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Kvaser(#[from] kvaser_api::KvaserError),

    #[error("no log selected")]
    EmptyLogSelection,

    #[error("SocketCAN command failed: {command}: {stderr}")]
    SocketCanCommand { command: String, stderr: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
