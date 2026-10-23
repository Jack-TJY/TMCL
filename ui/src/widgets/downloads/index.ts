import { ArrowDownload24Regular } from "@vicons/fluent";
import { defineWidget } from "../types";

export default defineWidget({
  id: "downloads",
  titleKey: "widget-downloads.title",
  descriptionKey: "widget-downloads.description",
  icon: ArrowDownload24Regular,
  defaultSize: { w: 6, h: 4 },
  minSize: { w: 4, h: 3 },
  settingsSchema: [
    {
      key: "showFinished",
      type: "boolean",
      labelKey: "widget-downloads.settings.showFinished",
      default: true,
    },
  ],
  component: () => import("./DownloadTasks.vue"),
});
