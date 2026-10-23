import { News24Regular } from "@vicons/fluent";
import { defineWidget } from "../types";

export default defineWidget({
  id: "news",
  titleKey: "widget-news.title",
  descriptionKey: "widget-news.description",
  icon: News24Regular,
  defaultSize: { w: 12, h: 3 },
  minSize: { w: 4, h: 2 },
  component: () => import("./ReleaseNotes.vue"),
});
