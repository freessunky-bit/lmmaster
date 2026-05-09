import { readFileSync } from "fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const pkg = JSON.parse(readFileSync("./package.json", "utf-8")) as {
  version: string;
};

// Tauri 권장 설정 (https://tauri.app)
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  // 빌드 시점에 package.json 버전을 정적 상수로 주입.
  // Settings.tsx의 __APP_VERSION__ 참조가 실제 버전으로 대체됨.
  define: {
    __APP_VERSION__: JSON.stringify(pkg.version),
  },
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "esnext",
    sourcemap: true,
  },
});
