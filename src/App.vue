<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import AppSidebar from './components/layout/AppSidebar.vue'
import ToastContainer from './components/common/ToastContainer.vue'
import { setupEventListener } from './services/eventListener'

let cleanupEvents: (() => void) | null = null

onMounted(async () => {
  cleanupEvents = await setupEventListener()
})

onUnmounted(() => {
  cleanupEvents?.()
})
</script>

<template>
  <div class="flex h-screen w-screen bg-surface overflow-hidden font-sans text-on-surface">
    <!-- M3 Navigation Rail -->
    <AppSidebar />

    <!-- Main Content Area -->
    <main class="flex-1 h-screen overflow-y-auto px-8 py-7 bg-surface">
      <div class="max-w-5xl mx-auto">
        <router-view />
      </div>
    </main>

    <!-- Global Floating Notifications -->
    <ToastContainer />
  </div>
</template>
