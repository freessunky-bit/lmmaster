// SplashScreen — 첫 실행 시 1.2~1.5s 환상적인 connection 연출.
//
// 컨셉 (Phase 14' v2 디자인 결정 2026-05-04):
// - 6 outer node (6 pillar 메타: 자동설치 / 한국어 / 포터블 / 큐레이션 / 워크벤치 / 자동갱신).
// - 60° 간격 orbital. radius 130, viewBox 400×400.
// - stagger fade-in 70ms → connection line stroke draw 외→중앙 → BrandMark spring scale.
// - 1.55s 후 호흡 루프 (PulseGlow + Node opacity 미세 sine).
// - ready 신호 도착 + 최소 1.2s 충족 시 350ms ease-out fade dismiss.
// - 최대 5s 강제 dismiss (ready 안 와도). Esc로 즉시 skip.
// - prefers-reduced-motion 자동 정적 (CSS animation 0ms).
//
// 정책 (CLAUDE.md §4.3 + Phase 14'):
// - lucide / Brand.tsx 재사용 (코드 중복 X). connection line은 별개 SVG 안에서.
// - 외부 brand mark (BrandMark)는 SVG 위에 absolute 배치 (z-index 위) — line이 자연스럽게 BrandMark
//   뒤로 수렴되는 시각.
// - role="status" aria-live polite — 스크린리더 친화.

import { motion, AnimatePresence, useReducedMotion } from "framer-motion";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { BrandMark } from "../Brand";
import "./splash.css";

export interface SplashScreenProps {
  /**
   * Gateway / 카탈로그 / probe 모두 ready 시 true. 부모가 결정.
   * default true — gw 동기화 X 시 최소 시간만 보고 dismiss (가장 단순 흐름).
   * v1.x 후속: 부모가 gw.status === 'listening' 시점 전달로 정확한 ready 동기.
   */
  ready?: boolean;
  /** 최소 표시 시간 (ms). 너무 빠른 dismiss 방지 — 사용자가 보기 좋게. */
  minDurationMs?: number;
  /** 최대 표시 시간 (ms). ready 안 와도 강제 dismiss. */
  maxDurationMs?: number;
  /** dismiss 완료 콜백 (parent unmount 신호). */
  onComplete?: () => void;
}

/**
 * 6 pillar 노드 — 12시 시작 시계방향 60° 간격.
 * label은 디버그 + a11y 용. 시각엔 노출 X (시각 깔끔 + 엘리트 톤).
 */
const PILLAR_NODES = [
  { angleDeg: -90, key: "auto-install" },
  { angleDeg: -30, key: "korean" },
  { angleDeg: 30, key: "portable" },
  { angleDeg: 90, key: "curation" },
  { angleDeg: 150, key: "workbench" },
  { angleDeg: 210, key: "auto-update" },
];

const SVG_SIZE = 400;
const CENTER = SVG_SIZE / 2; // 200
const ORBIT_RADIUS = 130;
const NODE_RADIUS = 6;
const INNER_RADIUS = 38; // BrandMark 외각 — line 끝점.

function nodePos(angleDeg: number) {
  const rad = (angleDeg * Math.PI) / 180;
  return {
    x: CENTER + ORBIT_RADIUS * Math.cos(rad),
    y: CENTER + ORBIT_RADIUS * Math.sin(rad),
  };
}

function lineEnd(angleDeg: number) {
  // line은 BrandMark 외각까지만 — center 침범 X.
  const rad = (angleDeg * Math.PI) / 180;
  return {
    x: CENTER + INNER_RADIUS * Math.cos(rad),
    y: CENTER + INNER_RADIUS * Math.sin(rad),
  };
}

export function SplashScreen({
  ready = true,
  minDurationMs = 1500,
  maxDurationMs = 5000,
  onComplete,
}: SplashScreenProps) {
  const { t } = useTranslation();
  const reducedMotion = useReducedMotion();
  const [shown, setShown] = useState(true);
  const [minElapsed, setMinElapsed] = useState(false);

  // 최소 시간 보장 — 사용자가 짧게라도 splash를 봤음.
  useEffect(() => {
    const timer = setTimeout(() => setMinElapsed(true), minDurationMs);
    return () => clearTimeout(timer);
  }, [minDurationMs]);

  // 최대 시간 강제 dismiss — ready 안 와도.
  useEffect(() => {
    const timer = setTimeout(() => setShown(false), maxDurationMs);
    return () => clearTimeout(timer);
  }, [maxDurationMs]);

  // ready + 최소 시간 둘 다 충족 시 dismiss.
  useEffect(() => {
    if (ready && minElapsed) setShown(false);
  }, [ready, minElapsed]);

  // Esc로 즉시 dismiss — a11y 우선.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShown(false);
    };
    globalThis.window?.addEventListener("keydown", onKey);
    return () => globalThis.window?.removeEventListener("keydown", onKey);
  }, []);

  // dismiss 후 부모에 통지.
  const handleExitComplete = () => {
    onComplete?.();
  };

  // reducedMotion이면 stagger 시간 압축 + loop 정지.
  const stagger = reducedMotion ? 0.02 : 0.07;
  const lineDelay = reducedMotion ? 0.1 : 0.55;
  const brandDelay = reducedMotion ? 0.2 : 0.95;
  const wordmarkDelay = reducedMotion ? 0.3 : 1.15;

  return (
    <AnimatePresence onExitComplete={handleExitComplete}>
      {shown && (
        <motion.div
          className="splash-screen"
          role="status"
          aria-live="polite"
          aria-busy="true"
          aria-label={t("screens.splash.aria", "LMmaster를 준비하고 있어요")}
          initial={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.35, ease: "easeOut" }}
        >
          {/* PulseGlow — 배경 radial 호흡 (CSS keyframes). */}
          <div className="splash-pulse-glow" aria-hidden="true" />

          {/* Network SVG — 6 connection line + 6 outer node. */}
          <svg
            className="splash-network"
            viewBox={`0 0 ${SVG_SIZE} ${SVG_SIZE}`}
            width={SVG_SIZE}
            height={SVG_SIZE}
            aria-hidden="true"
          >
            {/* 6 connection line — 외곽 node에서 중앙 BrandMark 외각까지. */}
            {PILLAR_NODES.map((node, i) => {
              const start = nodePos(node.angleDeg);
              const end = lineEnd(node.angleDeg);
              return (
                <motion.line
                  key={`line-${node.key}`}
                  x1={start.x}
                  y1={start.y}
                  x2={end.x}
                  y2={end.y}
                  stroke="#38ff7e"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeOpacity={0.5}
                  initial={{ pathLength: 0, opacity: 0 }}
                  animate={{ pathLength: 1, opacity: 0.5 }}
                  transition={{
                    delay: lineDelay + i * 0.04,
                    duration: reducedMotion ? 0.15 : 0.35,
                    ease: "easeOut",
                  }}
                />
              );
            })}

            {/* 6 outer node — 시계 12시부터 60° 간격. stagger + 호흡 loop. */}
            {PILLAR_NODES.map((node, i) => {
              const pos = nodePos(node.angleDeg);
              return (
                <motion.circle
                  key={`node-${node.key}`}
                  cx={pos.x}
                  cy={pos.y}
                  r={NODE_RADIUS}
                  fill="#38ff7e"
                  initial={{ scale: 0.4, opacity: 0 }}
                  animate={{
                    scale: reducedMotion ? 1 : [0.4, 1.15, 1],
                    opacity: reducedMotion ? 1 : [0, 1, 0.85],
                  }}
                  transition={{
                    delay: 0.3 + i * stagger,
                    duration: reducedMotion ? 0.2 : 0.55,
                    ease: "easeOut",
                  }}
                  style={{
                    transformBox: "fill-box",
                    transformOrigin: "center",
                  }}
                />
              );
            })}
          </svg>

          {/* BrandMark — center, scale spring + glow. SVG 위에 absolute. */}
          <motion.div
            className="splash-center-brand"
            initial={{ scale: 0, opacity: 0 }}
            animate={{
              scale: reducedMotion ? 1 : [0, 1.08, 1],
              opacity: 1,
            }}
            transition={{
              delay: brandDelay,
              duration: reducedMotion ? 0.2 : 0.55,
              ease: "easeOut",
            }}
            aria-hidden="true"
          >
            <BrandMark size={88} />
          </motion.div>

          {/* Wordmark — BrandMark 아래 슬라이드 fade. */}
          <motion.div
            className="splash-wordmark"
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{
              delay: wordmarkDelay,
              duration: reducedMotion ? 0.2 : 0.4,
              ease: "easeOut",
            }}
          >
            LMmaster
          </motion.div>

          {/* Esc skip 힌트 — 1.4s 후 fade in (사용자가 인식할 시간). */}
          <motion.div
            className="splash-skip-hint"
            initial={{ opacity: 0 }}
            animate={{ opacity: 0.6 }}
            transition={{
              delay: reducedMotion ? 0.4 : 1.4,
              duration: 0.4,
            }}
          >
            {t("screens.splash.skipHint", "Esc로 건너뛰기")}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
