// 账号管理尚未完成（M1-08），按 AGENTS.md §6.3 先用替身：账号列表保存在 WebView 的 localStorage 中，
// 只为让界面可用，不是持久化方案（真实实现中令牌存系统密钥库，元数据存 SQLite）。
// M1-08 提供 `account_*` 命令后，`ui/src/api/account.ts` 改为调用命令，并删除本文件。
import type { AppError } from "../../bindings/AppError";
import type {
  Account,
  AccountAddAuthlibArgs,
  AccountAddOfflineArgs,
  AccountIdArgs,
  AccountList,
  AuthlibServer,
  AuthlibServerArgs,
  DeviceCode,
  MsLogin,
} from "../account";

const STORAGE_KEY = "tmcl.mock.accounts";
/** M1-07 的替身账号用过的 key：存在时作为首个离线账号的用户名。 */
const LEGACY_NAME_KEY = "tmcl.mock.offlineName";
const DEFAULT_NAME = "Player";
const OFFLINE_NAME = /^[A-Za-z0-9_]{3,16}$/;

/** localStorage 不可用（如单元测试）时的内存副本。 */
let memory: AccountList | null = null;

function storage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

function newId(): string {
  return crypto.randomUUID();
}

/** 首次使用时放入一个离线账号，与 M1-07 的替身行为一致（玩家名 `Player`）。 */
function seed(): AccountList {
  const name = storage()?.getItem(LEGACY_NAME_KEY) || DEFAULT_NAME;
  const account: Account = { id: newId(), kind: "offline", name, uuid: newId(), server: null };
  return { accounts: [account], default_id: account.id };
}

function read(): AccountList {
  const store = storage();
  if (!store) return (memory ??= seed());
  try {
    const raw = store.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as AccountList;
      if (Array.isArray(parsed.accounts)) return parsed;
    }
  } catch {
    // 数据损坏时重新生成。
  }
  const fresh = seed();
  write(fresh);
  return fresh;
}

function write(list: AccountList) {
  const store = storage();
  try {
    if (store) {
      store.setItem(STORAGE_KEY, JSON.stringify(list));
      return;
    }
  } catch {
    // 写入失败时退回内存副本。
  }
  memory = list;
}

function fail(code: AppError["code"], params: Record<string, string> = {}): AppError {
  return { code, params, detail: null };
}

/** 加入一个账号并设为默认；同类型同 UUID 的账号视为同一个，只更新其余字段。 */
export function mockAccountPut(account: Omit<Account, "id">): Account {
  const list = read();
  const existing = list.accounts.find((a) => a.kind === account.kind && a.uuid === account.uuid);
  const saved = existing ? Object.assign(existing, account) : { id: newId(), ...account };
  if (!existing) list.accounts.push(saved);
  list.default_id = saved.id;
  write(list);
  return { ...saved };
}

export async function mockAccountList(): Promise<AccountList> {
  const list = read();
  return { accounts: list.accounts.map((a) => ({ ...a })), default_id: list.default_id };
}

export async function mockAccountSetDefault({ id }: AccountIdArgs): Promise<void> {
  const list = read();
  if (!list.accounts.some((a) => a.id === id)) throw fail("AUTH_NO_ACCOUNT");
  list.default_id = id;
  write(list);
}

export async function mockAccountRemove({ id }: AccountIdArgs): Promise<void> {
  const list = read();
  list.accounts = list.accounts.filter((a) => a.id !== id);
  if (list.default_id === id) list.default_id = list.accounts[0]?.id ?? null;
  write(list);
}

export async function mockAccountAddOffline({ name }: AccountAddOfflineArgs): Promise<Account> {
  if (!OFFLINE_NAME.test(name)) throw fail("AUTH_OFFLINE_INVALID_NAME", { name });
  // 真实实现的 UUID 由用户名决定（M1-08），同名离线账号只会有一个。
  const same = read().accounts.find((a) => a.kind === "offline" && a.name === name);
  return mockAccountPut({ kind: "offline", name, uuid: same?.uuid ?? newId(), server: null });
}

/**
 * 微软登录替身的结果，由 localStorage 的 `tmcl.mock.msOutcome` 控制：
 * 缺省为成功；`not_approved` 模拟目前 `login_with_xbox` 返回 403 的情况。
 */
const MS_OUTCOME_KEY = "tmcl.mock.msOutcome";
/** 拿到设备码、以及“用户在浏览器中完成登录”所需的模拟时间。 */
const MS_CODE_DELAY_MS = 600;
const MS_LOGIN_DELAY_MS = 8000;

export function mockMsLoginStart(): MsLogin {
  const timers: ReturnType<typeof setTimeout>[] = [];
  let rejectResult: (error: AppError) => void = () => undefined;
  const deviceCode = new Promise<DeviceCode>((resolve) => {
    timers.push(
      setTimeout(
        () =>
          resolve({
            user_code: "TMCL-MOCK",
            verification_uri: "https://www.microsoft.com/link",
            expires_in: 900,
          }),
        MS_CODE_DELAY_MS,
      ),
    );
  });
  const result = new Promise<Account>((resolve, reject) => {
    rejectResult = reject;
    timers.push(
      setTimeout(() => {
        if (storage()?.getItem(MS_OUTCOME_KEY) === "not_approved") {
          reject(fail("AUTH_MC_APP_NOT_APPROVED" as AppError["code"]));
          return;
        }
        resolve(
          mockAccountPut({ kind: "microsoft", name: "MockPlayer", uuid: "mock-ms", server: null }),
        );
      }, MS_CODE_DELAY_MS + MS_LOGIN_DELAY_MS),
    );
  });
  return {
    deviceCode,
    result,
    cancel() {
      timers.forEach(clearTimeout);
      rejectResult(fail("COMMON_CANCELLED"));
    },
  };
}

export async function mockAuthlibServerInfo({
  api_url,
}: AuthlibServerArgs): Promise<AuthlibServer> {
  const input = api_url.trim();
  let url: URL;
  try {
    url = new URL(/^https?:\/\//i.test(input) ? input : `https://${input}`);
  } catch {
    throw fail("NET_REQUEST", { url: input });
  }
  return { api_url: url.href.replace(/\/$/, ""), name: url.hostname };
}

export async function mockAccountAddAuthlib({
  api_url,
  username,
}: AccountAddAuthlibArgs): Promise<Account> {
  const server = await mockAuthlibServerInfo({ api_url });
  // 皮肤站的角色名不一定等于登录名；替身取邮箱 @ 之前的部分。
  const name =
    username
      .split("@")[0]
      .replace(/[^A-Za-z0-9_]/g, "_")
      .slice(0, 16) || "Player";
  return mockAccountPut({ kind: "authlib", name, uuid: `${server.api_url}#${username}`, server });
}

/** 仅供单元测试：清空内存副本。 */
export function mockAccountReset() {
  memory = null;
  storage()?.removeItem(STORAGE_KEY);
}
