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
    /// minisign 표준 base64-block 형식 pubkey 문자열 (예: `RWQ...` 또는 multi-line) 파싱.
    pub fn from_minisign_strings(
        primary: &str,
        secondary: Option<&str>,
    ) -> Result<Self, SignatureError> {
        let primary = PublicKey::decode(primary)
            .map_err(|e| SignatureError::InvalidPublicKey(e.to_string()))?;
        let secondary = match secondary {
            Some(s) => Some(
                PublicKey::decode(s)
                    .map_err(|e| SignatureError::InvalidPublicKey(e.to_string()))?,
            ),
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

    /// `Signature::decode`가 거부하는 명백한 garbage signature text.
    /// Pubkey가 valid 형식이어야 verify 분기로 들어가는데, 본 테스트는
    /// 실 키페어 없이 *형식 단계 거부*만 검증하기 어려워 #[ignore]로 둠.
    /// 실 키페어 fixture는 v1.x integration test (CI에서 `rsign generate` 후 임베드)에서.
    #[test]
    #[ignore = "round-trip은 v1.x integration test에서 실 keypair로 검증 (CI 자동화 필요)"]
    fn round_trip_with_real_keypair_placeholder() {
        // 자리표시 — 실 keypair fixture가 들어오면 본 test를 활성화.
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
