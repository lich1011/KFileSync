<script setup lang="ts">
import { onMounted } from 'vue'
import { useDeviceStore } from '../stores/devices'

const store = useDeviceStore()

onMounted(() => {
  store.fetchDevices()
})
</script>

<template>
  <div class="device-list">
    <div class="device-list__header">
      <h2>附近设备</h2>
      <button 
        class="primary" 
        @click="store.fetchDevices()" 
        :disabled="store.loading"
      >
        {{ store.loading ? '扫描中...' : '刷新' }}
      </button>
    </div>

    <div v-if="store.loading && store.devices.length === 0" class="empty">
      正在扫描局域网设备...
    </div>
    
    <div v-else-if="store.devices.length === 0" class="empty">
      未发现设备，请确保其他设备在同一局域网中
    </div>

    <ul v-else>
      <li v-for="device in store.devices" :key="device.id" class="device-item">
        <div class="device-info">
          <div class="device-alias">{{ device.alias }}</div>
          <div class="device-meta">
            {{ device.address }}
            <span class="device-status" :class="device.status.toLowerCase()">
              {{ device.status }}
            </span>
          </div>
        </div>
        
        <button 
          v-if="device.status === 'Discovered'" 
          class="primary" 
          @click="store.requestPairing(device.id)"
        >
          配对
        </button>
        <span v-else-if="device.status === 'Paired'" class="badge badge--success">
          已配对
        </span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.device-list {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 20px;
}

.device-list__header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.device-list__header h2 {
  font-size: 16px;
  font-weight: 600;
}

ul {
  list-style: none;
  padding: 0;
  margin: 0;
}

.device-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 0;
  border-bottom: 1px solid var(--border);
}

.device-item:last-child {
  border-bottom: none;
}

.device-alias {
  font-weight: 500;
}

.device-meta {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 4px;
}

.device-status {
  margin-left: 8px;
  font-weight: 500;
}

.device-status.discovered { color: var(--warning); }
.device-status.paired { color: var(--success); }
.device-status.revoked { color: var(--danger); }

.empty {
  text-align: center;
  color: var(--text-muted);
  padding: 40px 0;
}

.badge {
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 12px;
}

.badge--success {
  background: rgba(46, 204, 113, 0.15);
  color: var(--success);
}
</style>