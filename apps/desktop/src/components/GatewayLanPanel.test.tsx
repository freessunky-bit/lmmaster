/**
 * @vitest-environment jsdom
 */
// GatewayLanPanel — Phase 8'.c.4 (ADR-0066) 사내망 노출 토글 + LAN URL 표시 단위 테스트.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) => {
      if (params && Object.keys(params).length > 0) {
        return `${key}|${JSON.stringify(params)}`;
      }
      return key;
    },
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

vi.mock("../ipc/gateway-settings", () => ({
  getGatewayAllowExternal: vi.fn(),
  setGatewayAllowExternal: vi.fn(),
  listLanAddresses: vi.fn(),
}));

vi.mock("../ipc/gateway", () => ({
  getGatewayStatus: vi.fn(),
}));

import {
  getGatewayAllowExternal,
  listLanAddresses,
  setGatewayAllowExternal,
} from "../ipc/gateway-settings";
import { getGatewayStatus } from "../ipc/gateway";

import { GatewayLanPanel } from "./GatewayLanPanel";

beforeEach(() => {
  vi.mocked(getGatewayAllowExternal).mockResolvedValue(false);
  vi.mocked(listLanAddresses).mockResolvedValue(["192.168.1.42", "10.0.0.15"]);
  vi.mocked(getGatewayStatus).mockResolvedValue({
    port: 8788,
    status: "listening",
    error: null,
  });
  vi.mocked(setGatewayAllowExternal).mockResolvedValue();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("GatewayLanPanel — 기본 렌더", () => {
  it("기본 비활성 상태로 렌더돼요 (aria-checked=false)", async () => {
    render(<GatewayLanPanel />);
    await waitFor(() => {
      expect(getGatewayAllowExternal).toHaveBeenCalled();
    });
    const toggle = screen.getByTestId("settings-gateway-lan-toggle");
    await waitFor(() => {
      expect(toggle.getAttribute("aria-checked")).toBe("false");
    });
  });

  it("회사 PC 보안 경고 카피가 항상 노출돼요", async () => {
    render(<GatewayLanPanel />);
    await waitFor(() => {
      expect(
        screen.getByTestId("settings-gateway-lan-warning"),
      ).toBeTruthy();
    });
  });

  it("토글 OFF 상태에서는 LAN URL 섹션이 안 보여요", async () => {
    render(<GatewayLanPanel />);
    await waitFor(() => {
      expect(getGatewayAllowExternal).toHaveBeenCalled();
    });
    expect(screen.queryByTestId("settings-gateway-lan-urls")).toBeNull();
  });
});

describe("GatewayLanPanel — 토글", () => {
  it("토글 켜면 setGatewayAllowExternal(true) 호출", async () => {
    const user = userEvent.setup();
    render(<GatewayLanPanel />);
    await waitFor(() => {
      expect(getGatewayAllowExternal).toHaveBeenCalled();
    });
    await user.click(screen.getByTestId("settings-gateway-lan-toggle"));
    expect(setGatewayAllowExternal).toHaveBeenCalledWith(true);
  });

  it("토글 변경 후 '재시작 후 적용' 안내가 노출돼요", async () => {
    const user = userEvent.setup();
    render(<GatewayLanPanel />);
    await waitFor(() => {
      expect(getGatewayAllowExternal).toHaveBeenCalled();
    });
    await user.click(screen.getByTestId("settings-gateway-lan-toggle"));
    await waitFor(() => {
      expect(
        screen.getByTestId("settings-gateway-lan-pending-restart"),
      ).toBeTruthy();
    });
  });

  it("토글 ON일 때 LAN URL이 port 포함해서 표시돼요", async () => {
    vi.mocked(getGatewayAllowExternal).mockResolvedValue(true);
    render(<GatewayLanPanel />);
    await waitFor(() => {
      expect(
        screen.getByTestId("settings-gateway-lan-urls"),
      ).toBeTruthy();
    });
    const urls = screen.getByTestId("settings-gateway-lan-urls");
    expect(within(urls).getByText("http://192.168.1.42:8788")).toBeTruthy();
    expect(within(urls).getByText("http://10.0.0.15:8788")).toBeTruthy();
  });

  it("LAN IP 빈 배열이면 '감지 실패' 안내가 노출돼요", async () => {
    vi.mocked(getGatewayAllowExternal).mockResolvedValue(true);
    vi.mocked(listLanAddresses).mockResolvedValue([]);
    render(<GatewayLanPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("settings-gateway-lan-empty")).toBeTruthy();
    });
  });
});
