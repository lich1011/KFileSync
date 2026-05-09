<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useDeviceStore } from '../../stores/devices'
import { useTransferStore } from '../../stores/transfers'
import { useNotificationStore } from '../../stores/notifications'

const emit = defineEmits<{ close: [] }>()

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const notify = useNotificationStore()

const selectedDeviceId = ref('')
const selectedFiles = ref<string[]>([])

onMounted(() => {
  if (deviceStore.devices.length === 0) {
    deviceStore.fetchDevices()
  }
})

const pairedDevices = () => deviceStore.devices.filter(d => d.status === 'Paired')

async function pickFiles() {
  const result = await open({ multiple: true, directory: false })
  if (result) {
    selectedFiles.value = Array.isArray(result) ? result : [result]
  }
}

async function send() {
  if (!selectedDeviceId.value) {
    notify.add('warning', '请选择目标设备')
    return
  }
  
  if (selectedFiles.value.length === 0) {
    notify.add('warning', '请选择文件')
    return
  }

  const device = deviceStore.devices.find(d => d.id === selectedDeviceId.value)
  const files = selectedFiles.value.map(path => ({
    filePath: path,
    fileSize: 0,
    sha256: '',
  }))

  await transferStore.sendFiles(selectedDeviceId.value, device?.alias ?? selectedDeviceId.value, files)
  emit('close')
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <h3>发送文件</h3>

      <label class="field">
        <span>目标设备</span>
        <select v-model="selectedDeviceId">
          <option value="" disabled>选择已配对设备</option>
          <option v-for="d in pairedDevices()" :key="d.id" :value="d.id">
            {{ d.alias }} ({{ d.address }})
          </option>
        </select>
      </label>

      <div class="field">
        <span>文件</span>
        <button class="primary" @click="pickFiles" style="width: auto;">选择文件</button>
        <div v-if="selectedFiles.length" class="file-list">
          <div v-for="(f, i) in selectedFiles" :key="i" class="file-item">{{ f }}</div>
        </div>
      </div>

      <div class="actions">
        <button class="ghost" @click="emit('close')">取消</button>
        <button class="primary" @click="send">发送</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 24px;
  width: 420px;
  max-width: 90vw;
}

h3 {
  font-size: 18px;
  margin-bottom: 16px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 14px;
  font-size: 13px;
  color: var(--text-muted);
}

.file-list {
  margin-top: 8px;
  max-height: 120px;
  overflow-y: auto;
}

.file-item {
  font-size: 12px;
  color: var(--text);
  padding: 4px 0;
  word-break: break-all;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
</style>