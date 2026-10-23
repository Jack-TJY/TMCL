// 组件的异步加载：每种组件只创建一次异步组件，首页和组件库共用。
import { defineAsyncComponent, markRaw, type Component } from "vue";
import type { WidgetDefinition } from "../../widgets/types";

const cache = new Map<string, Component>();

export function widgetComponent(def: WidgetDefinition): Component {
  let component = cache.get(def.id);
  if (!component) {
    component = markRaw(defineAsyncComponent(def.component));
    cache.set(def.id, component);
  }
  return component;
}
