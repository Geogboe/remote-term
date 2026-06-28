use anyhow::ensure;
use diceware_wordlists::Wordlist;
use rand::prelude::IndexedRandom;

pub const GENERATED_WORD_COUNT: usize = 5;
pub const MAX_EXPLICIT_TOKEN_LEN: usize = 256;

pub fn generate() -> String {
    let words = Wordlist::EffLong.get_list();
    let mut rng = rand::rng();

    (0..GENERATED_WORD_COUNT)
        .map(|_| {
            words
                .choose(&mut rng)
                .expect("EFF long wordlist must not be empty")
        })
        .copied()
        .collect::<Vec<_>>()
        .join("-")
}

pub fn validate_user_supplied(token: &str) -> anyhow::Result<()> {
    ensure!(!token.is_empty(), "token must not be empty");
    ensure!(
        token.len() <= MAX_EXPLICIT_TOKEN_LEN,
        "token must be at most {MAX_EXPLICIT_TOKEN_LEN} bytes"
    );
    ensure!(
        token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')),
        "token must contain only URL-safe ASCII letters, digits, '-', '_', '.', or '~'"
    );
    Ok(())
}

pub fn is_valid(candidate: &str, expected: &str) -> bool {
    !candidate.is_empty() && candidate == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_five_eff_words() {
        let token = generate();
        let words = token.split('-').collect::<Vec<_>>();
        let wordlist = diceware_wordlists::Wordlist::EffLong.get_list();

        assert_eq!(words.len(), 5);
        assert_eq!(wordlist.len(), 7_776);
        assert!(words.iter().all(|word| wordlist.contains(word)));
    }

    #[test]
    fn explicit_tokens_must_be_url_path_safe() {
        assert!(validate_user_supplied("my-session_1").is_ok());
        assert!(validate_user_supplied("").is_err());
        assert!(validate_user_supplied("two/segments").is_err());
        assert!(validate_user_supplied("not ascii").is_err());
        assert!(validate_user_supplied("café").is_err());
        assert!(validate_user_supplied(&"a".repeat(MAX_EXPLICIT_TOKEN_LEN + 1)).is_err());
    }

    #[test]
    fn token_validation_rejects_missing_or_wrong_tokens() {
        assert!(is_valid("abc", "abc"));
        assert!(!is_valid("", "abc"));
        assert!(!is_valid("wrong", "abc"));
    }
}
