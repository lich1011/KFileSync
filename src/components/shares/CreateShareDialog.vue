<script setup lang="ts">
import { ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useShareStore } from '@/stores/shares'
import { useNotificationStore } from '@/stores/notifications'
import type { SyncMode } from '@/types'
import { Dialog, Button, Input } from '@/components/ui'
import { FolderPlus, FolderOpen, ArrowLeftRight, Check } from 'lucide-vue-next'

const emit = defineEmits<{ close: [] }>()

const shareStore = useShareStore()
const notify = useNotificationStore()

const shareName = ref('')
const localPath = ref('')
const syncMode = ref<SyncMode>('two_way')
const creating = ref(false)

async function pickFolder() {
  const result = await open({ directory: true, multiple: false })
  if (result) {
    localPath.value = result as string
    if (!shareName.value.trim()) {
      // 提取文件夹名作为默认共享名
      const parts = (result as string).replace(/\\/g, '/').split('/')
      shareName.value = parts[parts.length - 1] || '新共享'
    }
  }
}

async function create() {
  if (!shareName.value.trim()) {
    notify.add('warning', '请输入共享目录名称')
    return
  }
  if (!localPath.value) {
    notify.add('warning', '请选取本地目录路径')
    return
  }

  creating.value = true
  try {
    await shareStore.createShare(shareName.value.trim(), localPath.value, syncMode.value)
    notify.add('success', `共享目录「${shareName.value}」创建成功！`)
    emit('close')
  } catch (e: any) {
    notify.add('error', `创建失败: ${e}`)
  } finally {
    creating.value = false
  }
}
</script>

<template>
  <Dialog :open="true" @close="emit('close')">
    <div class="space-y-5">
      <!-- Header -->
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-2xl bg-primary-container flex items-center justify-center text-primary-on-container">
          <FolderPlus class="w-5 h-5" />
        </div>
        <div>
          <h3 class="text-lg font-bold text-on-surface">创建共享目录</h3>
          <p class="text-xs text-on-surface-muted mt-0.5">选择本地文件夹并配置局域网同步规则</p>
        </div>
      </div>

      <!-- Local Folder Picker -->
      <div class="space-y-2">
        <label class="text-xs font-semibold text-on-surface">选择本地物理目录</label>
        <div class="flex gap-2">
          <Input
            v-model="localPath"
            placeholder="点击右侧浏览选择文件夹"
            class="flex-1"
          />
          <Button
            variant="tonal"
            size="md"
            @click="pickFolder"
            class="gap-1.5 flex-shrink-0"
          >
            <FolderOpen class="w-4 h-4" />
            浏览
          </Button>
        </div>
      </div>

      <!-- Share Name -->
      <div class="space-y-2">
        <label class="text-xs font-semibold text-on-surface">共享别名</label>
        <Input
          v-model="shareName"
          placeholder="例如：团队设计稿、项目代码"
        />
      </div>

      <!-- Sync Mode -->
      <div class="space-y-2">
        <label class="text-xs font-semibold text-on-surface">同步模式</label>
        <div class="grid grid-cols-3 gap-2">
          <button
            type="button"
            @click="syncMode = 'two_way'"
            :class="[
              'p-3 rounded-2xl border text-center transition-all flex flex-col items-center gap-1.5',
              syncMode === 'two_way'
                ? 'bg-primary-container/80 border-primary text-primary-on-container font-semibold shadow-sm'
                : 'bg-surface-container border-outline-variant/30 text-on-surface-variant hover:bg-surface-container-highest'
            ]"
          >
            <ArrowLeftRight class="w-4 h-4" />
            <span class="text-xs">双向同步</span>
          </button>
          <button
            type="button"
            @click="syncMode = 'send_only'"
            :class="[
              'p-3 rounded-2xl border text-center transition-all flex flex-col items-center gap-1.5',
              syncMode === 'send_only'
                ? 'bg-primary-container/80 border-primary text-primary-on-container font-semibold shadow-sm'
                : 'bg-surface-container border-outline-variant/30 text-on-surface-variant hover:bg-surface-container-highest'
            ]"
          >
            <span class="text-sm">⬆️</span>
            <span class="text-xs">仅发送</span>
          </button>
          <button
            type="button"
            @click="syncMode = 'receive_only'"
            :class="[
              'p-3 rounded-2xl border text-center transition-all flex flex-col items-center gap-1.5',
              syncMode === 'receive_only'
                ? 'bg-primary-container/80 border-primary text-primary-on-container font-semibold shadow-sm'
                : 'bg-surface-container border-outline-variant/30 text-on-surface-variant hover:bg-surface-container-highest'
            ]"
          >
            <span class="text-sm">⬇️</span>
            <span class="text-xs">仅接收</span>
          </button>
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
          :disabled="creating || !shareName.trim() || !localPath"
          @click="create"
          class="gap-1.5"
        >
          <Check class="w-4 h-4" />
          {{ creating ? '创建中...' : '确认创建' }}
        </Button>
      </div>
    </div>
  </Dialog>
</template>