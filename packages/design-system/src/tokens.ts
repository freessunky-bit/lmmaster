// JS 측에서 동일한 디자인 토큰을 참조할 때 사용.
// CSS variable과 1:1 대응. 변경 시 tokens.css와 함께 업데이트.

export const color = {
  bg: "#07090b",
  bgSubtle: "#0b0e12",
  surface: "#11151a",
  surface2: "#161b21",
  surface3: "#1c2229",
  text: "#e8edf2",
  textSecondary: "#9aa3ad",
  textMuted: "#6b7480",
  border: "#1f2630",
  borderStrong: "#2a323d",
  primary: "#38ff7e",
  primaryHover: "#4cffa0",
  primaryPress: "#1ee063",
  primaryOn: "#04140a",
  info: "#6aa3ff",
  warn: "#ffb454",
  error: "#ff5c6c",
  accent: "#b388ff",
} as const;

export const space = [0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64] as const;
export const radius = { r1: 4, r2: 8, r3: 12, pill: 9999 } as const;

export const motion = {
  durFast: "120ms",
  durBase: "180ms",
  durSlow: "280ms",
  easeStandard: "cubic-bezier(0.2, 0.8, 0.2, 1)",
} as const;
