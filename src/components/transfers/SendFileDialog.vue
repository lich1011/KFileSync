<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useDeviceStore } from '@/stores/devices'
import { useTransferStore } from '@/stores/transfers'
import { useNotificationStore } from '@/stores/notifications'
import { Dialog, Button } from '@/components/ui'
import { UploadCloud, FolderOpen, FileText, SendHorizontal, X } from 'lucide-vue-next'

const emit = defineEmits<{ close: [] }>()

const deviceStore = useDeviceStore()
const transferStore = useTransferStore()
const notify = useNotificationStore()

const selectedDeviceId = ref('')
const selectedFiles = ref<string[]>([])
const sending = ref(false)

onMounted(() => {
  if (deviceStore.devices.length === 0) {
    deviceStore.fetchDevices()
  }
})

const pairedDevices = computed(() => deviceStore.devices.filter((d) => d.status === 'Paired'))

async function pickFiles() {
  const result = await open({ multiple: true, directory: false })
  if (result) {
    selectedFiles.value = Array.isArray(result) ? result : [result]
  }
}

function removeFile(index: number) {
  selectedFiles.value.splice(index, 1)
}

async function send() {
  if (!selectedDeviceId.value) {
    notify.add('warning', '请选择已配对的目标设备')
    return
  }

  if (selectedFiles.value.length === 0) {
    notify.add('warning', '请先选取需要传输的文件')
    return
  }

  sending.value = true
  try {
    const device = deviceStore.devices.find((d) => d.id === selectedDeviceId.value)
    const files = selectedFiles.value.map((path) => ({
      filePath: path,
      fileSize: 0,
      sha256: '',
    }))

    await transferStore.sendFiles(
      selectedDeviceId.value,
      device?.alias ?? selectedDeviceId.value,
      files
    )
    notify.add('success', '传输请求已成功发送！')
    emit('close')
  } catch (err: any) {
    notify.add('error', `发送失败: ${err}`)
  } finally {
    sending.value = false
  }
}
</script>

<template>
  <Dialog :open="true" @close="emit('close')">
    <div class="space-y-5">
      <!-- Header -->
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-2xl bg-primary-container flex items-center justify-center text-primary-on-container">
          <UploadCloud class="w-5 h-5" />
        </div>
        <div>
          <h3 class="text-lg font-bold text-on-surface">快速发送文件</h3>
          <p class="text-xs text-on-surface-muted mt-0.5">选取待传输的本地文件直传至局域网配对设备</p>
        </div>
      </div>

      <!-- Target Device Select -->
      <div class="space-y-2">
        <label class="text-xs font-semibold text-on-surface">选择目标配对设备</label>
        <div class="relative">
          <select
            v-model="selectedDeviceId"
            class="w-full h-11 px-4 rounded-2xl bg-surface-container-highest border border-outline-variant/50 text-sm text-on-surface focus:outline-none focus:border-primary appearance-none cursor-pointer"
          >
            <option value="" disabled selected>请选择在线已配对设备</option>
            <option v-for="d in pairedDevices" :key="d.id" :value="d.id">
              {{ d.alias }} ({{ d.address }})
            </option>
          </select>
          <div v-if="pairedDevices.length === 0" class="text-[11px] text-yellow-500 mt-1.5">
            ⚠️ 暂无已配对设备，请先至「设备发现」页面与目标设备配对
          </div>
        </div>
      </div>

      <!-- Pick Files Section -->
      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <label class="text-xs font-semibold text-on-surface">待发送文件</label>
          <Button
            variant="tonal"
            size="sm"
            @click="pickFiles"
            class="gap-1.5"
          >
            <FolderOpen class="w-3.5 h-3.5" />
            浏览选择文件
          </Button>
        </div>

        <div
          v-if="selectedFiles.length === 0"
          @click="pickFiles"
          class="border-2 border-dashed border-outline-variant/50 rounded-2xl p-6 text-center cursor-pointer hover:border-primary/60 transition-colors bg-surface-container-lowest/50"
        >
          <FolderOpen class="w-8 h-8 text-on-surface-muted mx-auto mb-2" />
          <p class="text-xs font-medium text-on-surface">点击浏览或拖拽文件到此处</p>
          <p class="text-[11px] text-on-surface-muted mt-0.5">支持同时选择任意格式多文件</p>
        </div>

        <div
          v-else
          class="max-h-36 overflow-y-auto space-y-1.5 p-2 rounded-2xl bg-surface-container-lowest border border-outline-variant/30"
        >
          <div
            v-for="(f, idx) in selectedFiles"
            :key="idx"
            class="flex items-center justify-between gap-2 px-3 py-1.5 rounded-xl bg-surface-container-high/60 text-xs text-on-surface"
          >
            <div class="flex items-center gap-2 truncate">
              <FileText class="w-4 h-4 text-primary flex-shrink-0" />
              <span class="truncate font-mono text-[11px]">{{ f }}</span>
            </div>
            <button
              @click="removeFile(idx)"
              class="text-on-surface-muted hover:text-error p-1 rounded-full"
            >
              <X class="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      </div>

      <!-- Actions -->
      <div class="flex items-center justify-end gap-2 pt-2">
        <Button variant="outlined" size="md" @click="emit('close')">
          取消
        </Button>
        <Button
          variant="filled"
          size="md"
          :disabled="sending || !selectedDeviceId || selectedFiles.length === 0"
          @click="send"
          class="gap-1.5"
        >
          <SendHorizontal class="w-4 h-4" />
          {{ sending ? '正在打包发送...' : '立刻发送' }}
        </Button>
      </div>
    </div>
  </Dialog>
</template>