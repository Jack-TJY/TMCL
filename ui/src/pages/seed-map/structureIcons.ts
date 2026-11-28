// Original TMCL artwork on a 16×16 pixel grid. No external textures, fonts or spritesheets.
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
import house from "./icons/village.svg";
import gate from "./icons/stronghold.svg";
import pyramid from "./icons/desert_pyramid.svg";
import temple from "./icons/jungle_pyramid.svg";
import monument from "./icons/ocean_monument.svg";
import camp from "./icons/abandoned_camp.svg";
import mansion from "./icons/mansion.svg";

/** One asset per IPC structure ID, shared by map markers and their legend. */
export const structureIcons = {
  village: house,
  stronghold: gate,
  desert_pyramid: pyramid,
  jungle_pyramid: temple,
  ocean_monument: monument,
  mansion,
  abandoned_camp: camp,
} satisfies Record<SeedStructureKind, string>;
