//! argon2id 해시 + verify (ADR-0022 §7, OWASP 2024).
//!
//! 정책:
//! - mem 64 MB / iter 3 / parallelism 1.
//! - PHC string 형식으로 저장 (salt + params 포함).
//! - 평문은 verify 후 즉시 zeroize 책임 (caller).

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};

/// Argon2id 파라미터 — OWASP 2024 권장.
fn argon2() -> Argon2<'static> {
    let params = Params::new(64 * 1024, 3, 1, None).expect("argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

#[derive(Debug, thiserror::Error)]
pub enum HashError {
    #[error("argon2 hash 실패: {0}")]
    Hash(String),
    #[error("argon2 verify 실패")]
    Verify,
    #[error("PHC string parse 실패: {0}")]
    Parse(String),
}

/// 평문 키 → PHC argon2id 해시.
pub fn hash_key(plaintext: &str) -> Result<String, HashError> {
    let salt = SaltString::generate(&mut OsRng);
    let h = argon2()
        .hash_password(plaintext.as_bytes(), &salt)
        .map_err(|e| HashError::Hash(e.to_string()))?;
    Ok(h.to_string())
}

/// PHC string에서 평문 매칭 검증.
pub fn verify_key(phc: &str, plaintext: &str) -> Result<bool, HashError> {
    let parsed = PasswordHash::new(phc).map_err(|e| HashError::Parse(e.to_string()))?;
    match argon2().verify_password(plaintext.as_bytes(), &parsed) {
        Ok(_) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(HashError::Verify).map_err(|_| HashError::Hash(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_round_trip() {
        let plaintext = "lm-abcd1234X9Y8Z7";
        let phc = hash_key(plaintext).unwrap();
        assert!(phc.starts_with("$argon2id$"));
        assert!(verify_key(&phc, plaintext).unwrap());
    }

    #[test]
    fn verify_wrong_plaintext_returns_false() {
        let phc = hash_key("correct").unwrap();
        assert!(!verify_key(&phc, "wrong").unwrap());
    }

    #[test]
    fn hash_different_each_time_due_to_salt() {
        let a = hash_key("x").unwrap();
        let b = hash_key("x").unwrap();
        assert_ne!(a, b);
        // 그러나 둘 다 같은 평문 verify.
        assert!(verify_key(&a, "x").unwrap());
        assert!(verify_key(&b, "x").unwrap());
    }

    #[test]
    fn invalid_phc_returns_parse_error() {
        let r = verify_key("not a phc string", "x");
        assert!(matches!(r, Err(HashError::Parse(_))));
    }
}
