<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSyncStore } from '@/stores/sync'
import { getPairedDevices } from '@/api/tauri'
import type { PairedDevice } from '@/types'
import { useShareStore } from '@/stores/shares'
import { useNotificationStore } from '@/stores/notifications'
import { Card, CardHeader, CardTitle, CardContent, Button, Badge } from '@/components/ui'
import {
  RefreshCw,
  FolderSync,
  Laptop,
  AlertTriangle,
  FileCode,
  CheckCheck,
  RotateCcw,
  Layers,
} from 'lucide-vue-next'

const syncStore = useSyncStore()
const sharesStore = useShareStore()
const notifyStore = useNotificationStore()
const pairedDevices = ref<PairedDevice[]>([])
const selectedShareId = ref<string>('')
const selectedPeerId = ref<string>('')

onMounted(async () => {
  try {
    const [devices] = await Promise.all([
      getPairedDevices(),
      sharesStore.fetchShares(),
    ])
    pairedDevices.value = devices
  } catch (_) {}
})

async function onSelectShare(shareId: string) {
  selectedShareId.value = shareId
  await syncStore.fetchStatus(shareId)
  await syncStore.fetchConflicts(shareId)
}

async function onSync() {
  if (!selectedShareId.value || !selectedPeerId.value) return
  try {
    const result = await syncStore.syncNow(selectedShareId.value, selectedPeerId.value)
    notifyStore.add('success', result ?? '同步任务执行成功！')
  } catch (e: any) {
    notifyStore.add('error', `同步失败: ${e}`)
  }
}

async function onDismissConflict(conflictId: string) {
  await syncStore.dismissConflict(conflictId)
  notifyStore.add('info', '已忽略该冲突项')
}

async function onResolve(
  conflictId: string,
  resolution: 'keep_local' | 'keep_remote' | 'keep_both'
) {
  await syncStore.resolveConflictAs(conflictId, resolution)
  notifyStore.add('success', `冲突已成功解决为: ${resolution}`)
}
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-on-surface">数据同步</h1>
        <p class="text-sm text-on-surface-muted mt-1">触发目录版本比对与向量钟协同同步，并处理并发冲突</p>
      </div>

      <Button
        variant="filled"
        size="md"
        :disabled="!selectedShareId || !selectedPeerId || syncStore.loading"
        @click="onSync"
        class="gap-2 shadow-sm"
      >
        <RefreshCw class="w-4 h-4" :class="{ 'animate-spin': syncStore.loading }" />
        {{ syncStore.loading ? '正在同步数据...' : '立刻同步' }}
      </Button>
    </div>

    <!-- Sync Controls Card -->
    <Card variant="filled" class="bg-surface-container border border-outline-variant/20 shadow-sm">
      <CardHeader class="pb-3 border-b border-outline-variant/20">
        <CardTitle class="text-sm font-semibold tracking-wide text-on-surface-variant flex items-center gap-2">
          同步参数配置
        </CardTitle>
      </CardHeader>

      <CardContent class="p-5 space-y-4">
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <!-- Share Select -->
          <div class="space-y-2">
            <label class="text-xs font-semibold text-on-surface flex items-center gap-1.5">
              <FolderSync class="w-3.5 h-3.5 text-primary" />
              选择同步共享目录
            </label>
            <select
              v-model="selectedShareId"
              @change="onSelectShare(selectedShareId)"
              class="w-full h-11 px-4 rounded-2xl bg-surface-container-highest border border-outline-variant/50 text-sm text-on-surface focus:outline-none focus:border-primary appearance-none cursor-pointer"
            >
              <option value="" disabled selected>选择目录...</option>
              <option v-for="s in sharesStore.shares" :key="s.shareId" :value="s.shareId">
                {{ s.shareName }} ({{ s.localPath }})
              </option>
            </select>
          </div>

          <!-- Peer Device Select -->
          <div class="space-y-2">
            <label class="text-xs font-semibold text-on-surface flex items-center gap-1.5">
              <Laptop class="w-3.5 h-3.5 text-primary" />
              选择目标配对设备
            </label>
            <select
              v-model="selectedPeerId"
              class="w-full h-11 px-4 rounded-2xl bg-surface-container-highest border border-outline-variant/50 text-sm text-on-surface focus:outline-none focus:border-primary appearance-none cursor-pointer"
            >
              <option value="" disabled selected>选择设备...</option>
              <option v-for="d in pairedDevices" :key="d.id" :value="d.id">
                {{ d.alias }} ({{ d.address }}) · {{ d.online ? '🟢 在线' : '⚪ 离线' }}
              </option>
            </select>
          </div>
        </div>

        <!-- Status Pill Cards if selected -->
        <div v-if="syncStore.status" class="grid grid-cols-2 gap-3 pt-2">
          <div class="p-3.5 rounded-2xl bg-surface-container-high/60 border border-outline-variant/20 flex items-center gap-3">
            <div class="w-9 h-9 rounded-xl bg-primary-container/70 flex items-center justify-center text-primary">
              <Layers class="w-4 h-4" />
            </div>
            <div>
              <span class="text-[11px] text-on-surface-muted">目录文件总数</span>
              <p class="text-lg font-bold text-on-surface">{{ syncStore.status.totalFiles }} 个</p>
            </div>
          </div>

          <div class="p-3.5 rounded-2xl bg-surface-container-high/60 border border-outline-variant/20 flex items-center gap-3">
            <div
              class="w-9 h-9 rounded-xl flex items-center justify-center"
              :class="syncStore.status.conflicts > 0 ? 'bg-error-container text-error' : 'bg-success-container text-success'"
            >
              <AlertTriangle class="w-4 h-4" />
            </div>
            <div>
              <span class="text-[11px] text-on-surface-muted">检测到并发冲突</span>
              <p class="text-lg font-bold text-on-surface">{{ syncStore.status.conflicts }} 项</p>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Conflicts Resolution Panel -->
    <Card
      v-if="syncStore.conflicts.length > 0"
      variant="filled"
      class="bg-surface-container border border-error/30 shadow-sm"
    >
      <CardHeader class="pb-3 border-b border-outline-variant/20 flex flex-row items-center justify-between">
        <CardTitle class="text-sm font-semibold tracking-wide text-error flex items-center gap-2">
          <AlertTriangle class="w-4 h-4" />
          待决策的文件版本冲突
          <Badge variant="error" size="sm">{{ syncStore.conflicts.length }} 个待解决</Badge>
        </CardTitle>
      </CardHeader>

      <CardContent class="p-4 space-y-3">
        <div
          v-for="c in syncStore.conflicts"
          :key="c.conflictId"
          class="p-4 rounded-2xl bg-surface-container-high/70 border border-outline-variant/20 space-y-3"
        >
          <div class="flex items-center justify-between gap-3">
            <div class="flex items-center gap-2 min-w-0">
              <FileCode class="w-4 h-4 text-primary flex-shrink-0" />
              <span class="font-mono text-xs font-semibold text-on-surface truncate">{{ c.filePath }}</span>
            </div>
            <Badge variant="surface" size="sm" class="flex-shrink-0">
              策略: {{ c.resolution }}
            </Badge>
          </div>

          <!-- Conflict Action Buttons -->
          <div class="flex flex-wrap items-center gap-2 pt-1 border-t border-outline-variant/20">
            <Button
              variant="tonal"
              size="sm"
              @click="onResolve(c.conflictId, 'keep_local')"
              class="gap-1 text-xs"
            >
              <CheckCheck class="w-3.5 h-3.5 text-primary" />
              保留本地修改
            </Button>
            <Button
              variant="tonal"
              size="sm"
              @click="onResolve(c.conflictId, 'keep_remote')"
              class="gap-1 text-xs"
            >
              <RotateCcw class="w-3.5 h-3.5 text-secondary" />
              采用远程版本
            </Button>
            <Button
              variant="outlined"
              size="sm"
              @click="onResolve(c.conflictId, 'keep_both')"
              class="gap-1 text-xs"
            >
              双向均保留 (.sync-conflict)
            </Button>
            <Button
              variant="ghost"
              size="sm"
              @click="onDismissConflict(c.conflictId)"
              class="text-on-surface-muted hover:text-error text-xs"
            >
              忽略
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Error Alert Banner -->
    <div
      v-if="syncStore.error"
      class="p-4 rounded-2xl bg-error-container/80 text-error-foreground text-xs flex items-center gap-2.5 border border-error/30"
    >
      <AlertTriangle class="w-4 h-4 flex-shrink-0" />
      <span>{{ syncStore.error }}</span>
    </div>
  </div>
</template>