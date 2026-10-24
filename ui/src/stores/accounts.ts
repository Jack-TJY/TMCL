// 账号状态（PLAN.md M1-14）：账号页、“当前账号”组件与启动游戏共用同一份列表。
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import {
  accountAddOffline,
  accountList,
  accountRemove,
  accountSetDefault,
  launchAccountArgs,
  type Account,
  type AccountList,
} from "../api/account";
import type { AppError } from "../bindings/AppError";
import type { LaunchAccountArgs } from "../bindings/LaunchAccountArgs";

export const useAccountsStore = defineStore("accounts", () => {
  const accounts = ref<Account[]>([]);
  const defaultId = ref<string | null>(null);
  const loaded = ref(false);
  let pending: Promise<void> | null = null;

  /** 启动游戏时使用的账号。 */
  const current = computed(() => accounts.value.find((a) => a.id === defaultId.value) ?? null);

  function apply(list: AccountList) {
    accounts.value = list.accounts;
    defaultId.value = list.default_id;
    loaded.value = true;
  }

  /** 重新读取列表；并发调用共用同一次请求。失败时抛出 AppError。 */
  function load(): Promise<void> {
    pending ??= accountList()
      .then(apply)
      .finally(() => (pending = null));
    return pending;
  }

  /** 首次使用时读取列表。 */
  async function ensureLoaded() {
    if (!loaded.value) await load();
  }

  async function setDefault(id: string) {
    await accountSetDefault({ id });
    await load();
  }

  async function remove(id: string) {
    await accountRemove({ id });
    await load();
  }

  async function addOffline(name: string): Promise<Account> {
    const account = await accountAddOffline({ name });
    await load();
    return account;
  }

  /** 启动参数中的账号；没有可用账号时抛出 `AUTH_NO_ACCOUNT`。 */
  async function launchAccount(): Promise<LaunchAccountArgs> {
    await ensureLoaded();
    if (!current.value) {
      const error: AppError = { code: "AUTH_NO_ACCOUNT", params: {}, detail: null };
      throw error;
    }
    return launchAccountArgs(current.value);
  }

  return {
    accounts,
    defaultId,
    loaded,
    current,
    load,
    ensureLoaded,
    setDefault,
    remove,
    addOffline,
    launchAccount,
  };
});
