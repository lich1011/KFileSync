<script setup lang="ts">
import DeviceList from './components/DeviceList.vue'
import AppSidebar from './components/layout/AppSidebar.vue';
import ToastContainer from './components/common/ToastContainer.vue';
</script>

<template>
  <div class="app-layout">
    <AppSidebar />
    <main class = "app-main">
      <router-view />
    </main>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;

  min-height: 100vh;
}
.app-main {
  flex: 1;
  padding: 24px 32px;
  overflow-y: auto;
}
</style>
