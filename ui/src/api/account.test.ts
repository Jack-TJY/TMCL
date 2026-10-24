import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));
/** 事件名 → 当前订阅的处理函数。 */
const listeners = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(name, handler);
    return () => listeners.delete(name);
  }),
}));

import {
  accountAddAuthlib,
  authlibServerInfo,
  isAppNotApproved,
  msLoginStart,
  msLoginStatus,
  openVerificationPage,
} from "./account";

const steve = {
  id: "a1",
  kind: "microsoft",
  name: "Steve",
  uuid: "0b6f4c2a-9d3e-4f8a-8b1c-2d3e4f5a6b7c",
  server: null,
};

const code = {
  user_code: "TMCLTEST",
  verification_uri: "https://www.microsoft.com/link",
  expires_in: 900,
};

function emit(name: string, payload: unknown) {
  listeners.get(name)?.({ payload });
}

/** 模拟后端：`auth_ms_begin` 返回任务 t1；`beforeReturn` 在命令返回前执行（模拟事件先到）。 */
function backend(beforeReturn?: () => void) {
  tauriInvoke.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case "auth_ms_begin":
        beforeReturn?.();
        return { task_id: "t1" };
      case "auth_list":
        return { accounts: [steve], default_id: "a1" };
      default:
        return { cancelled: true };
    }
  });
}

/** 等 `auth_ms_begin` 返回、任务 ID 就位。 */
async function begun() {
  await vi.waitFor(() => expect(tauriInvoke).toHaveBeenCalledWith("auth_ms_begin", undefined));
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("account api", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    listeners.clear();
  });

  it("calls the auth_* commands with args", async () => {
    tauriInvoke.mockResolvedValue({ api_url: "https://a.test/api", name: "A" });
    await authlibServerInfo({ api_url: "a.test" });
    expect(tauriInvoke).toHaveBeenLastCalledWith("auth_authlib_server", {
      args: { api_url: "a.test" },
    });
    const args = { api_url: "https://a.test/api", username: "u", password: "test-password" };
    await accountAddAuthlib(args);
    expect(tauriInvoke).toHaveBeenLastCalledWith("auth_add_authlib", { args });
    tauriInvoke.mockResolvedValue(null);
    await openVerificationPage(code.verification_uri);
    expect(tauriInvoke).toHaveBeenLastCalledWith("auth_ms_open_verification", {
      args: { uri: code.verification_uri },
    });
  });

  it("reports whether Microsoft sign-in is built in", async () => {
    tauriInvoke.mockResolvedValueOnce({ ms_login_enabled: true });
    expect(await msLoginStatus()).toBe("enabled");
    tauriInvoke.mockResolvedValueOnce({ ms_login_enabled: false });
    expect(await msLoginStatus()).toBe("no_client_id");
    tauriInvoke.mockRejectedValueOnce(new Error("no backend"));
    expect(await msLoginStatus()).toBe("no_client_id");
  });

  it("Microsoft sign-in delivers the code and the new default account", async () => {
    backend();
    const login = msLoginStart();
    await begun();
    emit("auth:device-code", { task_id: "other", ...code, user_code: "WRONG" });
    emit("auth:device-code", { task_id: "t1", ...code });
    expect(await login.deviceCode).toEqual(code);
    emit("task:finished", { task_id: "t1", state: "succeeded", error: null });
    expect(await login.result).toEqual(steve);
    expect(listeners.size).toBe(0);
  });

  it("replays events that arrive before the task id", async () => {
    backend(() => {
      emit("auth:device-code", { task_id: "t1", ...code });
      emit("task:finished", {
        task_id: "t1",
        state: "failed",
        error: { code: "AUTH_XSTS_CHILD_ACCOUNT", params: {}, detail: null },
      });
    });
    const login = msLoginStart();
    expect(await login.deviceCode).toEqual(code);
    await expect(login.result).rejects.toMatchObject({ code: "AUTH_XSTS_CHILD_ACCOUNT" });
  });

  it("cancel cancels the task", async () => {
    backend();
    const login = msLoginStart();
    await begun();
    login.cancel();
    expect(tauriInvoke).toHaveBeenLastCalledWith("task_cancel", { args: { task_id: "t1" } });
    emit("task:finished", { task_id: "t1", state: "cancelled", error: null });
    await expect(login.result).rejects.toMatchObject({ code: "COMMON_CANCELLED" });
    await expect(login.deviceCode).rejects.toMatchObject({ code: "COMMON_CANCELLED" });
  });

  it("recognizes AUTH_MC_APP_NOT_APPROVED", () => {
    const notApproved = { code: "AUTH_MC_APP_NOT_APPROVED" as const, params: {}, detail: null };
    expect(isAppNotApproved(notApproved)).toBe(true);
    expect(isAppNotApproved({ code: "NET_TIMEOUT", params: {}, detail: null })).toBe(false);
  });
});
