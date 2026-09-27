<script setup lang="ts">
import { useNotificationStore } from '../../stores/notifications'
import {
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  Info,
  X,
} from 'lucide-vue-next'

const store = useNotificationStore()

function getIcon(type: string) {
  switch (type) {
    case 'success':
      return CheckCircle2
    case 'error':
      return AlertCircle
    case 'warning':
      return AlertTriangle
    case 'info':
    default:
      return Info
  }
}

function getStyle(type: string) {
  switch (type) {
    case 'success':
      return 'bg-surface-container-high border-success/40 text-on-surface [&_.icon]:text-success'
    case 'error':
      return 'bg-surface-container-high border-error/40 text-on-surface [&_.icon]:text-error'
    case 'warning':
      return 'bg-surface-container-high border-yellow-500/40 text-on-surface [&_.icon]:text-yellow-400'
    case 'info':
    default:
      return 'bg-surface-container-high border-primary/40 text-on-surface [&_.icon]:text-primary'
  }
}
</script>

<template>
  <div class="fixed bottom-6 right-6 z-50 flex flex-col gap-2.5 max-w-sm w-full pointer-events-none">
    <transition-group
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="transform translate-y-3 opacity-0 scale-95"
      enter-to-class="transform translate-y-0 opacity-100 scale-100"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="transform translate-y-0 opacity-100 scale-100"
      leave-to-class="transform translate-y-2 opacity-0 scale-95"
    >
      <div
        v-for="n in store.notifications"
        :key="n.id"
        :class="[
          'pointer-events-auto flex items-center justify-between gap-3 p-3.5 pl-4 rounded-2xl border shadow-m3-3 backdrop-blur-md cursor-pointer transition-all',
          getStyle(n.type)
        ]"
        @click="store.remove(n.id)"
      >
        <div class="flex items-center gap-3 min-w-0">
          <component :is="getIcon(n.type)" class="icon w-5 h-5 flex-shrink-0" />
          <span class="text-xs font-medium tracking-wide break-words">{{ n.message }}</span>
        </div>
        <button class="text-on-surface-muted hover:text-on-surface p-1 rounded-full hover:bg-on-surface/8">
          <X class="w-3.5 h-3.5" />
        </button>
      </div>
    </transition-group>
  </div>
</template>