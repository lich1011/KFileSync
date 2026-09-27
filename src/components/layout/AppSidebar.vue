<script setup lang="ts">
import { useRoute } from 'vue-router'
import {
  Laptop,
  ArrowLeftRight,
  FolderSync,
  RefreshCw,
  Wifi,
} from 'lucide-vue-next'

const route = useRoute()

const navItems = [
  { path: '/devices', label: '设备发现', icon: Laptop },
  { path: '/transfers', label: '文件传输', icon: ArrowLeftRight },
  { path: '/shares', label: '目录共享', icon: FolderSync },
  { path: '/sync', label: '数据同步', icon: RefreshCw },
]
</script>

<template>
  <nav class="w-56 h-screen flex-shrink-0 bg-surface-container-low border-r border-outline-variant/30 flex flex-col justify-between select-none">
    <div>
      <!-- Brand / App Header -->
      <div class="px-5 pt-6 pb-4 flex items-center gap-3">
        <div class="h-9 w-9 rounded-2xl bg-primary flex items-center justify-center text-primary-foreground shadow-sm">
          <FolderSync class="w-5 h-5" />
        </div>
        <div>
          <h1 class="text-base font-bold tracking-tight text-on-surface">KFileSync</h1>
          <p class="text-[11px] font-medium text-on-surface-muted">局域网极速同步</p>
        </div>
      </div>

      <!-- Navigation Items (M3 Navigation Rail / Drawer style) -->
      <ul class="px-3 py-2 space-y-1">
        <li v-for="item in navItems" :key="item.path">
          <router-link
            :to="item.path"
            class="group relative flex items-center gap-3.5 px-4 py-2.5 rounded-full text-sm font-medium transition-all duration-150"
            :class="[
              route.path === item.path
                ? 'bg-primary-container text-primary-on-container font-semibold shadow-sm'
                : 'text-on-surface-variant hover:bg-surface-container-high hover:text-on-surface active:scale-[0.98]'
            ]"
          >
            <component
              :is="item.icon"
              class="w-5 h-5 flex-shrink-0 transition-transform duration-150 group-hover:scale-105"
            />
            <span class="tracking-wide">{{ item.label }}</span>
          </router-link>
        </li>
      </ul>
    </div>

    <!-- Bottom Status Bar -->
    <div class="p-3.5 m-3 rounded-2xl bg-surface-container border border-outline-variant/30 flex items-center gap-2.5">
      <div class="relative flex items-center justify-center">
        <span class="absolute inline-flex h-2.5 w-2.5 rounded-full bg-success/40 animate-ping" />
        <span class="relative inline-flex rounded-full h-2 w-2 bg-success" />
      </div>
      <div class="flex-1 min-w-0">
        <p class="text-xs font-semibold text-on-surface truncate">局域网服务运行中</p>
        <p class="text-[10px] text-on-surface-muted flex items-center gap-1">
          <Wifi class="w-3 h-3 text-success" /> 发现端口 37788
        </p>
      </div>
    </div>
  </nav>
</template>