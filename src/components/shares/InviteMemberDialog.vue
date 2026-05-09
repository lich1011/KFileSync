<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useDeviceStore } from '../../stores/devices'
import { useShareStore } from '../../stores/shares'
import { useNotificationStore } from '../../stores/notifications'

const props = defineProps<{ shareId: string }>()
const emit = defineEmits<{ close: [] }>()

const deviceStore = useDeviceStore()
const shareStore = useShareStore()
const notify = useNotificationStore()

const selectedDeviceId = ref('')
const permission = ref('read_write')

onMounted(() => {
  if (deviceStore.devices.length === 0) {
    deviceStore.fetchDevices()
  }
})

const pairedDevices = () => deviceStore.devices.filter(d => d.status === 'Paired')

async function invite() {
  if (!selectedDeviceId.value) {
    notify.add('warning', '请选择设备')
    return
  }
  await shareStore.inviteMember(props.shareId, selectedDeviceId.value, permission.value)
  emit('close')
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <h3>邀请成员</h3>

      <label class="field">
        <span>选择设备</span>
        <select v-model="selectedDeviceId">
          <option value="" disabled>选择已配对设备</option>
          <option v-for="d in pairedDevices()" :key="d.id" :value="d.id">
            {{ d.alias }} ({{ d.address }})
          </option>
        </select>
      </label>

      <label class="field">
        <span>权限</span>
        <select v-model="permission">
          <option value="read_write">读写</option>
          <option value="read_only">只读</option>
          <option value="send_only">仅发送</option>
          <option value="receive_only">仅接收</option>
        </select>
      </label>

      <div class="actions">
        <button class="ghost" @click="emit('close')">取消</button>
        <button class="primary" @click="invite">邀请</button>
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
  width: 380px;
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

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
</style>