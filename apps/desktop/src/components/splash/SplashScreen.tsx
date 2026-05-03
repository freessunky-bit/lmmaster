// SplashScreen v3 — 세계 최고 수준 레퍼런스 종합 (Phase 14' v3 디자인 결정 2026-05-04).
//
// 차용 패턴:
// - 진행 텍스트 cycling (JetBrains IDE / Adobe Suite / League of Legends Client)
// - 레이어드 radial bloom (Kling AI / Runway / Stripe Mesh)
// - Network + data packet (Linear team Loop / Apple Music)
// - Concentric pulse (Spotify Wave / Stripe)
// - Orbital ring (Apple Vision Pro / Halo)
// - Cubic-bezier(0.16, 1, 0.3, 1) Linear 시그니처 easing
// - 3-tone gradient + cyan accent (Vercel / Anthropic)
//
// 동작:
// - 5단계 시간 기반 진행 (텍스트 cycling + dot indicator).
// - 8 node 45° 간격 orbital + 외각 faded ring.
// - 6 connection line stroke draw + data packet motion (line 따라 점 흐름).
// - BrandMark center + orbital ring (회전 loop) + concentric pulse 외부 방출.
// - 3-tone gradient (#4cffa0 / #38ff7e / #1ee063) + cyan accent (#7cfff5).
// - prefers-reduced-motion 자동 정적 + 시간 압축.
// - role=status aria-live=polite + Esc skip.

import { motion, AnimatePresence, useReducedMotion } from "framer-motion";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { BrandMark } from "../Brand";
import "./splash.css";

export interface SplashScreenProps {
  ready?: boolean;
  minDurationMs?: number;
  maxDurationMs?: number;
  onComplete?: () => void;
}

// 5단계 — 각 ~360ms (총 1.8s). 마지막 stage는 minDuration까지 hold.
// i18n 키만 명시 — 실제 텍스트는 ko/en에서 fetch.
const STAGES = [
  { key: "checking", durationMs: 360 },
  { key: "detecting", durationMs: 360 },
  { key: "catalog", durationMs: 360 },
  { key: "gateway", durationMs: 360 },
  { key: "ready", durationMs: 360 },
] as const;

const NODE_COUNT = 8;
const SVG_SIZE = 460;
const CENTER = SVG_SIZE / 2; // 230
const ORBIT_RADIUS = 150;
const NODE_RADIUS = 5;
const INNER_RADIUS = 40;
const OUTER_FADED_RING_RADIUS = 175;

// 8 node — 12시 시작 시계방향 45° 간격.
function nodeAngle(i: number) {
  return -90 + i * (360 / NODE_COUNT);
}

function nodePos(angleDeg: number, radius = ORBIT_RADIUS) {
  const rad = (angleDeg * Math.PI) / 180;
  return {
    x: CENTER + radius * Math.cos(rad),
    y: CENTER + radius * Math.sin(rad),
  };
}

// Linear 시그니처 easing (cubic-bezier(0.16, 1, 0.3, 1)) — Framer Motion 표기.
const LINEAR_EASE = [0.16, 1, 0.3, 1] as const;

export function SplashScreen({
  ready = true,
  minDurationMs = 1800,
  maxDurationMs = 6000,
  onComplete,
}: SplashScreenProps) {
  const { t } = useTranslation();
  const reducedMotion = useReducedMotion();
  const [shown, setShown] = useState(true);
  const [minElapsed, setMinElapsed] = useState(false);
  const [stageIdx, setStageIdx] = useState(0);

  // 단계 진행 — 시간 기반 5 stage (총 1.8s).
  useEffect(() => {
    const speedFactor = reducedMotion ? 0.4 : 1;
    let cumulative = 0;
    const timers: ReturnType<typeof setTimeout>[] = [];
    for (let i = 0; i < STAGES.length; i++) {
      const stage = STAGES[i];
      if (!stage) continue;
      cumulative += stage.durationMs * speedFactor;
      const nextIdx = Math.min(i + 1, STAGES.length - 1);
      timers.push(setTimeout(() => setStageIdx(nextIdx), cumulative));
    }
    return () => timers.forEach(clearTimeout);
  }, [reducedMotion]);

  // 최소 시간 보장.
  useEffect(() => {
    const t = setTimeout(() => setMinElapsed(true), minDurationMs);
    return () => clearTimeout(t);
  }, [minDurationMs]);

  // 최대 시간 강제 dismiss.
  useEffect(() => {
    const t = setTimeout(() => setShown(false), maxDurationMs);
    return () => clearTimeout(t);
  }, [maxDurationMs]);

  // ready + 최소 시간 충족 시 dismiss.
  useEffect(() => {
    if (ready && minElapsed) setShown(false);
  }, [ready, minElapsed]);

  // Esc skip.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShown(false);
    };
    globalThis.window?.addEventListener("keydown", onKey);
    return () => globalThis.window?.removeEventListener("keydown", onKey);
  }, []);

  const currentStage = STAGES[stageIdx] ?? STAGES[0]!;
  const stageLabel = t(`screens.splash.stage.${currentStage.key}`, currentStage.key);

  // 8 node positions — 미리 계산.
  const nodes = Array.from({ length: NODE_COUNT }, (_, i) => ({
    i,
    angleDeg: nodeAngle(i),
    pos: nodePos(nodeAngle(i)),
  }));

  return (
    <AnimatePresence onExitComplete={onComplete}>
      {shown && (
        <motion.div
          className="splash-screen"
          role="status"
          aria-live="polite"
          aria-busy="true"
          aria-label={t("screens.splash.aria", "LMmaster를 준비하고 있어요")}
          initial={{ opacity: 1 }}
          exit={{ opacity: 0, scale: 1.02 }}
          transition={{ duration: 0.45, ease: LINEAR_EASE }}
        >
          {/* Layer 1 — 배경 grid overlay (CSS pseudo). */}

          {/* Layer 2 — 다층 radial bloom (CSS keyframes loop). */}
          <div className="splash-bloom-1" aria-hidden="true" />
          <div className="splash-bloom-2" aria-hidden="true" />

          {/* Layer 3 — Network SVG: outer faded ring + 8 nodes + 6 connection + 6 data packets. */}
          <svg
            className="splash-network"
            viewBox={`0 0 ${SVG_SIZE} ${SVG_SIZE}`}
            width={SVG_SIZE}
            height={SVG_SIZE}
            aria-hidden="true"
          >
            <defs>
              {/* 3-tone gradient + cyan accent stop. */}
              <linearGradient id="splash-grad-line" x1="0" y1="0" x2="1" y2="1">
                <stop offset="0%" stopColor="#4cffa0" />
                <stop offset="60%" stopColor="#38ff7e" />
                <stop offset="100%" stopColor="#7cfff5" />
              </linearGradient>
              <radialGradient id="splash-grad-node" cx="0.5" cy="0.5" r="0.5">
                <stop offset="0%" stopColor="#7cfff5" />
                <stop offset="50%" stopColor="#4cffa0" />
                <stop offset="100%" stopColor="#1ee063" />
              </radialGradient>
              {/* Concentric pulse — center에서 외부로 방출하는 ring stroke. */}
              <radialGradient id="splash-grad-pulse" cx="0.5" cy="0.5" r="0.5">
                <stop offset="60%" stopColor="rgba(56,255,126,0)" />
                <stop offset="80%" stopColor="rgba(56,255,126,0.4)" />
                <stop offset="100%" stopColor="rgba(56,255,126,0)" />
              </radialGradient>
            </defs>

            {/* Outer faded ring — orbit 위 ambient ring. */}
            <motion.circle
              cx={CENTER}
              cy={CENTER}
              r={OUTER_FADED_RING_RADIUS}
              fill="none"
              stroke="rgba(56,255,126,0.12)"
              strokeWidth="1"
              strokeDasharray="2 6"
              initial={{ opacity: 0, rotate: 0 }}
              animate={{
                opacity: 1,
                rotate: reducedMotion ? 0 : 360,
              }}
              transition={{
                opacity: { delay: 0.2, duration: 0.5, ease: LINEAR_EASE },
                rotate: {
                  duration: reducedMotion ? 0 : 24,
                  repeat: reducedMotion ? 0 : Infinity,
                  ease: "linear",
                },
              }}
              style={{ transformOrigin: `${CENTER}px ${CENTER}px` }}
            />

            {/* Concentric pulse — center에서 ring 방출 1.8s 주기. */}
            {!reducedMotion && (
              <motion.circle
                cx={CENTER}
                cy={CENTER}
                r={1}
                fill="none"
                stroke="url(#splash-grad-pulse)"
                strokeWidth="2"
                initial={{ opacity: 0, scale: 0.4 }}
                animate={{
                  opacity: [0, 0.5, 0],
                  scale: [0.4, 3.0, 4.0],
                }}
                transition={{
                  delay: 1.0,
                  duration: 1.8,
                  ease: LINEAR_EASE,
                  repeat: Infinity,
                  repeatDelay: 0.4,
                }}
                style={{ transformOrigin: `${CENTER}px ${CENTER}px` }}
              />
            )}

            {/* Connection lines — 외곽 node에서 BrandMark 외각으로. */}
            {nodes.map((node, i) => {
              const start = node.pos;
              const end = nodePos(node.angleDeg, INNER_RADIUS);
              return (
                <motion.line
                  key={`line-${i}`}
                  x1={start.x}
                  y1={start.y}
                  x2={end.x}
                  y2={end.y}
                  stroke="url(#splash-grad-line)"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeOpacity={0.55}
                  initial={{ pathLength: 0, opacity: 0 }}
                  animate={{ pathLength: 1, opacity: 0.55 }}
                  transition={{
                    delay: 0.5 + i * 0.04,
                    duration: reducedMotion ? 0.18 : 0.5,
                    ease: LINEAR_EASE,
                  }}
                />
              );
            })}

            {/* Data packets — line을 따라 흐르는 작은 점 (4개만 — 시각 정신없음 회피). */}
            {!reducedMotion &&
              nodes.slice(0, 4).map((node, i) => {
                const angleRad = (node.angleDeg * Math.PI) / 180;
                const dx = Math.cos(angleRad);
                const dy = Math.sin(angleRad);
                return (
                  <motion.circle
                    key={`packet-${i}`}
                    r={2.5}
                    fill="#7cfff5"
                    filter="url(#splash-bloom-filter)"
                    initial={{ opacity: 0 }}
                    animate={{
                      cx: [
                        CENTER + ORBIT_RADIUS * dx,
                        CENTER + INNER_RADIUS * dx,
                      ],
                      cy: [
                        CENTER + ORBIT_RADIUS * dy,
                        CENTER + INNER_RADIUS * dy,
                      ],
                      opacity: [0, 1, 0.8, 0],
                    }}
                    transition={{
                      delay: 1.0 + i * 0.18,
                      duration: 1.4,
                      ease: LINEAR_EASE,
                      repeat: Infinity,
                      repeatDelay: 1.2,
                    }}
                  />
                );
              })}

            {/* 8 outer nodes — stagger fade-in + scale spring + 호흡 loop. */}
            {nodes.map((node, i) => (
              <motion.circle
                key={`node-${i}`}
                cx={node.pos.x}
                cy={node.pos.y}
                r={NODE_RADIUS}
                fill="url(#splash-grad-node)"
                initial={{ scale: 0.3, opacity: 0 }}
                animate={{
                  scale: reducedMotion ? 1 : [0.3, 1.2, 1],
                  opacity: reducedMotion ? 1 : [0, 1, 0.85],
                }}
                transition={{
                  delay: 0.25 + i * 0.06,
                  duration: reducedMotion ? 0.18 : 0.55,
                  ease: LINEAR_EASE,
                }}
                style={{
                  transformBox: "fill-box",
                  transformOrigin: "center",
                }}
              />
            ))}
          </svg>

          {/* BrandMark center + orbital ring (회전 loop). */}
          <motion.div
            className="splash-center-brand"
            initial={{ scale: 0, opacity: 0 }}
            animate={{
              scale: reducedMotion ? 1 : [0, 1.08, 1],
              opacity: 1,
            }}
            transition={{
              delay: reducedMotion ? 0.15 : 0.95,
              duration: reducedMotion ? 0.2 : 0.6,
              ease: LINEAR_EASE,
            }}
            aria-hidden="true"
          >
            <div className="splash-orbital-ring" />
            <BrandMark size={96} />
          </motion.div>

          {/* Wordmark + 진행 텍스트 + dot 인디케이터 (한 영역). */}
          <div className="splash-bottom-area">
            <motion.div
              className="splash-wordmark"
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{
                delay: reducedMotion ? 0.25 : 1.15,
                duration: reducedMotion ? 0.2 : 0.45,
                ease: LINEAR_EASE,
              }}
            >
              LMmaster
            </motion.div>

            {/* 진행 텍스트 cycling — fade in/out 300ms. */}
            <div className="splash-stage-text-wrap" aria-live="polite">
              <AnimatePresence mode="wait">
                <motion.div
                  key={currentStage.key}
                  className="splash-stage-text"
                  initial={{ opacity: 0, y: 4 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -4 }}
                  transition={{ duration: 0.3, ease: LINEAR_EASE }}
                >
                  {stageLabel}
                </motion.div>
              </AnimatePresence>
            </div>

            {/* 5 dot 인디케이터. */}
            <div className="splash-dots" aria-hidden="true">
              {STAGES.map((s, i) => (
                <span
                  key={s.key}
                  className={`splash-dot${i <= stageIdx ? " is-active" : ""}`}
                />
              ))}
            </div>

            <motion.div
              className="splash-skip-hint"
              initial={{ opacity: 0 }}
              animate={{ opacity: 0.55 }}
              transition={{
                delay: reducedMotion ? 0.4 : 1.5,
                duration: 0.4,
              }}
            >
              {t("screens.splash.skipHint", "Esc로 건너뛰기")}
            </motion.div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
