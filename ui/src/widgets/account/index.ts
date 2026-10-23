import { PersonCircle24Regular } from "@vicons/fluent";
import { defineWidget } from "../types";

export default defineWidget({
  id: "account",
  titleKey: "widget-account.title",
  descriptionKey: "widget-account.description",
  icon: PersonCircle24Regular,
  defaultSize: { w: 4, h: 2 },
  minSize: { w: 3, h: 2 },
  maxSize: { w: 6, h: 3 },
  component: () => import("./CurrentAccount.vue"),
});
