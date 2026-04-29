//! 평문 키 생성 — 형식: `lm-{prefix8}{secret24}` (전체 35자).
//!
//! 정책 (ADR-0022 §7):
//! - prefix 8자 = DB 인덱스 lookup용 (충돌 가능, narrow 후 argon2 verify).
//! - secret 24자 = 무작위 entropy (한정된 prefix space에서 충돌 무관).
//! - alphabet = base32-like (A-Z + 2-9, lowercase 변환). 모호 문자 (0/O/1/I/l) 제외.

use rand::distributions::{Distribution, Uniform};
use rand::rngs::OsRng;

/// base32 alphabet — 모호 문자 (0, O, 1, I, l, L) 제외.
const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";

const PREFIX_LEN: usize = 8;
const SECRET_LEN: usize = 24;

/// 발급 결과 — caller가 분리해서 DB(prefix만) + 응답(plaintext)으로 사용.
pub struct GeneratedKey {
    /// "lm-{prefix8}{secret24}" 전체 평문 — 1회만 사용자에게 노출.
    pub plaintext: String,
    /// "lm-{prefix8}" — DB lookup index + UI 표시용.
    pub prefix: String,
}

/// 새 평문 키 생성 — OS CSPRNG 사용.
pub fn generate() -> GeneratedKey {
    let mut rng = OsRng;
    let dist = Uniform::from(0..ALPHABET.len());

    let mut chars: Vec<u8> = (0..(PREFIX_LEN + SECRET_LEN))
        .map(|_| ALPHABET[dist.sample(&mut rng)])
        .collect();

    let secret_part = chars.split_off(PREFIX_LEN);
    let prefix_part = chars;

    let prefix = format!(
        "lm-{}",
        std::str::from_utf8(&prefix_part).expect("alphabet utf8")
    );
    let plaintext = format!(
        "{}{}",
        prefix,
        std::str::from_utf8(&secret_part).expect("alphabet utf8")
    );

    GeneratedKey { plaintext, prefix }
}

/// 평문에서 prefix(첫 11자, "lm-" + 8자) 추출 — verify 흐름에서 인덱스 lookup용.
pub fn prefix_of(plaintext: &str) -> Option<String> {
    if !plaintext.starts_with("lm-") {
        return None;
    }
    if plaintext.len() < 3 + PREFIX_LEN {
        return None;
    }
    Some(plaintext[..3 + PREFIX_LEN].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_has_correct_format() {
        let k = generate();
        assert!(k.plaintext.starts_with("lm-"));
        assert_eq!(k.prefix.len(), 3 + PREFIX_LEN); // "lm-" + 8
        assert_eq!(k.plaintext.len(), 3 + PREFIX_LEN + SECRET_LEN); // 35
        assert!(k.plaintext.starts_with(&k.prefix));
    }

    #[test]
    fn generate_alphabet_excludes_ambiguous() {
        let k = generate();
        // "lm-" prefix는 검사 제외 (브랜드 prefix는 'l'을 의도적으로 포함).
        // 그 뒤 본문에서만 ambiguous 문자 없는지 — 0/1/O/I/L/l.
        let body = &k.plaintext[3..];
        for c in body.chars() {
            assert!(
                !matches!(c, '0' | '1' | 'O' | 'I' | 'L' | 'l'),
                "body must not contain ambiguous char: {c}"
            );
        }
    }

    #[test]
    fn generate_is_random() {
        let a = generate();
        let b = generate();
        assert_ne!(a.plaintext, b.plaintext);
        assert_ne!(a.prefix, b.prefix);
    }

    #[test]
    fn prefix_of_extracts_first_eleven() {
        let k = generate();
        let p = prefix_of(&k.plaintext).unwrap();
        assert_eq!(p, k.prefix);
    }

    #[test]
    fn prefix_of_rejects_non_lm_prefix() {
        assert!(prefix_of("sk-abcdef1234").is_none());
        assert!(prefix_of("").is_none());
    }

    #[test]
    fn prefix_of_rejects_too_short() {
        assert!(prefix_of("lm-abc").is_none());
    }
}
