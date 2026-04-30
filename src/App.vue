<script setup lang="ts">
import DeviceList from './components/DeviceList.vue'
</script>

<template>
  <div class="container">
    <h1>KFileSync (零信任阶段 1)</h1>
    <DeviceList />
  </div>
</template>

<style scoped>
.container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  font-family: "Inter", sans-serif;
  background-color: #0d0d14;
  min-height: 100vh;
  color: #ececec;
}
h1 {
  background: -webkit-linear-gradient(#3498db, #8e44ad);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}
</style>
