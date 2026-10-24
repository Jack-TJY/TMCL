import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

const tauriInvoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: tauriInvoke }));

import type { Account } from "../bindings/Account";
import { useAccountsStore } from "./accounts";

/** 内存中的最小后端：只模拟 `auth_*` 命令的列表与默认账号语义。 */
function fakeBackend(initial: Account[]) {
  let accounts = [...initial];
  let defaultId: string | null = accounts[0]?.id ?? null;
  tauriInvoke.mockImplementation(
    async (cmd: string, payload?: { args: Record<string, string> }) => {
      const args = payload?.args ?? {};
      switch (cmd) {
        case "auth_list":
          return { accounts, default_id: defaultId };
        case "auth_set_default":
          defaultId = args.id;
          return null;
        case "auth_remove":
          accounts = accounts.filter((a) => a.id !== args.id);
          if (defaultId === args.id) defaultId = accounts[0]?.id ?? null;
          return null;
        case "auth_add_offline": {
          if (!/^[A-Za-z0-9_]{3,16}$/.test(args.name))
            throw { code: "AUTH_OFFLINE_INVALID_NAME", params: { name: args.name }, detail: null };
          const account: Account = {
            id: `id-${args.name}`,
            kind: "offline",
            name: args.name,
            uuid: "u",
            server: null,
          };
          accounts.push(account);
          defaultId = account.id;
          return account;
        }
        default:
          throw new Error(`unexpected ${cmd}`);
      }
    },
  );
}

const player: Account = { id: "p", kind: "offline", name: "Player", uuid: "u", server: null };
const alex: Account = { id: "x", kind: "authlib", name: "Alex", uuid: "u2", server: null };

describe("accounts store", () => {
  beforeEach(() => {
    tauriInvoke.mockReset();
    setActivePinia(createPinia());
  });

  it("loads the list and launches by account id", async () => {
    fakeBackend([player, alex]);
    const store = useAccountsStore();
    await store.load();
    expect(store.current).toEqual(player);
    expect(await store.launchAccount()).toEqual({ kind: "account", id: "p" });
  });

  it("refreshes after adding, switching and removing", async () => {
    fakeBackend([player]);
    const store = useAccountsStore();
    const steve = await store.addOffline("Steve");
    expect(store.defaultId).toBe(steve.id);
    await store.setDefault("p");
    expect(store.current?.name).toBe("Player");
    await store.remove("p");
    expect(store.current?.name).toBe("Steve");
    expect(tauriInvoke).toHaveBeenCalledWith("auth_remove", { args: { id: "p" } });
  });

  it("passes backend errors through", async () => {
    fakeBackend([]);
    const store = useAccountsStore();
    await expect(store.addOffline("a b")).rejects.toMatchObject({
      code: "AUTH_OFFLINE_INVALID_NAME",
    });
  });

  it("throws AUTH_NO_ACCOUNT when there is no account", async () => {
    fakeBackend([]);
    const store = useAccountsStore();
    await expect(store.launchAccount()).rejects.toMatchObject({ code: "AUTH_NO_ACCOUNT" });
  });
});
