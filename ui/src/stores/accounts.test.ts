import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it } from "vitest";
import { mockAccountPut, mockAccountReset } from "../api/__mocks__/account";
import { useAccountsStore } from "./accounts";

describe("accounts store", () => {
  beforeEach(() => {
    mockAccountReset();
    setActivePinia(createPinia());
  });

  it("starts with the offline Player account as default", async () => {
    const store = useAccountsStore();
    await store.load();
    expect(store.accounts).toHaveLength(1);
    expect(store.current).toMatchObject({ kind: "offline", name: "Player" });
    expect(await store.launchAccount()).toEqual({ kind: "offline", name: "Player" });
  });

  it("makes a newly added offline account the default", async () => {
    const store = useAccountsStore();
    const steve = await store.addOffline("Steve");
    expect(store.defaultId).toBe(steve.id);
    expect(store.accounts.map((a) => a.name)).toEqual(["Player", "Steve"]);
  });

  it("keeps one account per offline name", async () => {
    const store = useAccountsStore();
    const first = await store.addOffline("Steve");
    await store.setDefault(store.accounts[0].id);
    const again = await store.addOffline("Steve");
    expect(again.id).toBe(first.id);
    expect(store.accounts).toHaveLength(2);
    expect(store.defaultId).toBe(first.id);
  });

  it("rejects invalid offline names", async () => {
    const store = useAccountsStore();
    await expect(store.addOffline("ab")).rejects.toMatchObject({
      code: "AUTH_OFFLINE_INVALID_NAME",
      params: { name: "ab" },
    });
    await expect(store.addOffline("bad name")).rejects.toMatchObject({
      code: "AUTH_OFFLINE_INVALID_NAME",
    });
  });

  it("falls back to the first account when the default is removed", async () => {
    const store = useAccountsStore();
    await store.load();
    const player = store.accounts[0];
    const steve = await store.addOffline("Steve");
    await store.remove(steve.id);
    expect(store.defaultId).toBe(player.id);
  });

  it("throws AUTH_NO_ACCOUNT when every account is removed", async () => {
    const store = useAccountsStore();
    await store.load();
    await store.remove(store.accounts[0].id);
    expect(store.current).toBeNull();
    await expect(store.launchAccount()).rejects.toMatchObject({ code: "AUTH_NO_ACCOUNT" });
  });

  it("launches online accounts under their player name until M1-08", async () => {
    const store = useAccountsStore();
    mockAccountPut({ kind: "microsoft", name: "Alex", uuid: "u-1", server: null });
    await store.load();
    expect(await store.launchAccount()).toEqual({ kind: "offline", name: "Alex" });
  });
});
