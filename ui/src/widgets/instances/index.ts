import { AppsList24Regular } from "@vicons/fluent";
import { defineWidget } from "../types";

export default defineWidget({
  id: "instances",
  titleKey: "widget-instances.title",
  descriptionKey: "widget-instances.description",
  icon: AppsList24Regular,
  defaultSize: { w: 6, h: 4 },
  minSize: { w: 3, h: 3 },
  settingsSchema: [
    {
      key: "count",
      type: "number",
      labelKey: "widget-instances.settings.count",
      default: 5,
      min: 1,
      max: 20,
    },
  ],
  component: () => import("./InstanceShortcuts.vue"),
});
