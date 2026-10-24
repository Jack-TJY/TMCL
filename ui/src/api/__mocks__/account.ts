// 微软登录尚未完成（M0-10，等待 Mojang 审核），按 AGENTS.md §6.3 先用替身走通设备码弹窗。
// 只在没有后端的浏览器调试中使用（见 `msLoginStatus`）；M0-10 提供 `auth_ms_begin` 后删除本文件。
import type { AppError } from "../../bindings/AppError";
import type { Account, DeviceCode, MsLogin } from "../account";

/**
 * 替身的结果，由 localStorage 的 `tmcl.mock.msOutcome` 控制：
 * 缺省为成功；`not_approved` 模拟目前 `login_with_xbox` 返回 403 的情况。
 */
const MS_OUTCOME_KEY = "tmcl.mock.msOutcome";
/** 拿到设备码、以及“用户在浏览器中完成登录”所需的模拟时间。 */
const MS_CODE_DELAY_MS = 600;
const MS_LOGIN_DELAY_MS = 8000;

function outcome(): string | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage.getItem(MS_OUTCOME_KEY);
  } catch {
    return null;
  }
}

function fail(code: string): AppError {
  return { code: code as AppError["code"], params: {}, detail: null };
}

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
        if (outcome() === "not_approved") {
          reject(fail("AUTH_MC_APP_NOT_APPROVED"));
          return;
        }
        // 替身不写入账号列表，只返回一个账号供弹窗显示结果。
        resolve({
          id: "mock-microsoft",
          kind: "microsoft",
          name: "MockPlayer",
          uuid: "00000000-0000-0000-0000-000000000000",
          server: null,
        });
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
