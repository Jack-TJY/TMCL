<script setup lang="ts">
import DemoTask from "./components/DemoTask.vue";
import LocaleSwitcher from "./components/LocaleSwitcher.vue";
</script>

<template>
  <main class="shell">
    <h1>TMCL</h1>
    <DemoTask />
    <LocaleSwitcher />
  </main>
</template>

<style>
html,
body,
#app {
  height: 100%;
  margin: 0;
}

body {
  font-family:
    "Segoe UI Variable", "Segoe UI", system-ui, "Microsoft YaHei", "PingFang SC", sans-serif;
}

.shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  align-items: center;
  justify-content: center;
}
</style>
