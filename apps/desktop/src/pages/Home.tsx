// Home — MainShell의 메인 영역 본문. Phase 1' 추가 화면.
//
// 정책:
// - Tailscale 패턴: 게이트웨이 status pill + 추천 카드 그리드 + 자가스캔 mini summary.
// - Phase 2' 카탈로그 진입 전이라 추천 카드는 정적 시드 (Korean 1순위 EXAONE/HCX-SEED).
// - 자가스캔 결과는 props로 받음 — 향후 scanner crate IPC 연결 시 채움.
// - 디자인 토큰만 사용. 인라인 스타일 금지.

import { useTranslation } from "react-i18next";
import { StatusPill, type PillStatus } from "@lmmaster/design-system/react";

import type { GatewayState } from "../ipc/gateway";
import { SpotlightCard } from "../components/SpotlightCard";

export interface HomeProps {
  gw: GatewayState;
  /** 자가스캔 한 줄 요약 — scanner.ScanSummary.summary_korean. 없으면 안내 placeholder. */
  scanSummary?: string;
  /** 추천 카드 클릭 시 호출 — Catalog로 이동하면서 모델 ID 전달. */
  onPickModel?: (modelId: string) => void;
}

interface RecommendedModel {
  id: string;
  display_name: string;
  reason: string;
  size: string;
  badge: string;
}

const SEED_MODELS: RecommendedModel[] = [
  {
    id: "exaone:1.2b",
    display_name: "EXAONE 4.0 1.2B",
    reason: "한국어 + 영어 이중. 작은 PC에서도 잘 돌아요.",
    size: "약 800MB",
    badge: "한국어 1순위",
  },
  {
    id: "hyperclova-x-seed-text-instruct:8b",
    display_name: "HyperCLOVA-X SEED 8B",
    reason: "네이버 한국어 토크나이저로 응답이 빨라요.",
    size: "약 5GB",
    badge: "한국어",
  },
  {
    id: "qwen2.5:3b",
    display_name: "Qwen 2.5 3B",
    reason: "다국어 + 코드. 가벼운 사이즈에 안정적이에요.",
    size: "약 2GB",
    badge: "다국어",
  },
];

export function Home({ gw, scanSummary, onPickModel }: HomeProps) {
  const { t } = useTranslation();

  return (
    <div className="home-root">
      <section className="home-hero" aria-labelledby="home-hero-title">
        <h2 id="home-hero-title" className="home-hero-title">
          <span className="home-hero-accent">
            {t("home.hero.titleAccent", "함께여서")}
          </span>
          {t("home.hero.titleSuffix", " 가능한 것들")}
        </h2>
        <p className="home-hero-subtitle">{t("home.hero.subtitle")}</p>
        <GatewayPillLarge gw={gw} />
      </section>

      <section className="home-section" aria-labelledby="home-recommend-title">
        <header className="home-section-header">
          <h3 id="home-recommend-title" className="home-section-title">
            {t("home.recommend.title")}
          </h3>
          <p className="home-section-subtitle">{t("home.recommend.subtitle")}</p>
        </header>
        <div className="home-card-grid">
          {SEED_MODELS.map((m) => (
            <SpotlightCard
              key={m.id}
              className="home-model-card is-clickable"
              role="button"
              tabIndex={0}
              onClick={() => onPickModel?.(m.id)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  onPickModel?.(m.id);
                }
              }}
              aria-label={t("home.recommend.cardLabel", {
                name: m.display_name,
                reason: m.reason,
              })}
            >
              <header className="home-model-card-header">
                <span className="home-model-card-title">{m.display_name}</span>
                <span className="home-model-card-badge">{m.badge}</span>
              </header>
              <p className="home-model-card-reason">{m.reason}</p>
              <p className="home-model-card-size num">{m.size}</p>
            </SpotlightCard>
          ))}
        </div>
      </section>

      <section className="home-section" aria-labelledby="home-scan-title">
        <header className="home-section-header">
          <h3 id="home-scan-title" className="home-section-title">
            {t("home.scan.title")}
          </h3>
        </header>
        <div className="home-scan-card">
          {scanSummary ? (
            <p className="home-scan-summary">{scanSummary}</p>
          ) : (
            <p className="home-scan-empty">{t("home.scan.empty")}</p>
          )}
        </div>
      </section>
    </div>
  );
}

// ── 부속 ───────────────────────────────────────────────────────────

function GatewayPillLarge({ gw }: { gw: GatewayState }) {
  const { t } = useTranslation();
  const banner =
    gw.status === "listening"
      ? t("home.gateway-ready", { port: gw.port ?? "?" })
      : gw.status === "failed"
        ? t("home.gateway-failed", { error: gw.error ?? "" })
        : gw.status === "stopping"
          ? t("home.gateway-stopping")
          : t("home.gateway-booting");

  return (
    <StatusPill
      status={mapStatus(gw.status)}
      label={banner}
      detail={gw.port != null ? `:${gw.port}` : null}
      size="lg"
    />
  );
}

function mapStatus(s: GatewayState["status"]): PillStatus {
  switch (s) {
    case "listening":
      return "listening";
    case "failed":
      return "failed";
    case "stopping":
      return "stopping";
    case "booting":
      return "booting";
    default:
      return "idle";
  }
}
