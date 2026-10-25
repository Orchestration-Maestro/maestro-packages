//! Retry policy for the HTTP client: the attempt budget, the backoff curve and
//! the retry loop shared by [`crate::request`] and [`crate::request_with_attempts`].

use std::time::Duration;

use crate::Response;

/// Default attempt budget for [`crate::request`]: the first try plus two
/// retries on transient failures.
pub const DEFAULT_ATTEMPTS: u32 = 3;

/// Largest attempt budget a caller may request; larger values are clamped.
const MAX_ATTEMPTS: u32 = 5;

/// Base delay between attempts in milliseconds; doubles per retry.
const RETRY_BASE_DELAY_MS: u64 = 250;
/// Cap on the delay between attempts in milliseconds.
const RETRY_MAX_DELAY_MS: u64 = 4000;
/// Cap on a server-supplied `Retry-After` in seconds.
const RETRY_AFTER_CAP_SECS: u64 = 30;

/// Status codes safe to retry: rate limits and server errors.
fn retryable_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

/// Exponential backoff between attempts: 250 ms, 500 ms, 1 s, … capped at
/// four seconds.
fn backoff_delay(attempt: u32) -> Duration {
    let shift = (attempt - 1).min(4);
    let ms = RETRY_BASE_DELAY_MS
        .saturating_mul(1u64 << shift)
        .min(RETRY_MAX_DELAY_MS);
    Duration::from_millis(ms)
}

/// Run `attempt` up to `attempts` times (clamped to 1..=5), retrying any
/// transport error and 429/5xx statuses with exponential backoff. A
/// `Retry-After` header in seconds is honored, capped.
pub fn run_with_retry<F>(attempts: u32, mut attempt: F) -> Result<Response, String>
where
    F: FnMut() -> Result<Response, String>,
{
    let budget = attempts.clamp(1, MAX_ATTEMPTS);
    let mut n = 0u32;
    loop {
        n += 1;
        let outcome = attempt();
        let retryable = match &outcome {
            Err(_) => true,
            Ok(response) => retryable_status(response.status),
        };
        if !retryable || n >= budget {
            return outcome;
        }
        let delay = match &outcome {
            Err(_) => backoff_delay(n),
            Ok(response) => response.retry_after.map_or_else(
                || backoff_delay(n),
                |secs| Duration::from_secs(secs.min(RETRY_AFTER_CAP_SECS)),
            ),
        };
        std::thread::sleep(delay);
    }
}

#[cfg(test)]
mod tests {
    use super::backoff_delay;
    use std::time::Duration;

    /// The backoff doubles from 250 ms and caps at 4 s.
    #[test]
    fn backoff_doubles_then_caps() {
        assert_eq!(backoff_delay(1), Duration::from_millis(250));
        assert_eq!(backoff_delay(2), Duration::from_millis(500));
        assert_eq!(backoff_delay(3), Duration::from_millis(1000));
        assert_eq!(backoff_delay(4), Duration::from_millis(2000));
        assert_eq!(backoff_delay(5), Duration::from_millis(4000));
        assert_eq!(backoff_delay(6), Duration::from_millis(4000));
    }
}
