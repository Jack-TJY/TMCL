import { describe, expect, it } from "vitest";
import {
  controlFor,
  defaultValue,
  fromDisplay,
  KNOWN_SETTINGS,
  keyFromEvent,
  keyFromMouse,
  toDisplay,
} from "./controls";

describe("controls", () => {
  it("picks a control per option", () => {
    expect(controlFor("enableVsync", "true").kind).toBe("switch");
    expect(controlFor("graphicsMode", "1")).toEqual({ kind: "select", values: ["0", "1", "2"] });
    expect(controlFor("renderDistance", "12").kind).toBe("number");
    expect(controlFor("key_key.jump", "key.keyboard.space").kind).toBe("key");
    expect(controlFor("soundCategory_music", "0.5").kind).toBe("number");
    expect(controlFor("modelPart_cape", "true").kind).toBe("switch");
    expect(controlFor("lang", "zh_cn").kind).toBe("select");
    expect(controlFor("someMod.flag", "false").kind).toBe("switch");
    expect(controlFor("someMod.name", "abc").kind).toBe("text");
  });

  it("switches smooth lighting between old and new formats", () => {
    expect(controlFor("ao", "2")).toEqual({ kind: "select", values: ["0", "1", "2"] });
    expect(controlFor("ao", "true").kind).toBe("switch");
  });

  it("converts numbers to game units and back", () => {
    const fov = controlFor("fov", "0.0");
    expect(toDisplay(fov, "0.0")).toBe(70);
    expect(toDisplay(fov, "1.0")).toBe(110);
    expect(fromDisplay(fov, 90)).toBe("0.5");
    expect(fromDisplay(fov, 200)).toBe("1");
    const sens = controlFor("mouseSensitivity", "0.5");
    expect(toDisplay(sens, "0.5")).toBe(100);
    expect(fromDisplay(sens, 0)).toBe("0");
    const volume = controlFor("soundCategory_master", "1.0");
    expect(toDisplay(volume, "0.35")).toBe(35);
    expect(fromDisplay(volume, 35)).toBe("0.35");
    const rd = controlFor("renderDistance", "12");
    expect(fromDisplay(rd, 12.6)).toBe("13");
    expect(fromDisplay(rd, 1)).toBe("2");
    expect(toDisplay(rd, "abc")).toBeNull();
    expect(toDisplay(rd, "")).toBeNull();
  });

  it("maps key and mouse events to game key names", () => {
    expect(keyFromEvent("KeyW")).toBe("key.keyboard.w");
    expect(keyFromEvent("Digit1")).toBe("key.keyboard.1");
    expect(keyFromEvent("Numpad5")).toBe("key.keyboard.keypad.5");
    expect(keyFromEvent("F3")).toBe("key.keyboard.f3");
    expect(keyFromEvent("ShiftLeft")).toBe("key.keyboard.left.shift");
    expect(keyFromEvent("Escape")).toBe("key.keyboard.unknown");
    expect(keyFromEvent("IntlRo")).toBeNull();
    expect(keyFromMouse(0)).toBe("key.mouse.left");
    expect(keyFromMouse(2)).toBe("key.mouse.right");
    expect(keyFromMouse(3)).toBe("key.mouse.4");
  });

  it("gives sensible defaults for new options", () => {
    expect(defaultValue("fov")).toBe("0.0");
    expect(defaultValue("chatOpacity")).toBe("1.0");
    expect(defaultValue("enableVsync")).toBe("false");
    expect(defaultValue("graphicsMode")).toBe("0");
    expect(defaultValue("key_key.jump")).toBe("key.keyboard.unknown");
    expect(defaultValue("someMod.name")).toBe("");
    expect(new Set(KNOWN_SETTINGS).size).toBe(KNOWN_SETTINGS.length);
    expect(KNOWN_SETTINGS).toContain("renderDistance");
  });
});
