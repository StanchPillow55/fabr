//! Failure classification for the poll loop.
//!
//! Pipeline errors fall into two buckets that drive very different handling in
//! `Runner::process`: transient failures that should be retried with bounded
//! backoff, and terminal failures that should stop retrying *that cause* and be
//! surfaced for human correction through the `Data_API`.
//!
//! Anthropic failures arrive as `FabrError::Remote(format!("anthropic {code}: {text}"))`
//! from `model::AnthropicModel::post`, and raw network errors as
//! `FabrError::Remote(<ureq error>)` (no `anthropic {code}:` prefix).

use fabr_core::FabrError;

/// How the poll loop should react to a pipeline error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// Transient failure: apply bounded backoff and try again later.
    Backoff,
    /// Terminal failure: stop retrying this cause and surface it for correction.
    Terminal,
}

/// Classify a pipeline error as transient (`Backoff`) or terminal (`Terminal`).
///
/// Transient: network errors, HTTP 5xx, HTTP 429.
/// Terminal: HTTP 401/403, invalid configuration, corrupted/unreadable record,
/// workspace-safety refusals (unsupported tools, `approval_allows_execute`
/// rejection).
pub fn classify(err: &FabrError) -> Retry {
    match err {
        // Remote failures carry the Anthropic status code (when the HTTP call
        // reached the API) or a raw transport error (when it did not). Parse the
        // status; absence of a status means a network/transport failure, which
        // is transient.
        FabrError::Remote(msg) => match anthropic_status(msg) {
            Some(code) => classify_status(code),
            None => Retry::Backoff,
        },
        // A transport timeout is a transient network condition.
        FabrError::Timeout(_) => Retry::Backoff,
        // Optimistic-concurrency conflicts resolve on retry.
        FabrError::Conflict { .. } => Retry::Backoff,
        // Invalid configuration and workspace-safety refusals (unsupported
        // tools, approval rejections) surface as BadRequest/Unprocessable and
        // will not fix themselves on retry.
        FabrError::BadRequest(_) | FabrError::Unprocessable(_) | FabrError::NotFound(_) => Retry::Terminal,
        // A corrupted or unreadable record is terminal until the record is fixed.
        FabrError::Io(_) => Retry::Terminal,
    }
}

/// HTTP status classification for Anthropic responses.
fn classify_status(code: u16) -> Retry {
    match code {
        429 => Retry::Backoff,        // rate limited
        500..=599 => Retry::Backoff,  // server-side, transient
        401 | 403 => Retry::Terminal, // invalid / forbidden key
        _ => Retry::Terminal,         // other 4xx: bad request, not retryable
    }
}

/// Extract the numeric status from an `"anthropic {code}: ..."` message.
///
/// Returns `None` for raw transport errors, which do not carry the prefix.
fn anthropic_status(msg: &str) -> Option<u16> {
    let rest = msg.strip_prefix("anthropic ")?;
    let code = rest.split([':', ' ']).next()?;
    code.parse::<u16>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(s: &str) -> FabrError {
        FabrError::Remote(s.to_string())
    }

    #[test]
    fn rate_limit_is_transient() {
        assert_eq!(classify(&remote("anthropic 429: slow down")), Retry::Backoff);
    }

    #[test]
    fn server_errors_are_transient() {
        assert_eq!(classify(&remote("anthropic 500: oops")), Retry::Backoff);
        assert_eq!(classify(&remote("anthropic 503: unavailable")), Retry::Backoff);
    }

    #[test]
    fn auth_failures_are_terminal() {
        assert_eq!(classify(&remote("anthropic 401: invalid key")), Retry::Terminal);
        assert_eq!(classify(&remote("anthropic 403: forbidden")), Retry::Terminal);
    }

    #[test]
    fn other_client_errors_are_terminal() {
        assert_eq!(classify(&remote("anthropic 400: bad request")), Retry::Terminal);
    }

    #[test]
    fn raw_network_errors_are_transient() {
        assert_eq!(classify(&remote("dns error: failed to lookup address")), Retry::Backoff);
        assert_eq!(classify(&remote("Connection refused (os error 61)")), Retry::Backoff);
    }

    #[test]
    fn timeout_is_transient() {
        assert_eq!(classify(&FabrError::Timeout("deadline".into())), Retry::Backoff);
    }

    #[test]
    fn conflict_is_transient() {
        assert_eq!(classify(&FabrError::Conflict { message: "stale".into(), node: None }), Retry::Backoff);
    }

    #[test]
    fn config_and_safety_refusals_are_terminal() {
        assert_eq!(classify(&FabrError::BadRequest("unsupported tool".into())), Retry::Terminal);
        assert_eq!(classify(&FabrError::Unprocessable("approval_allows_execute rejected".into())), Retry::Terminal);
    }

    #[test]
    fn corrupted_record_is_terminal() {
        assert_eq!(classify(&FabrError::Io("record.md: invalid frontmatter".into())), Retry::Terminal);
    }

    #[test]
    fn status_parsing_handles_colon_and_space() {
        assert_eq!(anthropic_status("anthropic 429: text"), Some(429));
        assert_eq!(anthropic_status("anthropic 503 text"), Some(503));
        assert_eq!(anthropic_status("some ureq error"), None);
    }
}
