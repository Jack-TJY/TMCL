import { describe, expect, it, vi } from "vitest";
import {
  createRenderer,
  defineComponent,
  h,
  nextTick,
  ref,
  ssrContextKey,
  type Component,
} from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import type { LoaderSpec } from "../../bindings/LoaderSpec";
import { loaderList } from "../../api/loader";
import LoaderPicker from "./LoaderPicker.vue";
import { useFabricApiWarning } from "./useFabricApiWarning";

import type { AppError } from "../../bindings/AppError";
import type { LoaderVersion } from "../../bindings/LoaderVersion";
import {
  defaultVersion,
  fabricApiDisplayError,
  fabricApiRetryRoute,
  loaderInstallArgs,
  toState,
  visibleVersions,
} from "./loaders";

const handles = vi.hoisted(() => ({
  warning: vi.fn(),
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("naive-ui", () => ({
  NButton: "button",
  NCheckbox: "input",
  NSelect: "select",
  NTag: "span",
  useDialog: () => ({ warning: handles.warning }),
}));
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
vi.mock("../../i18n", () => ({ formatError: () => "" }));
vi.mock("../../api/events", () => ({ onFabricApiFailed: handles.listen }));
vi.mock("../../api/loader", () => ({ loaderList: vi.fn() }));
vi.mock("../../stores/settings", () => ({
  useSettingsStore: () => ({ settings: { download: { source: "official" } } }),
}));

// 使用 Vue 的真实组件更新队列，不依赖浏览器 DOM。这里只断言组件发出的选择和导航。
function appFor(component: Component) {
  const node = () => ({});
  return createRenderer<object, object>({
    createElement: node,
    createText: node,
    createComment: node,
    setText() {},
    setElementText() {},
    patchProp() {},
    insert() {},
    remove() {},
    parentNode: () => null,
    nextSibling: () => null,
    insertStaticContent: () => [node(), node()],
  }).createApp(component);
}
const v = (version: string, stable = true, recommended = false): LoaderVersion => ({
  kind: "forge",
  game_version: "1.20.1",
  version,
  stable,
  recommended,
});

const error = (code: AppError["code"]): AppError => ({ code, params: {}, detail: null });

describe("toState", () => {
  it("treats empty lists and unsupported versions as incompatible", () => {
    expect(toState({ versions: [] })).toEqual({ status: "unsupported" });
    expect(toState(error("LOADER_UNSUPPORTED_VERSION"))).toEqual({ status: "unsupported" });
  });

  it("keeps other errors for retry", () => {
    const err = error("NET_TIMEOUT");
    expect(toState(err)).toEqual({ status: "error", error: err });
  });

  it("returns versions when available", () => {
    const versions = [v("47.4.0")];
    expect(toState({ versions })).toEqual({ status: "ready", versions });
  });
});

describe("defaultVersion", () => {
  it("prefers the recommended version, then the newest stable one", () => {
    expect(defaultVersion([v("2", false), v("1.5"), v("1", true, true)])?.version).toBe("1");
    expect(defaultVersion([v("2", false), v("1.5"), v("1")])?.version).toBe("1.5");
    expect(defaultVersion([v("2-beta", false)])?.version).toBe("2-beta");
    expect(defaultVersion([])).toBeNull();
  });
});

describe("visibleVersions", () => {
  const versions = [v("0.17.0", false), v("0.16.5", true, true), v("0.16.4")];
  const ids = (list: LoaderVersion[]) => list.map((x) => x.version);

  it("hides beta versions by default", () => {
    expect(ids(visibleVersions(versions, false, "0.16.5"))).toEqual(["0.16.5", "0.16.4"]);
  });

  it("shows everything when asked, when a beta is selected or when nothing is stable", () => {
    expect(visibleVersions(versions, true, null)).toHaveLength(3);
    expect(visibleVersions(versions, false, "0.17.0")).toHaveLength(3);
    expect(visibleVersions([v("1-beta", false)], false, null)).toHaveLength(1);
  });
});

describe("loaderInstallArgs", () => {
  it("asks for Fabric API only with Fabric", () => {
    expect(loaderInstallArgs("i", { kind: "fabric", version: "0.16.5" }, true)).toEqual({
      instance_id: "i",
      kind: "fabric",
      version: "0.16.5",
      with_fabric_api: true,
    });
    expect(
      loaderInstallArgs("i", { kind: "fabric", version: "0.16.5" }, false).with_fabric_api,
    ).toBe(false);
    expect(loaderInstallArgs("i", { kind: "quilt", version: "0.26.0" }, true).with_fabric_api).toBe(
      false,
    );
  });
});

it("shortens the displayed URL without changing the original error", () => {
  const url = "https://api.modrinth.com/v2/project/fabric-api/version?loaders=%5B%22fabric%22%5D";
  const source = { ...error("NET_HTTP_STATUS"), params: { url, status: "503" } };
  expect(fabricApiDisplayError(source).params).toEqual({ url: "api.modrinth.com", status: "503" });
  expect(source.params.url).toBe(url);
  expect(fabricApiDisplayError(error("NET_TIMEOUT"))).toEqual(error("NET_TIMEOUT"));
});

describe("fabricApiRetryRoute", () => {
  it("opens the mod tab of the instance's resource page", () => {
    expect(fabricApiRetryRoute("abc")).toEqual({
      name: "instance-resources",
      params: { id: "abc" },
      query: { kind: "mod", view: "browse", search: "Fabric API" },
    });
  });
});

it("keeps the recommended loader when game and model arrive in one parent update", async () => {
  const game = ref<string | null>(null);
  const selection = ref<LoaderSpec | null>(null);
  vi.mocked(loaderList).mockImplementation(async ({ kind, game_version }) => ({
    versions: [{ ...v("0.20.1"), kind, game_version }],
  }));
  const app = appFor(
    defineComponent(
      () => () =>
        h(LoaderPicker, {
          modelValue: selection.value,
          gameVersion: game.value,
          "onUpdate:modelValue": (value) => {
            selection.value = value;
          },
        }),
    ),
  );
  // Vitest 的 Vue 转换会登记组件模块。
  app.provide(ssrContextKey, { modules: new Set() });
  app.mount({});
  try {
    selection.value = { kind: "fabric", version: "" };
    game.value = "26.3";
    await nextTick();
    await vi.waitFor(() => expect(selection.value).toEqual({ kind: "fabric", version: "0.20.1" }));
  } finally {
    app.unmount();
  }
});

it("dispatches recovery again after the same instance's previous recovery", async () => {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: "/", component: {} },
      { path: "/instances/:id/resources", name: "instance-resources", component: {} },
    ],
  });
  const app = appFor(
    defineComponent(() => {
      useFabricApiWarning();
      return () => h("div");
    }),
  );
  app.use(router);
  app.mount({});
  try {
    const recover = () => {
      handles.listen.mock.lastCall![0]({
        instance_id: "abc",
        instance_name: "Test",
        error: error("NET_TIMEOUT"),
      });
      handles.warning.mock.lastCall![0].onPositiveClick();
    };
    recover();
    await vi.waitFor(() => expect(router.currentRoute.value.name).toBe("instance-resources"));
    const first = router.currentRoute.value;
    recover();
    await vi.waitFor(() => expect(router.currentRoute.value).not.toBe(first));
    expect(router.currentRoute.value.fullPath).toBe(first.fullPath);
  } finally {
    app.unmount();
  }
});
