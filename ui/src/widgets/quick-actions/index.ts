import { Flash24Regular } from "@vicons/fluent";
import { defineWidget } from "../types";

export default defineWidget({
  id: "quick-actions",
  titleKey: "widget-quick-actions.title",
  descriptionKey: "widget-quick-actions.description",
  icon: Flash24Regular,
  defaultSize: { w: 4, h: 2 },
  minSize: { w: 3, h: 2 },
  maxSize: { w: 12, h: 4 },
  settingsSchema: [
    {
      key: "labels",
      type: "boolean",
      labelKey: "widget-quick-actions.settings.labels",
      default: true,
    },
  ],
  component: () => import("./QuickActions.vue"),
});
