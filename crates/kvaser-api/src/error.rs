use thiserror::Error;

pub type Result<T> = std::result::Result<T, KvaserError>;

#[derive(Debug, Error)]
pub enum KvaserError {
    #[error("cannot load Kvaser library {library}; tried {tried}: {source}")]
    LibraryLoad {
        library: &'static str,
        tried: String,
        #[source]
        source: libloading::Error,
    },

    #[error("cannot load symbol {symbol} from {library}: {source}")]
    SymbolLoad {
        library: &'static str,
        symbol: &'static str,
        #[source]
        source: libloading::Error,
    },

    #[error("{library} {operation} failed with status {code}: {message}")]
    Status {
        library: &'static str,
        operation: &'static str,
        code: i32,
        message: String,
    },

    #[error("invalid CAN bitrate: {0}")]
    InvalidBitrate(String),

    #[error("invalid Kvaser logger config: {0}")]
    Config(String),

    #[error("path contains an interior NUL byte")]
    PathContainsNul(#[from] std::ffi::NulError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
