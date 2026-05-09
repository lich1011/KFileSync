<script setup lang="ts">
import { useNotificationStore } from '../../stores/notifications'

const store = useNotificationStore()

function typeClass(type: string) {
  return `toast-${type}`
}
</script>

<template>
  <div class="toast-container">
    <transition-group name="toast">
      <div
        v-for="n in store.notifications"
        :key="n.id"
        class="toast"
        :class="typeClass(n.type)"
        @click="store.remove(n.id)"
      >
        {{ n.message }}
      </div>
    </transition-group>
  </div>
</template>

<style scoped>
.toast-container {
  position: fixed;
  top: 16px;
  right: 16px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-width: 360px;
}

.toast {
  padding: 12px 16px;
  border-radius: 8px;
  color: #fff;
  font-size: 13px;
  cursor: pointer;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
}

.toast--success { background: var(--success); }
.toast--error { background: var(--danger); }
.toast--warning { background: var(--warning); color: #1a1a1a; }
.toast--info { background: var(--accent); }

/* 动画过渡 */
.toast-enter-active { transition: all 0.3s ease; }
.toast-leave-active { transition: all 0.2s ease; }
.toast-enter-from { opacity: 0; transform: translateX(40px); }
.toast-leave-to { opacity: 0; transform: translateX(40px); }
</style>