use thiserror::Error;

/// Crate-wide error representation for zca-rs.
#[derive(Error, Debug)]
pub enum ZaloError {
    /// Errors returned from the Zalo server APIs
    #[error("Zalo API Error: {message} (code: {code:?})")]
    ApiError { message: String, code: Option<i32> },
    /// QR Login aborted by caller
    #[error("Zalo QR Login Aborted")]
    LoginQRAborted,
    /// QR Login declined on the phone
    #[error("Zalo QR Login Declined")]
    LoginQRDeclined,
    /// Missing dimensions/metadata helper for attachment uploading
    #[error("Missing image metadata getter callback")]
    MissingImageMetadataGetter,
    /// Upstream HTTP/network request errors
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    /// Real-time WS socket connection errors
    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    /// Serialization or deserialization failures
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// AES-CBC or AES-GCM operations error
    #[error("Crypto error: {0}")]
    Crypto(String),
    /// Local filesystem file readings
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Parsing invalid URLs
    #[error("URL parsing error: {0}")]
    UrlParse(#[from] url::ParseError),
    /// Protocol frame format errors
    #[error("Payload format error: {0}")]
    PayloadFormat(String),
    /// Zlib/Decompress inflate failures
    #[error("Decompression error: {0}")]
    Decompression(String),
}

impl From<tokio_tungstenite::tungstenite::Error> for ZaloError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        ZaloError::WebSocket(Box::new(err))
    }
}
