import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import {
  accountAddAuthlib,
  authlibServerInfo,
  isAppNotApproved,
  msLoginStart,
  msLoginStatus,
} from "./account";

describe("account api", () => {
  beforeEach(() => tauriInvoke.mockReset());
  afterEach(() => vi.useRealTimers());

  it("calls the auth_* commands with args", async () => {
    tauriInvoke.mockResolvedValue({ api_url: "https://a.test/api", name: "A" });
    await authlibServerInfo({ api_url: "a.test" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("auth_authlib_server", {
      args: { api_url: "a.test" },
    });
    const args = { api_url: "https://a.test/api", username: "u", password: "test-password" };
    await accountAddAuthlib(args);
    expect(tauriInvoke).toHaveBeenLastCalledWith("auth_add_authlib", { args });
  });

  it("keeps Microsoft sign-in closed until M0-10", async () => {
    tauriInvoke.mockResolvedValueOnce({ ms_login_enabled: true });
    expect(await msLoginStatus()).toBe("pending");
    tauriInvoke.mockResolvedValueOnce({ ms_login_enabled: false });
    expect(await msLoginStatus()).toBe("no_client_id");
  });

  it("mock Microsoft sign-in delivers a code and can be cancelled", async () => {
    vi.useFakeTimers();
    const login = msLoginStart();
    await vi.advanceTimersByTimeAsync(1000);
    expect((await login.deviceCode).expires_in).toBeGreaterThan(0);
    login.cancel();
    await expect(login.result).rejects.toMatchObject({ code: "COMMON_CANCELLED" });
  });

  it("recognizes AUTH_MC_APP_NOT_APPROVED", () => {
    const code = "AUTH_MC_APP_NOT_APPROVED" as never;
    expect(isAppNotApproved({ code, params: {}, detail: null })).toBe(true);
    expect(isAppNotApproved({ code: "NET_TIMEOUT", params: {}, detail: null })).toBe(false);
  });
});
