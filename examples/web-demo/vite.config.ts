import { defineConfig } from "vite";

// Vite — 정적 호스팅. 데스크톱 앱과 별개의 dev 서버에서 동작 (예: 5173 포트).
// LMmaster gateway가 발급한 키의 allowed_origins에 이 포트를 등록해야 호출 통과.
export default defineConfig({
  server: {
    port: 5173,
    strictPort: true,
  },
});
