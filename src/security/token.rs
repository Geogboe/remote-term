use rand::distr::{Alphanumeric, SampleString};

pub const DEFAULT_TOKEN_LEN: usize = 32;

pub fn generate() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), DEFAULT_TOKEN_LEN)
}

pub fn is_valid(candidate: &str, expected: &str) -> bool {
    !candidate.is_empty() && candidate == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_high_entropy_url_safe_strings() {
        let token = generate();
        assert_eq!(token.len(), DEFAULT_TOKEN_LEN);
        assert!(token.chars().all(|ch| ch.is_ascii_alphanumeric()));
    }

    #[test]
    fn token_validation_rejects_missing_or_wrong_tokens() {
        assert!(is_valid("abc", "abc"));
        assert!(!is_valid("", "abc"));
        assert!(!is_valid("wrong", "abc"));
    }
}
