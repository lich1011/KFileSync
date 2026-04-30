<script setup lang="ts">
import { ref } from 'vue'

interface Device {
  id: string;
  alias: string;
  address: string;
  status: 'Unknown' | 'Discovered' | 'Paired' | 'Revoked';
}

// 模拟的发现数据 (真实情况需对接 Tauri invoke('get_discovered_devices'))
const devices = ref<Device[]>([
  { id: 'dev_mock_1', alias: 'Luokai MacBook (Self)', address: '192.168.1.10', status: 'Paired' },
  { id: 'dev_mock_2', alias: 'Windows Design Team', address: '192.168.1.12', status: 'Discovered' }
])

const requestPairing = (id: string) => {
  console.log('Requesting pairing for ', id)
  const device = devices.value.find(d => d.id === id)
  if(device) {
    // 乐观更新 UI
    device.status = 'Paired'
    alert(`配对码: ${Math.floor(100000 + Math.random() * 900000)} - 等待对方确认...`)
  }
}
</script>

<template>
  <div class="device-list">
    <h2>附近设备 (Discovery)</h2>
    <ul>
      <li v-for="device in devices" :key="device.id" class="device-item">
        <div class="info">
          <div><strong>{{ device.alias }}</strong></div>
          <div class="meta">{{ device.address }} 
            <span :class="device.status.toLowerCase()">[{{ device.status }}]</span>
          </div>
        </div>
        <button 
          class="pair-btn"
          v-if="device.status === 'Discovered'" 
          @click="requestPairing(device.id)"
        >
          信任配对
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.device-list {
  padding: 1.5rem;
  background-color: #1e1e2f;
  border-radius: 12px;
  max-width: 450px;
  margin: 2rem auto;
  box-shadow: 0 4px 6px rgba(0,0,0,0.3);
  color: #fff;
  text-align: left;
}
h2 {
  margin-top: 0;
  font-size: 1.25rem;
  border-bottom: 1px solid #333;
  padding-bottom: 10px;
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
  padding: 1rem 0;
  border-bottom: 1px dashed #444;
}
.device-item:last-child {
  border-bottom: none;
}
.meta {
  font-size: 0.85rem;
  color: #aaa;
  margin-top: 4px;
}
.discovered { color: #f39c12; }
.paired { color: #2ecc71; }
.pair-btn {
  background: #3498db;
  color: white;
  border: none;
  padding: 6px 12px;
  border-radius: 4px;
  cursor: pointer;
}
.pair-btn:hover { background: #2980b9; }
</style>
