import { Rocket24Regular } from "@vicons/fluent";
import { defineWidget } from "../types";

export default defineWidget({
  id: "start-game",
  titleKey: "widget-start-game.title",
  descriptionKey: "widget-start-game.description",
  icon: Rocket24Regular,
  defaultSize: { w: 8, h: 4 },
  minSize: { w: 4, h: 3 },
  maxSize: { w: 12, h: 6 },
  settingsSchema: [
    {
      key: "instance",
      type: "instance",
      labelKey: "widget-start-game.settings.instance",
      default: null,
    },
  ],
  component: () => import("./StartGame.vue"),
});
