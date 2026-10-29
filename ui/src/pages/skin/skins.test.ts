import { describe, expect, it } from "vitest";
import { fileStem, firstPng, headStyle } from "./skins";

describe("skin page helpers", () => {
  it("crops face and hat layers for both skin heights", () => {
    const modern = headStyle("blob:a", 64);
    expect(modern.backgroundSize).toBe("800% 800%");
    expect(modern.backgroundPosition).toBe(
      `${(40 / 56) * 100}% ${(8 / 56) * 100}%, ${(8 / 56) * 100}% ${(8 / 56) * 100}%`,
    );
    const legacy = headStyle("blob:b", 32);
    expect(legacy.backgroundSize).toBe("800% 400%");
    expect(legacy.backgroundPosition).toContain(`${(8 / 24) * 100}%`);
    expect(legacy.backgroundImage).toBe('url("blob:b"), url("blob:b")');
  });

  it("picks the first png from dropped paths", () => {
    expect(firstPng(["a.txt", "C:\\skins\\Steve.PNG", "b.png"])).toBe("C:\\skins\\Steve.PNG");
    expect(firstPng(["a.jpg"])).toBeNull();
  });

  it("extracts file stems", () => {
    expect(fileStem("C:\\skins\\My Skin.png")).toBe("My Skin");
    expect(fileStem("/home/u/alex.png")).toBe("alex");
    expect(fileStem("noext")).toBe("noext");
  });
});
