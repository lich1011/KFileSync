<script setup lang="ts">
import { ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { useShareStore } from '../../stores/shares'
import { useNotificationStore } from '../../stores/notifications'
import type { SyncMode } from '../../types'

const emit = defineEmits<{ close: [] }>()

const shareStore = useShareStore()
const notify = useNotificationStore()

const shareName = ref('')
const localPath = ref('')
const syncMode = ref<SyncMode>('two_way')

async function pickFolder() {
  const result = await open({ directory: true, multiple: false })
  if (result) {
    localPath.value = result as string
  }
}

async function create() {
  if (!shareName.value.trim()) {
    notify.add('warning', '请输入共享名称')
    return
  }
  if (!localPath.value) {
    notify.add('warning', '请选择共享目录')
    return
  }
  
  await shareStore.createShare(shareName.value.trim(), localPath.value, syncMode.value)
  emit('close')
}
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <h3>创建共享目录</h3>

      <label class="field">
        <span>共享名称</span>
        <input v-model="shareName" placeholder="例如：项目文档" />
      </label>

      <div class="field">
        <span>本地目录</span>
        <div class="path-row">
          <input v-model="localPath" placeholder="选择或输入路径" />
          <button class="primary" @click="pickFolder" style="flex-shrink: 0;">浏览</button>
        </div>
      </div>

      <label class="field">
        <span>同步模式</span>
        <select v-model="syncMode">
          <option value="two_way">双向同步</option>
          <option value="send_only">仅发送</option>
          <option value="receive_only">仅接收</option>
        </select>
      </label>

      <div class="actions">
        <button class="ghost" @click="emit('close')">取消</button>
        <button class="primary" @click="create">创建</button>
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

.path-row {
  display: flex;
  gap: 8px;
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
</style>