// Vitest 2.x — apps/desktop 테스트 설정.
// 정책 (Phase 1A.4.d.1 보강 §1):
// - 별도 config (vite.config.ts는 Tauri specific하게 유지).
// - mergeConfig로 viteConfig 재사용 — plugins 그대로 → React JSX/HMR/typescript 처리 동일.
// - default env=node (순수 단위 테스트). 컴포넌트 테스트는 파일 상단 `@vitest-environment jsdom` pragma로 opt-in.
// - globals: false — explicit imports 강제 (TS strict 친화 + grep 용이).
// - setupFiles로 jest-dom + 브리지 reset.

import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      globals: false,
      environment: "node",
      setupFiles: ["./src/__tests__/setup.ts"],
      include: ["src/**/*.{test,spec}.{ts,tsx}"],
      exclude: ["node_modules", "dist", "src-tauri"],
      coverage: {
        provider: "v8",
        reporter: ["text", "html", "lcov"],
        include: ["src/**/*.{ts,tsx}"],
        exclude: [
          "src/**/*.{test,spec}.{ts,tsx}",
          "src/__tests__/**",
          "src/main.tsx",
          "src/i18n/**",
        ],
        thresholds: {
          lines: 60,
          functions: 60,
          branches: 50,
          statements: 60,
        },
      },
    },
  }),
);
