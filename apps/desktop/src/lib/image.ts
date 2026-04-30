// 클라이언트 이미지 전처리 — Phase 13'.h (ADR-0050).
//
// 정책:
// - max 4096px 리사이즈 + JPEG 90% 압축으로 IPC 페이로드 보호 (위험 매트릭스 §6).
// - canvas 기반 — jsdom/SSR 환경에서 가용성 검증 후 호출 (테스트는 환경 가드 필요).
// - 결과는 base64 string (data URL prefix 제외) — Ollama API `images: [base64]` 그대로.
// - 외부 통신 0 정책 — 모든 처리는 브라우저 내.

const DEFAULT_MAX_DIMENSION = 4096;
const DEFAULT_JPEG_QUALITY = 0.9;

export interface ProcessImageOptions {
  /** 가로/세로 중 큰 쪽 한도 (px). 초과 시 비율 유지하며 축소. 기본 4096. */
  maxDimension?: number;
  /** JPEG 압축 품질 0..1. 기본 0.9. */
  jpegQuality?: number;
}

export interface ProcessedImage {
  /** base64 인코딩 — data URL prefix 제외 (Ollama 호환). */
  base64: string;
  /** 결과 가로 px. */
  width: number;
  /** 결과 세로 px. */
  height: number;
  /** 결과 용량 (byte) — base64 페이로드 길이로 추정. */
  approxBytes: number;
}

/**
 * File 또는 Blob을 max-dimension 리사이즈 + JPEG 압축한 base64로 변환.
 *
 * canvas/createImageBitmap이 없는 환경(node SSR 등)에서는 throw.
 */
export async function processImageForVision(
  blob: Blob,
  opts: ProcessImageOptions = {},
): Promise<ProcessedImage> {
  const maxDimension = opts.maxDimension ?? DEFAULT_MAX_DIMENSION;
  const jpegQuality = opts.jpegQuality ?? DEFAULT_JPEG_QUALITY;

  if (typeof document === "undefined") {
    throw new Error("이미지 전처리는 브라우저 환경에서만 가능해요.");
  }

  const bitmap = await createBitmap(blob);
  const { width, height } = scaleToMax(bitmap.width, bitmap.height, maxDimension);

  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("Canvas 2D 컨텍스트를 가져오지 못했어요.");
  }
  ctx.drawImage(bitmap, 0, 0, width, height);

  // JPEG으로 인코딩 — toDataURL은 동기. blob API가 더 정확하지만 단순화.
  const dataUrl = canvas.toDataURL("image/jpeg", jpegQuality);
  const base64 = stripDataUrlPrefix(dataUrl);

  return {
    base64,
    width,
    height,
    approxBytes: Math.floor((base64.length * 3) / 4), // base64 → byte 추정.
  };
}

/**
 * jsdom 환경에서는 createImageBitmap이 없을 수 있음 — Image 폴백.
 */
async function createBitmap(blob: Blob): Promise<ImageBitmap | HTMLImageElement> {
  if (typeof createImageBitmap === "function") {
    try {
      return await createImageBitmap(blob);
    } catch {
      // 폴백.
    }
  }
  return await new Promise<HTMLImageElement>((resolve, reject) => {
    const url = URL.createObjectURL(blob);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve(img);
    };
    img.onerror = (e) => {
      URL.revokeObjectURL(url);
      reject(new Error("이미지 로드 실패"));
      void e;
    };
    img.src = url;
  });
}

/**
 * 가로/세로 중 큰 쪽이 maxDimension을 넘으면 비율 유지 축소.
 * 그 이하면 원본 크기 반환.
 */
export function scaleToMax(
  width: number,
  height: number,
  maxDimension: number,
): { width: number; height: number } {
  const longest = Math.max(width, height);
  if (longest <= maxDimension) return { width, height };
  const ratio = maxDimension / longest;
  return {
    width: Math.round(width * ratio),
    height: Math.round(height * ratio),
  };
}

/** "data:image/jpeg;base64,..." 또는 "data:..." → "..." 만 반환. prefix 없으면 원본. */
export function stripDataUrlPrefix(dataUrl: string): string {
  const idx = dataUrl.indexOf(",");
  if (idx < 0) return dataUrl;
  return dataUrl.slice(idx + 1);
}
