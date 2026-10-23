// 按游戏版本并行获取四种加载器的版本列表；成功的结果在本次运行中缓存，失败的可重试。
import { reactive, watch, type Ref } from "vue";
import { toAppError } from "../../api/invoke";
import { loaderList } from "../../api/loader";
import type { LoaderKind } from "../../bindings/LoaderKind";
import { LOADER_KINDS, toState, type LoaderState } from "./loaders";

const cache = new Map<string, LoaderState>();

function cacheKey(kind: LoaderKind, gameVersion: string): string {
  return `${kind}@${gameVersion}`;
}

/** 返回各加载器的状态（随 `gameVersion` 变化重新获取）与单个加载器的重试函数。 */
export function useLoaderVersions(gameVersion: Ref<string | null>) {
  const states = reactive({} as Record<LoaderKind, LoaderState>);

  async function load(kind: LoaderKind, game: string) {
    const key = cacheKey(kind, game);
    const cached = cache.get(key);
    if (cached) {
      states[kind] = cached;
      return;
    }
    states[kind] = { status: "loading" };
    const state = toState(await loaderList({ kind, game_version: game }).catch(toAppError));
    if (state.status !== "error") cache.set(key, state);
    // 等待期间游戏版本可能已经改变。
    if (gameVersion.value === game) states[kind] = state;
  }

  watch(
    gameVersion,
    (game) => {
      for (const kind of LOADER_KINDS) {
        if (game) void load(kind, game);
        else states[kind] = { status: "unsupported" };
      }
    },
    { immediate: true },
  );

  function retry(kind: LoaderKind) {
    if (gameVersion.value) void load(kind, gameVersion.value);
  }

  return { states, retry };
}
