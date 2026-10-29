// 3D 预览可选的动作，及与 skinview3d 动画类的对应。
import {
  IdleAnimation,
  RunningAnimation,
  WalkingAnimation,
  WaveAnimation,
  type PlayerAnimation,
} from "skinview3d";
import type { SkinVariant } from "../../api/skin";

export const SKIN_ANIMATIONS = ["idle", "walk", "run", "wave", "none"] as const;
export type SkinAnimation = (typeof SKIN_ANIMATIONS)[number];

/** 创建动作对应的动画；`none` 为静止站立。 */
export function createAnimation(kind: SkinAnimation): PlayerAnimation | null {
  switch (kind) {
    case "idle":
      return new IdleAnimation();
    case "walk":
      return new WalkingAnimation();
    case "run":
      return new RunningAnimation();
    case "wave":
      return new WaveAnimation();
    case "none":
      return null;
  }
}

/** skinview3d 的模型名：Steve 为 `default`，Alex 为 `slim`。 */
export function viewerModel(variant: SkinVariant): "default" | "slim" {
  return variant === "slim" ? "slim" : "default";
}
