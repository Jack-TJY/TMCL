// Original TMCL artwork on a 16×16 pixel grid. No external textures, fonts or spritesheets.
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
import house from "./icons/village.svg";
import gate from "./icons/stronghold.svg";
import pyramid from "./icons/desert_pyramid.svg";
import temple from "./icons/jungle_pyramid.svg";
import monument from "./icons/ocean_monument.svg";
import camp from "./icons/abandoned_camp.svg";
import mansion from "./icons/mansion.svg";
import igloo from "./icons/igloo.svg";
import swampHut from "./icons/swamp_hut.svg";
import outpost from "./icons/pillager_outpost.svg";
import shipwreck from "./icons/shipwreck.svg";
import ancientCity from "./icons/ancient_city.svg";
import trailRuins from "./icons/trail_ruins.svg";
import trialChambers from "./icons/trial_chambers.svg";
import buriedTreasure from "./icons/buried_treasure.svg";
import fortress from "./icons/fortress.svg";
import bastion from "./icons/bastion.svg";

/** One asset per IPC structure ID, shared by map markers and their legend. */
export const structureIcons = {
  village: house,
  stronghold: gate,
  desert_pyramid: pyramid,
  jungle_pyramid: temple,
  ocean_monument: monument,
  mansion,
  igloo,
  swamp_hut: swampHut,
  pillager_outpost: outpost,
  shipwreck,
  ancient_city: ancientCity,
  trail_ruins: trailRuins,
  trial_chambers: trialChambers,
  buried_treasure: buriedTreasure,
  fortress,
  bastion,
  abandoned_camp: camp,
} satisfies Record<SeedStructureKind, string>;
