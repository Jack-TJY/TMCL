<script setup lang="ts">
import type { SeedStructureKind } from "../../bindings/SeedStructureKind";
import { structureIcons } from "./structureIcons";
defineProps<{ kind: SeedStructureKind }>();
</script>
<template>
  <img
    class="structure-icon"
    :src="structureIcons[kind]"
    alt=""
    aria-hidden="true"
    draggable="false"
    width="24"
    height="24"
  />
</template>
<style scoped>
.structure-icon {
  display: block;
  width: 24px;
  height: 24px;
  flex: none;
  image-rendering: pixelated;
  pointer-events: none;
  user-select: none;
}
</style>
