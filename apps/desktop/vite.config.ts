import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 권장 설정 (https://tauri.app)
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
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
