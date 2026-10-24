import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mockAccountReset } from "./__mocks__/account";
import {
  accountAddAuthlib,
  accountList,
  authlibServerInfo,
  isAppNotApproved,
  msLoginStart,
} from "./account";

describe("account api (mock)", () => {
  beforeEach(() => mockAccountReset());
  afterEach(() => vi.useRealTimers());

  it("delivers a device code, then adds the Microsoft account as default", async () => {
    vi.useFakeTimers();
    const login = msLoginStart();
    await vi.advanceTimersByTimeAsync(1000);
    const code = await login.deviceCode;
    expect(code.expires_in).toBeGreaterThan(0);
    expect(code.verification_uri).toMatch(/^https:\/\//);
    await vi.advanceTimersByTimeAsync(10_000);
    const account = await login.result;
    expect(account.kind).toBe("microsoft");
    expect((await accountList()).default_id).toBe(account.id);
  });

  it("rejects with COMMON_CANCELLED when cancelled", async () => {
    vi.useFakeTimers();
    const login = msLoginStart();
    login.cancel();
    await expect(login.result).rejects.toMatchObject({ code: "COMMON_CANCELLED" });
    expect((await accountList()).accounts.some((a) => a.kind === "microsoft")).toBe(false);
  });

  it("recognizes AUTH_MC_APP_NOT_APPROVED", () => {
    const code = "AUTH_MC_APP_NOT_APPROVED" as never;
    expect(isAppNotApproved({ code, params: {}, detail: null })).toBe(true);
    expect(isAppNotApproved({ code: "NET_TIMEOUT", params: {}, detail: null })).toBe(false);
  });

  it("normalizes the skin site address and logs in", async () => {
    const server = await authlibServerInfo({ api_url: " littleskin.cn/api/yggdrasil/ " });
    expect(server).toEqual({
      api_url: "https://littleskin.cn/api/yggdrasil",
      name: "littleskin.cn",
    });
    const account = await accountAddAuthlib({
      api_url: server.api_url,
      username: "steve@example.com",
      password: "test-password",
    });
    expect(account).toMatchObject({ kind: "authlib", name: "steve", server });
    expect((await accountList()).default_id).toBe(account.id);
  });

  it("rejects an invalid skin site address", async () => {
    await expect(authlibServerInfo({ api_url: "http://" })).rejects.toMatchObject({
      code: "NET_REQUEST",
    });
  });
});
