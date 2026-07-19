use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

pub(super) const ACCESS_TOKEN_HEADER: &str = "x-pana-studio-token";
const ACCESS_TOKEN_BYTES: usize = 32;
const ACCESS_TOKEN_ENCODED_LEN: usize = 43;

pub(super) fn generate_access_token() -> Result<String, String> {
    let mut bytes = [0_u8; ACCESS_TOKEN_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| format!("Nu am putut genera tokenul MCP din sursa OS: {error}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(super) fn is_valid_access_token(value: &str) -> bool {
    value.len() == ACCESS_TOKEN_ENCODED_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(super) fn access_tokens_equal(candidate: &str, expected: &str) -> bool {
    if candidate.len() != expected.len() {
        return false;
    }
    candidate
        .as_bytes()
        .iter()
        .zip(expected.as_bytes())
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_access_tokens_have_the_canonical_shape() {
        let first = generate_access_token().unwrap();
        let second = generate_access_token().unwrap();

        assert!(is_valid_access_token(&first));
        assert!(is_valid_access_token(&second));
        assert_ne!(first, second);
    }

    #[test]
    fn access_token_comparison_rejects_length_and_content_differences() {
        let token = "a".repeat(ACCESS_TOKEN_ENCODED_LEN);

        assert!(access_tokens_equal(&token, &token));
        assert!(!access_tokens_equal(
            &token,
            &"b".repeat(ACCESS_TOKEN_ENCODED_LEN)
        ));
        assert!(!access_tokens_equal(&token, "short"));
    }
}
