// SplashScreen v4 — Anthropic Imagine 시연 영상 차용 (Phase 14' v4 디자인 결정 2026-05-04).
//
// 컨셉: 회전하는 globe + 표면 노드 + connection arcs + 좌측 typography + 우측 진행 panel.
// 시연 영상 패턴 적용:
//   - 큰 globe (radial gradient + lat/long lines + perspective fake)
//   - 표면 노드 (depth 기반 size/opacity — 앞은 밝음/큼, 뒤는 흐림/작음)
//   - Connection arcs (bezier curve가 globe 표면 위로 솟아오름, traveling stroke)
//   - 좌측 큰 typography ("지능을 모으고 있어요", "있어요" cyan accent)
//   - 좌측 통계 ("3 런타임 / 40 모델 / 11 도메인")
//   - 우측 진행 panel (5단계 텍스트 cycling 유지)
//
// 정책:
// - 진짜 3D rotation은 Three.js 필요 (50KB+, ROI 낮음). SVG sphere illusion + traveling arcs로 90% 체감 차용.
// - prefers-reduced-motion 자동 정적.
// - role=status aria-live=polite + Esc skip.

import { motion, AnimatePresence, useReducedMotion } from "framer-motion";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { BrandMark } from "../Brand";
import { Globe3D } from "./Globe3D";
import "./splash.css";

export interface SplashScreenProps {
  ready?: boolean;
  minDurationMs?: number;
  maxDurationMs?: number;
  onComplete?: () => void;
}

// 5단계 weight 비례 분배.
const STAGES = [
  { key: "checking", weight: 1 },
  { key: "detecting", weight: 1 },
  { key: "catalog", weight: 1 },
  { key: "gateway", weight: 1 },
  { key: "ready", weight: 1 },
] as const;
const STAGE_WEIGHT_TOTAL = STAGES.reduce((s, x) => s + x.weight, 0);

const DEFAULT_MIN_DURATION_MS = (() => {
  try {
    return import.meta.env?.DEV ? 4500 : 3000;
  } catch {
    return 3000;
  }
})();

// SVG viewBox + globe 좌표.
const VB = 600;
const CENTER = VB / 2; // 300
const GLOBE_RADIUS = 220;

// 노드 좌표 (lat: -π/2 ~ π/2, lon: -π ~ π). depth는 cos(lon) — 1(앞) ~ -1(뒤).
// 12개 노드 — 시연영상의 도시 분포 차용 (균일 분포 + 일부 앞/뒤 섞임).
const NODES_LATLON = [
  [0.4, -0.3],
  [0.1, 0.3],
  [-0.3, 0.5],
  [-0.6, 0.0],
  [0.7, 0.4],
  [-0.5, -0.7],
  [0.2, -0.9],
  [-0.1, 1.3],   // 뒤
  [0.5, 1.6],    // 뒤
  [-0.4, -1.5],  // 뒤
  [0.0, -2.0],   // 뒤 (반대편)
  [0.8, -0.1],
];

interface ProjectedNode {
  x: number;
  y: number;
  depth: number; // 1 (front) ~ -1 (back)
}

function project(lat: number, lon: number): ProjectedNode {
  const x = CENTER + GLOBE_RADIUS * Math.cos(lat) * Math.sin(lon);
  const y = CENTER - GLOBE_RADIUS * Math.sin(lat);
  const depth = Math.cos(lon) * Math.cos(lat);
  return { x, y, depth };
}

const PROJECTED_NODES: ProjectedNode[] = NODES_LATLON.map(([lat, lon]) =>
  project(lat as number, lon as number),
);

// Connection arcs — 앞쪽 노드 위주 페어. 뒤쪽 (depth < 0) 노드는 arc 표시 안 함.
const ARC_PAIRS: [number, number][] = [
  [0, 1],
  [1, 4],
  [4, 6],
  [3, 5],
  [5, 0],
  [6, 11],
  [11, 4],
  [0, 3],
];

function arcPath(from: ProjectedNode, to: ProjectedNode): string {
  // bezier control point — 두 노드 중점에서 globe 위쪽 (y - 80px)으로 솟아오름.
  const midX = (from.x + to.x) / 2;
  const midY = (from.y + to.y) / 2 - 80;
  return `M ${from.x.toFixed(1)} ${from.y.toFixed(1)} Q ${midX.toFixed(1)} ${midY.toFixed(1)} ${to.x.toFixed(1)} ${to.y.toFixed(1)}`;
}

const LINEAR_EASE = [0.16, 1, 0.3, 1] as const;

export function SplashScreen({
  ready = true,
  minDurationMs = DEFAULT_MIN_DURATION_MS,
  maxDurationMs = 8000,
  onComplete,
}: SplashScreenProps) {
  const { t } = useTranslation();
  const reducedMotion = useReducedMotion();
  const [shown, setShown] = useState(true);
  const [minElapsed, setMinElapsed] = useState(false);
  const [stageIdx, setStageIdx] = useState(0);

  // Stage 진행 — minDuration 비례 분배.
  useEffect(() => {
    const speedFactor = reducedMotion ? 0.4 : 1;
    const totalMs = minDurationMs * speedFactor;
    let cumulative = 0;
    const timers: ReturnType<typeof setTimeout>[] = [];
    for (let i = 0; i < STAGES.length; i++) {
      const stage = STAGES[i];
      if (!stage) continue;
      cumulative += (stage.weight / STAGE_WEIGHT_TOTAL) * totalMs;
      const nextIdx = Math.min(i + 1, STAGES.length - 1);
      timers.push(setTimeout(() => setStageIdx(nextIdx), cumulative));
    }
    return () => timers.forEach(clearTimeout);
  }, [reducedMotion, minDurationMs]);

  useEffect(() => {
    const t = setTimeout(() => setMinElapsed(true), minDurationMs);
    return () => clearTimeout(t);
  }, [minDurationMs]);

  useEffect(() => {
    const t = setTimeout(() => setShown(false), maxDurationMs);
    return () => clearTimeout(t);
  }, [maxDurationMs]);

  useEffect(() => {
    if (ready && minElapsed) setShown(false);
  }, [ready, minElapsed]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setShown(false);
    };
    globalThis.window?.addEventListener("keydown", onKey);
    return () => globalThis.window?.removeEventListener("keydown", onKey);
  }, []);

  const currentStage = STAGES[stageIdx] ?? STAGES[0]!;
  const stageLabel = t(`screens.splash.stage.${currentStage.key}`, currentStage.key);

  return (
    <AnimatePresence onExitComplete={onComplete}>
      {shown && (
        <motion.div
          className="splash-screen splash-v4"
          role="status"
          aria-live="polite"
          aria-busy="true"
          aria-label={t("screens.splash.aria", "LMmaster를 준비하고 있어요")}
          initial={{ opacity: 1 }}
          exit={{ opacity: 0, scale: 1.02 }}
          transition={{ duration: 0.45, ease: LINEAR_EASE }}
        >
          {/* Layer 1 — 배경 grid overlay (CSS pseudo). */}

          {/* Layer 2 — 다층 radial bloom. */}
          <div className="splash-bloom-1" aria-hidden="true" />
          <div className="splash-bloom-2" aria-hidden="true" />

          {/* 메인 layout — 좌측 typography + 우측 globe. */}
          <div className="splash-v4-layout">
            {/* 좌측 — 큰 typography + 통계. */}
            <motion.div
              className="splash-v4-message"
              initial={{ opacity: 0, x: -16 }}
              animate={{ opacity: 1, x: 0 }}
              transition={{
                delay: reducedMotion ? 0.2 : 1.0,
                duration: reducedMotion ? 0.3 : 0.9,
                ease: LINEAR_EASE,
              }}
            >
              <h1 className="splash-v4-headline">
                {t("screens.splash.headline.prefix", "지능을 모으고 ")}
                <span className="splash-v4-accent">
                  {t("screens.splash.headline.accent", "있어요")}
                </span>
              </h1>
              <p className="splash-v4-subline">
                {t(
                  "screens.splash.subline",
                  "런타임 · 모델 · 카탈로그가 한자리에 모여요",
                )}
              </p>
              <div className="splash-v4-stats">
                <div className="splash-v4-stat">
                  <span className="splash-v4-stat-num num">3</span>
                  <span className="splash-v4-stat-label">
                    {t("screens.splash.stats.runtime", "런타임")}
                  </span>
                </div>
                <div className="splash-v4-stat">
                  <span className="splash-v4-stat-num num">40</span>
                  <span className="splash-v4-stat-label">
                    {t("screens.splash.stats.model", "모델")}
                  </span>
                </div>
                <div className="splash-v4-stat">
                  <span className="splash-v4-stat-num num">11</span>
                  <span className="splash-v4-stat-label">
                    {t("screens.splash.stats.intent", "도메인")}
                  </span>
                </div>
              </div>
            </motion.div>

            {/* 우측 — Three.js Globe3D (Phase 14' v5). */}
            <div className="splash-v4-globe-wrap" aria-hidden="true">
              <Globe3D
                size={520}
                rotationSpeed={reducedMotion ? 0 : 0.0014}
              />
              <svg
                className="splash-v4-globe-legacy"
                viewBox={`0 0 ${VB} ${VB}`}
                width="100%"
                height="100%"
                style={{ display: "none" }}
              >
                <defs>
                  <radialGradient id="globe-fill-v4" cx="0.42" cy="0.4" r="0.7">
                    <stop offset="0%" stopColor="rgba(56,255,126,0.18)" />
                    <stop offset="50%" stopColor="rgba(20,40,40,0.5)" />
                    <stop offset="100%" stopColor="rgba(0,0,0,0.85)" />
                  </radialGradient>
                  <linearGradient id="arc-grad-v4" x1="0" y1="0" x2="1" y2="0">
                    <stop offset="0%" stopColor="rgba(124,255,245,0)" />
                    <stop offset="50%" stopColor="rgba(124,255,245,1)" />
                    <stop offset="100%" stopColor="rgba(76,255,160,0)" />
                  </linearGradient>
                  <radialGradient id="node-grad-v4" cx="0.5" cy="0.5" r="0.5">
                    <stop offset="0%" stopColor="#7cfff5" />
                    <stop offset="60%" stopColor="#4cffa0" />
                    <stop offset="100%" stopColor="#1ee063" />
                  </radialGradient>
                </defs>

                {/* Globe 본체 — radial gradient (밝은 부분 좌상). */}
                <motion.circle
                  cx={CENTER}
                  cy={CENTER}
                  r={GLOBE_RADIUS}
                  fill="url(#globe-fill-v4)"
                  stroke="rgba(56,255,126,0.25)"
                  strokeWidth="1"
                  initial={{ opacity: 0, scale: 0.85 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{
                    delay: reducedMotion ? 0.1 : 0.3,
                    duration: reducedMotion ? 0.25 : 0.9,
                    ease: LINEAR_EASE,
                  }}
                />

                {/* Latitude lines (3 horizontal ellipses). */}
                {[60, 130, 190].map((ry, i) => (
                  <motion.ellipse
                    key={`lat-${i}`}
                    cx={CENTER}
                    cy={CENTER}
                    rx={GLOBE_RADIUS}
                    ry={ry}
                    fill="none"
                    stroke="rgba(56,255,126,0.1)"
                    strokeWidth="0.7"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{
                      delay: reducedMotion ? 0.2 : 0.6 + i * 0.07,
                      duration: 0.5,
                    }}
                  />
                ))}

                {/* Longitude lines (3 vertical ellipses). */}
                {[60, 130, 190].map((rx, i) => (
                  <motion.ellipse
                    key={`lon-${i}`}
                    cx={CENTER}
                    cy={CENTER}
                    rx={rx}
                    ry={GLOBE_RADIUS}
                    fill="none"
                    stroke="rgba(56,255,126,0.1)"
                    strokeWidth="0.7"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{
                      delay: reducedMotion ? 0.2 : 0.7 + i * 0.07,
                      duration: 0.5,
                    }}
                  />
                ))}

                {/* Connection arcs — bezier traveling stroke. */}
                {ARC_PAIRS.map(([a, b], i) => {
                  const from = PROJECTED_NODES[a];
                  const to = PROJECTED_NODES[b];
                  if (!from || !to) return null;
                  // 둘 다 앞쪽이거나 한쪽이라도 앞쪽이면 표시.
                  if (from.depth < -0.3 && to.depth < -0.3) return null;
                  const path = arcPath(from, to);
                  return (
                    <motion.path
                      key={`arc-${i}`}
                      d={path}
                      stroke="url(#arc-grad-v4)"
                      strokeWidth="1.4"
                      fill="none"
                      strokeLinecap="round"
                      initial={{ pathLength: 0, opacity: 0 }}
                      animate={{
                        pathLength: 1,
                        opacity: reducedMotion ? 0.5 : [0, 0.7, 0.5, 0.8, 0.5],
                      }}
                      transition={{
                        pathLength: {
                          delay: reducedMotion ? 0.4 : 1.5 + i * 0.12,
                          duration: reducedMotion ? 0.3 : 0.9,
                          ease: LINEAR_EASE,
                        },
                        opacity: reducedMotion
                          ? { delay: 0.4, duration: 0.3 }
                          : {
                              delay: 1.5 + i * 0.12,
                              duration: 3.5,
                              repeat: Infinity,
                              ease: "easeInOut",
                            },
                      }}
                    />
                  );
                })}

                {/* 표면 노드 — depth로 size + opacity 결정. */}
                {PROJECTED_NODES.map((node, i) => {
                  const isFront = node.depth >= 0;
                  const radius = isFront
                    ? 4 + node.depth * 2
                    : 1.5 + (1 + node.depth) * 1.2;
                  const opacity = isFront
                    ? 0.6 + node.depth * 0.4
                    : 0.15 + (1 + node.depth) * 0.25;
                  return (
                    <motion.circle
                      key={`node-${i}`}
                      cx={node.x}
                      cy={node.y}
                      r={radius}
                      fill={isFront ? "url(#node-grad-v4)" : "rgba(124,255,245,0.4)"}
                      initial={{ scale: 0, opacity: 0 }}
                      animate={{
                        scale: reducedMotion ? 1 : [0, 1.4, 1],
                        opacity: reducedMotion
                          ? opacity
                          : isFront
                            ? [0, 1, opacity, 1, opacity]
                            : opacity,
                      }}
                      transition={{
                        scale: {
                          delay: reducedMotion ? 0.2 : 0.9 + i * 0.05,
                          duration: reducedMotion ? 0.25 : 0.55,
                          ease: LINEAR_EASE,
                        },
                        opacity: reducedMotion
                          ? { delay: 0.2, duration: 0.3 }
                          : {
                              delay: 0.9 + i * 0.05,
                              duration: isFront ? 3.5 : 0.55,
                              repeat: isFront ? Infinity : 0,
                              ease: "easeInOut",
                            },
                      }}
                      style={{
                        transformBox: "fill-box",
                        transformOrigin: "center",
                        filter: isFront
                          ? `drop-shadow(0 0 ${4 + node.depth * 4}px rgba(124,255,245,0.6))`
                          : "none",
                      }}
                    />
                  );
                })}
              </svg>

              {/* Globe 정중앙 — BrandMark + 글로우 (z-index 위). */}
              <motion.div
                className="splash-v4-center-brand"
                initial={{ scale: 0, opacity: 0 }}
                animate={{
                  scale: reducedMotion ? 1 : [0, 1.08, 1],
                  opacity: 1,
                }}
                transition={{
                  delay: reducedMotion ? 0.3 : 1.9,
                  duration: reducedMotion ? 0.25 : 0.7,
                  ease: LINEAR_EASE,
                }}
                aria-hidden="true"
              >
                <BrandMark size={84} />
              </motion.div>
            </div>
          </div>

          {/* 하단 진행 panel — 5단계 텍스트 + dot + skip. */}
          <div className="splash-v4-bottom">
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
                delay: reducedMotion ? 0.4 : 2.6,
                duration: 0.5,
                ease: LINEAR_EASE,
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
