//! The error type returned by every fallible cinnamon operation.

use reqwest::StatusCode;

/// Shorthand for `Result<T, cinnamon::Error>`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A boxed error from the network stack. Kept opaque so reqwest never leaks into the public API.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Largest error body (in bytes) kept in [`Error`] messages.
const MAX_ERROR_BODY: usize = 4 * 1024;

/// Everything that can go wrong while talking to Nightscout.
///
/// Variants map onto Nightscout's HTTP semantics so callers can branch on intent
/// (`NotFound`, `Forbidden`, …) instead of on raw status codes.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The base URL could not be parsed or joined.
    #[error("invalid Nightscout URL: {0}")]
    InvalidUrl(String),

    /// The base URL uses plain `http://` for a non-local host.
    #[error(
        "refusing to talk to `{host}` over plain HTTP; use https:// or ClientBuilder::allow_insecure_http()"
    )]
    InsecureTransport {
        /// The offending host.
        host: String,
    },

    /// Nightscout answered with a redirect. Cinnamon never follows redirects so credentials
    /// cannot be forwarded to another host.
    #[error(
        "Nightscout redirected ({status}) to `{location}`; use that URL as the base URL instead"
    )]
    Redirected {
        /// The HTTP status (301, 302, 307, 308, …).
        status: u16,
        /// The `Location` header, if any.
        location: String,
    },

    /// Missing, invalid or expired credentials.
    ///
    /// Nightscout API v1/v2 also answers 401 when valid credentials lack the required
    /// permission, so on those endpoints this can mean "forbidden" too.
    #[error("unauthorized: {message}")]
    Unauthorized {
        /// Nightscout's explanation.
        message: String,
    },

    /// Valid credentials that lack a permission (API v3 only; v1/v2 answer 401 instead).
    #[error("forbidden: {message}")]
    Forbidden {
        /// The missing permission, e.g. `api:treatments:update`, when Nightscout names it.
        permission: Option<String>,
        /// Nightscout's explanation.
        message: String,
    },

    /// The document or route does not exist.
    #[error("not found")]
    NotFound,

    /// The document was soft-deleted (API v3 `410 Gone`).
    #[error("the document has been deleted (410 Gone)")]
    Gone,

    /// The document changed after the supplied `If-Unmodified-Since` time.
    #[error("the document was modified in the meantime (412 Precondition Failed)")]
    PreconditionFailed,

    /// The document is marked read-only and cannot be changed or deleted.
    #[error("the document is read-only (422)")]
    ReadOnly,

    /// Nightscout rejected the request as malformed.
    #[error("bad request: {message}")]
    BadRequest {
        /// Nightscout's explanation, e.g. `Bad or missing utcOffset field`.
        message: String,
    },

    /// A v1/v2 write route is not registered: `API_SECRET` is shorter than 12 characters,
    /// or the `careportal` feature is disabled for treatment/activity writes.
    #[error(
        "writes are disabled on this Nightscout (API_SECRET shorter than 12 characters, or the careportal feature is disabled)"
    )]
    ApiDisabled,

    /// The operation needs an API or credential type this client/server does not have.
    #[error("{feature} is not supported: {reason}")]
    Unsupported {
        /// What was attempted.
        feature: &'static str,
        /// Why it cannot be done.
        reason: String,
    },

    /// Any other non-success response.
    #[error("Nightscout returned HTTP {status}: {message}")]
    Server {
        /// The HTTP status code.
        status: u16,
        /// Nightscout's explanation, truncated to 4 KiB.
        message: String,
    },

    /// The request exceeded the configured timeout.
    #[error("request timed out")]
    Timeout,

    /// Connection, TLS or protocol failure.
    #[error("network error: {0}")]
    Transport(#[source] BoxError),

    /// The response did not match the expected shape. `path` names the offending field.
    #[error("could not decode the response at `{path}`: {source}")]
    Decode {
        /// JSON path of the failing field, e.g. `[3].direction`.
        path: String,
        /// The underlying serde error.
        #[source]
        source: serde_json::Error,
    },

    /// A caller-supplied value was rejected before any request was made.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl Error {
    /// The HTTP status behind this error, when there is one.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        Some(match self {
            Self::Redirected { status, .. } | Self::Server { status, .. } => *status,
            Self::Unauthorized { .. } => 401,
            Self::Forbidden { .. } => 403,
            Self::NotFound | Self::ApiDisabled => 404,
            Self::Gone => 410,
            Self::PreconditionFailed => 412,
            Self::ReadOnly => 422,
            Self::BadRequest { .. } => 400,
            _ => return None,
        })
    }

    /// Whether retrying the same request later could succeed (timeouts, connection
    /// failures, 408/429/5xx).
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        match self {
            Self::Timeout | Self::Transport(_) => true,
            Self::Server { status, .. } => matches!(*status, 408 | 429 | 500 | 502 | 503 | 504),
            _ => false,
        }
    }

    pub(crate) fn from_reqwest(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout
        } else if err.is_decode() {
            Self::Transport(Box::new(err))
        } else {
            Self::Transport(Box::new(err.without_url()))
        }
    }

    /// Maps a non-success response onto an [`Error`].
    pub(crate) fn from_response(status: StatusCode, body: &[u8], is_write: bool) -> Self {
        let message = error_message(body);
        match status.as_u16() {
            400 => Self::BadRequest { message },
            401 => Self::Unauthorized { message },
            403 => Self::Forbidden {
                permission: message
                    .strip_prefix("Missing permission ")
                    .map(|p| p.trim().to_owned()),
                message,
            },
            // Express's default 404 page means the write route was never registered.
            404 if is_write && message.starts_with("Cannot ") => Self::ApiDisabled,
            404 => Self::NotFound,
            410 => Self::Gone,
            412 => Self::PreconditionFailed,
            422 => Self::ReadOnly,
            code => Self::Server {
                status: code,
                message,
            },
        }
    }

    pub(crate) fn decode(err: serde_path_to_error::Error<serde_json::Error>) -> Self {
        let path = err.path().to_string();
        Self::Decode {
            path,
            source: err.into_inner(),
        }
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Self::InvalidUrl(err.to_string())
    }
}

/// Extracts a human-readable message from a Nightscout error body.
///
/// Handles the JSON envelope `{status, message, description}`, Express's HTML error page
/// (`<pre>Cannot GET …</pre>`) and plain text, truncated to [`MAX_ERROR_BODY`].
fn error_message(body: &[u8]) -> String {
    let body = &body[..body.len().min(MAX_ERROR_BODY)];
    let text = String::from_utf8_lossy(body);

    if let Ok(serde_json::Value::Object(map)) = serde_json::from_slice::<serde_json::Value>(body) {
        let pick = |key: &str| match map.get(key) {
            Some(serde_json::Value::String(s)) if !s.is_empty() => Some(s.clone()),
            Some(v @ serde_json::Value::Object(_)) => Some(v.to_string()),
            _ => None,
        };
        return match (pick("message"), pick("description")) {
            (Some(m), Some(d)) if m != d => format!("{m} ({d})"),
            (Some(m), _) => m,
            (None, Some(d)) => d,
            (None, None) => text.trim().to_owned(),
        };
    }

    if let (Some(start), Some(end)) = (text.find("<pre>"), text.find("</pre>")) {
        if start < end {
            return text[start + 5..end].trim().to_owned();
        }
    }
    text.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_error_bodies() {
        let err = Error::from_response(
            StatusCode::FORBIDDEN,
            br#"{"status":403,"message":"Missing permission api:treatments:update"}"#,
            true,
        );
        match err {
            Error::Forbidden { permission, .. } => {
                assert_eq!(permission.as_deref(), Some("api:treatments:update"));
            }
            other => panic!("unexpected {other:?}"),
        }

        let err = Error::from_response(
            StatusCode::UNAUTHORIZED,
            br#"{"status":401,"message":"Unauthorized","description":"Invalid/Missing"}"#,
            false,
        );
        assert_eq!(
            err.to_string(),
            "unauthorized: Unauthorized (Invalid/Missing)"
        );
    }

    #[test]
    fn express_404_on_write_means_api_disabled() {
        let html =
            b"<!DOCTYPE html><html><body><pre>Cannot POST /api/v1/treatments</pre></body></html>";
        assert!(matches!(
            Error::from_response(StatusCode::NOT_FOUND, html, true),
            Error::ApiDisabled
        ));
        assert!(matches!(
            Error::from_response(StatusCode::NOT_FOUND, html, false),
            Error::NotFound
        ));
    }

    #[test]
    fn error_bodies_are_truncated() {
        let huge = vec![b'x'; 100_000];
        match Error::from_response(StatusCode::INTERNAL_SERVER_ERROR, &huge, false) {
            Error::Server { message, status } => {
                assert_eq!(status, 500);
                assert_eq!(message.len(), MAX_ERROR_BODY);
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
