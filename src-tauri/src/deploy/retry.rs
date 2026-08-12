const IDEMPOTENT_REMOTE_ATTEMPTS: usize = 3;

/// Retries only operations whose provider contract is idempotent: reads,
/// content-addressed writes, same-key overwrites and deletes. Deployment
/// creation is deliberately excluded because retrying it can create a second
/// version after an ambiguous response.
pub(crate) fn retry_idempotent<T>(
    mut operation: impl FnMut() -> Result<T, String>,
) -> Result<T, String> {
    let mut last_error = None;
    for attempt in 0..IDEMPOTENT_REMOTE_ATTEMPTS {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) => last_error = Some(error),
        }
        if attempt + 1 < IDEMPOTENT_REMOTE_ATTEMPTS {
            retry_delay(attempt);
        }
    }
    Err(last_error.unwrap_or_else(|| "Operația remote nu a fost executată.".to_string()))
}

#[cfg(not(test))]
fn retry_delay(attempt: usize) {
    std::thread::sleep(std::time::Duration::from_millis(100u64 << attempt));
}

#[cfg(test)]
fn retry_delay(_: usize) {}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn succeeds_after_a_transient_failure() {
        let attempts = Cell::new(0);
        let value = retry_idempotent(|| {
            attempts.set(attempts.get() + 1);
            if attempts.get() < 2 {
                Err("transient".to_string())
            } else {
                Ok(42)
            }
        })
        .unwrap();
        assert_eq!(value, 42);
        assert_eq!(attempts.get(), 2);
    }

    #[test]
    fn returns_the_last_error_after_the_bound() {
        let attempts = Cell::new(0);
        let error = retry_idempotent::<()>(|| {
            attempts.set(attempts.get() + 1);
            Err(format!("failure-{}", attempts.get()))
        })
        .unwrap_err();
        assert_eq!(attempts.get(), IDEMPOTENT_REMOTE_ATTEMPTS);
        assert_eq!(error, "failure-3");
    }
}
