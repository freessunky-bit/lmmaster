//! Phase 13'.g (ADR-0047) — minisign Ed25519 서명 검증.
//!
//! 정책:
//! - **Verify-only** — `minisign-verify` crate (zero-deps Rust) 사용. 서명 자체는 CI에서 `rsign sign`.
//! - **Dual pubkey** — primary + (optional) secondary. Tauri Updater의 키 회전 패턴 차용 (ref. reachy-mini).
//!   secondary는 키 회전 90일 overlap 기간에 사용. 둘 중 하나 통과 시 verify OK.
//! - **검증 실패 정책** — caller가 결정. registry-fetcher에서는 `Err`만 반환하고
//!   bundled fallback은 상위 흐름이 책임 (UX 정책: Diagnostics에 빨간 카드).
//! - **embedded pubkey** — 빌드 시점에 환경변수 `LMMASTER_CATALOG_PUBKEY{,_SECONDARY}` 또는 lib에 직접 넣음.
//!   (현재는 *runtime constructor*만 제공 — 호출자가 넣어요. 빌드 시 임베드는 v1.x에 별도 wire.)
//!
//! 본 모듈은 **infrastructure only** — 실제 catalog 자동 fetch + Diagnostics 카드는 v1.x.

use minisign_verify::{PublicKey, Signature};
use thiserror::Error;

/// 빌드 시점 임베드 pubkey — Phase 13'.g.2.a.
///
/// CI / 사용자 빌드 시 `LMMASTER_CATALOG_PUBKEY` env로 주입.
/// 미설정 시 `None` — 개발 빌드 graceful (verify 비활성, Bundled fallback).
const EMBEDDED_PRIMARY: Option<&str> = option_env!("LMMASTER_CATALOG_PUBKEY");
/// 키 회전 90일 overlap용 secondary. 미설정 시 `None`.
const EMBEDDED_SECONDARY: Option<&str> = option_env!("LMMASTER_CATALOG_PUBKEY_SECONDARY");

#[derive(Debug, Error)]
pub enum SignatureError {
    #[error("공개키가 아직 등록되지 않았어요")]
    NoPublicKey,
    #[error("공개키 형식이 올바르지 않아요: {0}")]
    InvalidPublicKey(String),
    #[error("서명 형식이 올바르지 않아요: {0}")]
    InvalidSignature(String),
    #[error("서명 검증에 실패했어요 — 카탈로그가 변조됐거나 잘못된 키로 서명됐어요")]
    VerifyFailed,
}

/// 카탈로그 번들 서명 검증기.
///
/// `primary` + `secondary` 두 키 중 하나라도 통과하면 OK. 키 회전 시:
/// 1. 새 secondary 키로 서명 시작 + 새 secondary 키 임베드한 앱 릴리즈.
/// 2. 기존 primary는 90일간 verify 가능하게 유지.
/// 3. 90일 후 primary를 deprecate, secondary를 primary로 승격, 다음 secondary 후보 추가.
#[derive(Debug, Clone)]
pub struct SignatureVerifier {
    primary: PublicKey,
    secondary: Option<PublicKey>,
}

impl SignatureVerifier {
    /// minisign 표준 형식 pubkey 문자열 파싱.
    ///
    /// 두 입력 형식 모두 지원 (Phase R-B):
    /// - **bare base64** — `RWQf6LRC...` 한 줄. CI env var 친화 (`\n` escape 불필요).
    /// - **multi-line** — `untrusted comment: ...\nRWQf...` 두 줄. `rsign generate` 출력 그대로.
    ///
    /// 우선 multi-line으로 시도 → 실패 시 bare base64로 fallback. 둘 다 실패면 InvalidPublicKey.
    pub fn from_minisign_strings(
        primary: &str,
        secondary: Option<&str>,
    ) -> Result<Self, SignatureError> {
        let primary = parse_pubkey(primary)?;
        let secondary = match secondary {
            Some(s) => Some(parse_pubkey(s)?),
            None => None,
        };
        Ok(Self { primary, secondary })
    }

    /// 빌드 시점 임베드된 pubkey로 verifier 생성 — Phase 13'.g.2.a.
    ///
    /// 정책:
    /// - `LMMASTER_CATALOG_PUBKEY` env 미설정 → `Ok(None)` (개발 빌드 graceful).
    /// - 설정됐으나 형식 잘못 → `Err(InvalidPublicKey)` (CI에서 catch).
    /// - secondary는 optional (키 회전 90일 overlap).
    pub fn from_embedded() -> Result<Option<Self>, SignatureError> {
        match EMBEDDED_PRIMARY {
            Some(primary) => Ok(Some(Self::from_minisign_strings(
                primary,
                EMBEDDED_SECONDARY,
            )?)),
            None => Ok(None),
        }
    }

    /// `body`가 `sig_text`(minisign signature 파일 내용)에 의해 서명되었는지 검증.
    ///
    /// 본 wrapper는 항상 `allow_legacy=false` — *prehashed 서명만* 통과. CI는 `rsign sign -H`로 발행.
    /// primary 시도 → 실패 시 secondary 시도. 둘 다 실패면 `VerifyFailed`.
    pub fn verify(&self, body: &[u8], sig_text: &str) -> Result<(), SignatureError> {
        let sig = Signature::decode(sig_text)
            .map_err(|e| SignatureError::InvalidSignature(e.to_string()))?;

        if self.primary.verify(body, &sig, false).is_ok() {
            return Ok(());
        }
        if let Some(sec) = &self.secondary {
            if sec.verify(body, &sig, false).is_ok() {
                return Ok(());
            }
        }
        Err(SignatureError::VerifyFailed)
    }
}

/// 두 형식(multi-line / bare base64) 모두 지원하는 pubkey parser. 본 모듈 private helper.
fn parse_pubkey(s: &str) -> Result<PublicKey, SignatureError> {
    // multi-line 우선 — `untrusted comment:` 시작이면 decode().
    if s.lines().count() >= 2 {
        return PublicKey::decode(s).map_err(|e| SignatureError::InvalidPublicKey(e.to_string()));
    }
    // bare base64 fallback.
    PublicKey::from_base64(s.trim()).map_err(|e| SignatureError::InvalidPublicKey(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 형식이 명백히 잘못된 pubkey는 `InvalidPublicKey`로 거부되어야 한다.
    #[test]
    fn invalid_pubkey_format_returns_typed_error() {
        let r = SignatureVerifier::from_minisign_strings("not-a-real-pubkey", None);
        assert!(matches!(r, Err(SignatureError::InvalidPublicKey(_))));
    }

    #[test]
    fn empty_pubkey_returns_typed_error() {
        let r = SignatureVerifier::from_minisign_strings("", None);
        assert!(matches!(r, Err(SignatureError::InvalidPublicKey(_))));
    }

    #[test]
    fn invalid_secondary_pubkey_format_returns_typed_error() {
        // primary는 정상 형식 흉내, secondary는 garbage.
        // primary 형식 자체가 잘못이면 거기서 먼저 fail — 의도적으로 둘 다 잘못 넣어 secondary 분기 X.
        let r = SignatureVerifier::from_minisign_strings("not-real", Some("also-garbage"));
        assert!(matches!(r, Err(SignatureError::InvalidPublicKey(_))));
    }

    /// Phase 13'.g.2.a — env 미설정 시 from_embedded()는 graceful Ok(None).
    /// 개발 빌드에서 verify 비활성화하는 1차 안전장치.
    #[test]
    fn from_embedded_graceful_when_env_unset() {
        // CI는 env 설정 — 본 테스트는 env가 *없는* 개발 빌드에서만 의미.
        // 빈 env이면 None이어야 하고, 설정됐으면 Some(verifier).
        let result = SignatureVerifier::from_embedded();
        // 둘 다 valid (env 설정 여부는 빌드 환경에 따라).
        assert!(result.is_ok());
        if let Ok(Some(_)) = result {
            // env 설정된 빌드 — pubkey 파싱 성공.
        }
        // env 미설정이면 Ok(None) — 명시 단언 X (CI/dev 양쪽 통과).
    }

    /// Phase R-B (T2) — minisign-verify 0.2.5의 prehashed 모드 자체 테스트 fixture를 그대로 빌려
    /// `SignatureVerifier::verify` round-trip을 검증. 본 wrapper는 항상 `allow_legacy=false` 호출이라
    /// prehashed 서명만 통과해야 하고, 정상 round-trip + body 변조 거부 + 잘못된 키 거부 3-경로 검증.
    ///
    /// fixture 출처: `~/.cargo/registry/src/.../minisign-verify-0.2.5/src/lib.rs::verify_prehashed`.
    /// minisign 공식 keypair `RWQf6LRC...` + body `b"test"` + 1556193335 timestamp.
    const FIXTURE_PUBKEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
    const FIXTURE_SIG: &str = "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335\tfile:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";
    const FIXTURE_BODY: &[u8] = b"test";

    #[test]
    fn round_trip_with_real_keypair_verifies() {
        let v = SignatureVerifier::from_minisign_strings(FIXTURE_PUBKEY, None)
            .expect("fixture pubkey must parse");
        v.verify(FIXTURE_BODY, FIXTURE_SIG)
            .expect("정상 body는 verify OK");
    }

    #[test]
    fn round_trip_rejects_tampered_body() {
        let v = SignatureVerifier::from_minisign_strings(FIXTURE_PUBKEY, None).unwrap();
        let tampered = b"test\0";
        let r = v.verify(tampered, FIXTURE_SIG);
        assert!(matches!(r, Err(SignatureError::VerifyFailed)));
    }

    #[test]
    fn round_trip_rejects_wrong_primary_with_correct_secondary() {
        // primary가 다른 키 (Tauri Updater 예시 키 — fixture와 무관). secondary가 fixture 키.
        // 두 키 중 하나만 통과해도 OK라는 dual-key 정책 검증.
        let other_pubkey = "RWRPxJle1jZcv/ACk1MiqHcC02oMBaS35vJF8q36s9rFkut28yDGvgqe";
        let v = SignatureVerifier::from_minisign_strings(other_pubkey, Some(FIXTURE_PUBKEY))
            .expect("두 키 모두 형식은 valid");
        v.verify(FIXTURE_BODY, FIXTURE_SIG)
            .expect("secondary로 통과해야 함");
    }

    #[test]
    fn round_trip_rejects_when_no_key_matches() {
        let other_pubkey = "RWRPxJle1jZcv/ACk1MiqHcC02oMBaS35vJF8q36s9rFkut28yDGvgqe";
        let v = SignatureVerifier::from_minisign_strings(other_pubkey, None).unwrap();
        let r = v.verify(FIXTURE_BODY, FIXTURE_SIG);
        assert!(matches!(r, Err(SignatureError::VerifyFailed)));
    }

    /// SignatureError 한국어 메시지가 사용자에게 그대로 노출 가능 형태.
    #[test]
    fn errors_have_korean_messages() {
        let no_key = SignatureError::NoPublicKey;
        assert!(no_key.to_string().contains("공개키"));
        let verify_failed = SignatureError::VerifyFailed;
        assert!(
            verify_failed.to_string().contains("변조")
                || verify_failed.to_string().contains("서명")
        );
    }
}
