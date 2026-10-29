import { beforeEach, describe, expect, it } from "vitest";
import type { Composer } from "vue-i18n";
import { i18n } from "../../i18n";
import { keyLabel, legacyCodeToKey, optionLabel, valueLabel, type Translator } from "./options";

const composer = i18n.global as unknown as Composer;
const tr: Translator = {
  t: (key, params) => composer.t(key, params ?? {}),
  te: (key) => composer.te(key),
};

describe("option names", () => {
  beforeEach(() => {
    composer.locale.value = "zh-CN";
  });

  it("names options, bindings and sound categories", () => {
    expect(optionLabel("renderDistance", tr)).toBe("渲染距离");
    expect(optionLabel("key_key.jump", tr)).toBe("跳跃");
    expect(optionLabel("key_key.hotbar.3", tr)).toBe("快捷栏 3");
    expect(optionLabel("soundCategory_music", tr)).toBe("音乐");
    expect(optionLabel("modelPart_hat", tr)).toBe("帽子");
  });

  it("returns null for unknown or mod options", () => {
    expect(optionLabel("key_key.jei.showRecipe", tr)).toBeNull();
    expect(optionLabel("key_mod.x", tr)).toBeNull();
    expect(optionLabel("someModOption", tr)).toBeNull();
  });

  it("follows the UI language", () => {
    composer.locale.value = "en-US";
    expect(optionLabel("key_key.jump", tr)).toBe("Jump");
    expect(keyLabel("key.keyboard.left.shift", tr)).toBe("Left Shift");
  });
});

describe("key names", () => {
  beforeEach(() => {
    composer.locale.value = "zh-CN";
  });

  it("formats modern key names", () => {
    expect(keyLabel("key.keyboard.w", tr)).toBe("W");
    expect(keyLabel("key.keyboard.f5", tr)).toBe("F5");
    expect(keyLabel("key.keyboard.space", tr)).toBe("空格");
    expect(keyLabel("key.keyboard.slash", tr)).toBe("/");
    expect(keyLabel("key.keyboard.keypad.7", tr)).toBe("小键盘 7");
    expect(keyLabel("key.keyboard.keypad.add", tr)).toBe("小键盘 +");
    expect(keyLabel("key.mouse.left", tr)).toBe("鼠标左键");
    expect(keyLabel("key.mouse.4", tr)).toBe("鼠标按键 4");
    expect(keyLabel("key.keyboard.unknown", tr)).toBe("未绑定");
  });

  it("decodes legacy LWJGL2 codes", () => {
    expect(legacyCodeToKey(57)).toBe("key.keyboard.space");
    expect(legacyCodeToKey(-100)).toBe("key.mouse.left");
    expect(legacyCodeToKey(-98)).toBe("key.mouse.middle");
    expect(legacyCodeToKey(-96)).toBe("key.mouse.5");
    expect(keyLabel("17", tr)).toBe("W");
    expect(keyLabel("99999", tr)).toBeNull();
  });
});

describe("values", () => {
  beforeEach(() => {
    composer.locale.value = "zh-CN";
  });

  it("formats common values", () => {
    expect(valueLabel("key_key.forward", "key.keyboard.w", tr)).toBe("W");
    expect(valueLabel("bobView", "true", tr)).toBe("开");
    expect(valueLabel("graphicsMode", "2", tr)).toBe("极佳！");
    expect(valueLabel("ao", "2", tr)).toBe("最大");
    expect(valueLabel("soundCategory_master", "0.5", tr)).toBe("50%");
    expect(valueLabel("mouseSensitivity", "0.5", tr)).toBe("100%");
    expect(valueLabel("fov", "0.25", tr)).toBe("80");
    expect(valueLabel("renderDistance", "12", tr)).toBe("12 区块");
    expect(valueLabel("maxFps", "260", tr)).toBe("无限制");
    expect(valueLabel("guiScale", "0", tr)).toBe("自动");
    expect(valueLabel("lang", "zh_CN", tr)).toBe("简体中文");
  });

  it("returns null when it can't explain a value", () => {
    expect(valueLabel("graphicsMode", "9", tr)).toBeNull();
    expect(valueLabel("maxFps", "120", tr)).toBeNull();
    expect(valueLabel("someModOption", "abc", tr)).toBeNull();
  });
});
