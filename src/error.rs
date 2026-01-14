use thiserror::Error;

#[derive(Error, Debug)]
pub enum NightscoutError {
    #[error("Invalid URL format: {0}")]
    UrlParseError(#[from] url::ParseError),

    #[error("Network or HTTP error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Failed to parse JSON response: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Nightscout API Error {status}: {message}")]
    ApiError {
        status: reqwest::StatusCode,
        message: String,
    },

    #[error("Authentication failed: API secret is missing or invalid")]
    AuthError,

    #[error("No data found")]
    NotFound,

    #[error("Unknown error occurred")]
    Unknown,
}