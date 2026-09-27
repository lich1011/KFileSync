<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useDeviceStore } from '@/stores/devices'
import { useShareStore } from '@/stores/shares'
import { useNotificationStore } from '@/stores/notifications'
import { Dialog, Button } from '@/components/ui'
import { UserPlus, Laptop, Shield } from 'lucide-vue-next'

const props = defineProps<{ shareId: string }>()
const emit = defineEmits<{ close: [] }>()

const deviceStore = useDeviceStore()
const shareStore = useShareStore()
const notify = useNotificationStore()

const selectedDeviceId = ref('')
const permission = ref('read_write')
const inviting = ref(false)

onMounted(() => {
  if (deviceStore.devices.length === 0) {
    deviceStore.fetchDevices()
  }
})

const pairedDevices = computed(() => deviceStore.devices.filter((d) => d.status === 'Paired'))

async function invite() {
  if (!selectedDeviceId.value) {
    notify.add('warning', '请选择要邀请的设备')
    return
  }

  inviting.value = true
  try {
    await shareStore.inviteMember(props.shareId, selectedDeviceId.value, permission.value)
    notify.add('success', '邀请成员已发送并加入')
    emit('close')
  } catch (e: any) {
    notify.add('error', `邀请失败: ${e}`)
  } finally {
    inviting.value = false
  }
}
</script>

<template>
  <Dialog :open="true" @close="emit('close')">
    <div class="space-y-5">
      <!-- Header -->
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-2xl bg-primary-container flex items-center justify-center text-primary-on-container">
          <UserPlus class="w-5 h-5" />
        </div>
        <div>
          <h3 class="text-lg font-bold text-on-surface">邀请设备加入协同</h3>
          <p class="text-xs text-on-surface-muted mt-0.5">授权已配对设备访问或双向同步该目录</p>
        </div>
      </div>

      <!-- Select Device -->
      <div class="space-y-2">
        <label class="text-xs font-semibold text-on-surface flex items-center gap-1.5">
          <Laptop class="w-3.5 h-3.5 text-primary" />
          选择已配对设备
        </label>
        <select
          v-model="selectedDeviceId"
          class="w-full h-11 px-4 rounded-2xl bg-surface-container-highest border border-outline-variant/50 text-sm text-on-surface focus:outline-none focus:border-primary appearance-none cursor-pointer"
        >
          <option value="" disabled selected>选择设备</option>
          <option v-for="d in pairedDevices" :key="d.id" :value="d.id">
            {{ d.alias }} ({{ d.address }})
          </option>
        </select>
        <p v-if="pairedDevices.length === 0" class="text-[11px] text-yellow-500">
          ⚠️ 暂无已配对设备，请先完成设备配对
        </p>
      </div>

      <!-- Select Permission -->
      <div class="space-y-2">
        <label class="text-xs font-semibold text-on-surface flex items-center gap-1.5">
          <Shield class="w-3.5 h-3.5 text-primary" />
          赋予权限范围
        </label>
        <select
          v-model="permission"
          class="w-full h-11 px-4 rounded-2xl bg-surface-container-highest border border-outline-variant/50 text-sm text-on-surface focus:outline-none focus:border-primary appearance-none cursor-pointer"
        >
          <option value="read_write">读写权限 (支持上传与拉取修改)</option>
          <option value="read_only">只读权限 (仅允许从本机下载)</option>
          <option value="send_only">仅发送权限</option>
          <option value="receive_only">仅接收权限</option>
        </select>
      </div>

      <!-- Actions -->
      <div class="flex items-center justify-end gap-2 pt-2">
        <Button variant="outlined" size="md" @click="emit('close')">
          取消
        </Button>
        <Button
          variant="filled"
          size="md"
          :disabled="inviting || !selectedDeviceId"
          @click="invite"
          class="gap-1.5"
        >
          {{ inviting ? '正在加入...' : '确认邀请' }}
        </Button>
      </div>
    </div>
  </Dialog>
</template>