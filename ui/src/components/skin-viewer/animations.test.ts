import { describe, expect, it } from "vitest";
import { IdleAnimation, RunningAnimation, WalkingAnimation, WaveAnimation } from "skinview3d";
import { SKIN_ANIMATIONS, createAnimation, viewerModel } from "./animations";

describe("skin viewer animations", () => {
  it("maps every animation kind", () => {
    expect(createAnimation("idle")).toBeInstanceOf(IdleAnimation);
    expect(createAnimation("walk")).toBeInstanceOf(WalkingAnimation);
    expect(createAnimation("run")).toBeInstanceOf(RunningAnimation);
    expect(createAnimation("wave")).toBeInstanceOf(WaveAnimation);
    expect(createAnimation("none")).toBeNull();
    expect(SKIN_ANIMATIONS).toHaveLength(5);
  });

  it("maps skin variants to viewer models", () => {
    expect(viewerModel("classic")).toBe("default");
    expect(viewerModel("slim")).toBe("slim");
  });
});
