<script setup lang="ts">
import { useTransferStore } from '@/stores/transfers'
import type { TransferJob } from '@/types'
import { Card, CardHeader, CardTitle, CardContent, Badge, Button } from '@/components/ui'
import {
  ArrowDownUp,
  Files,
  Play,
  Pause,
  X,
  Check,
  CheckCircle2,
  AlertCircle,
  Clock,
} from 'lucide-vue-next'

const store = useTransferStore()

function stateVariant(state: TransferJob['state']): 'primary' | 'success' | 'error' | 'surface' | 'secondary' {
  switch (state) {
    case 'active':
      return 'primary'
    case 'completed':
      return 'success'
    case 'failed':
      return 'error'
    case 'paused':
      return 'secondary'
    case 'pending':
    default:
      return 'surface'
  }
}

function stateLabel(state: TransferJob['state']): string {
  const map: Record<string, string> = {
    pending: '等待就绪',
    active: '正在极速传输',
    paused: '已暂停',
    verifying: '校验哈希中',
    completed: '传输完成',
    failed: '传输失败',
    cancelled: '已取消',
  }
  return map[state] ?? state
}

function isTerminal(state: TransferJob['state']): boolean {
  return state === 'completed' || state === 'failed' || state === 'cancelled'
}
</script>

<template>
  <Card variant="filled" class="bg-surface-container border border-outline-variant/20 shadow-sm">
    <CardHeader class="pb-3 border-b border-outline-variant/20 flex flex-row items-center justify-between">
      <CardTitle class="text-sm font-semibold tracking-wide text-on-surface-variant flex items-center gap-2">
        活跃与历史传输队列
        <Badge variant="surface" size="sm">{{ store.transfers.length }} 项</Badge>
      </CardTitle>
    </CardHeader>

    <CardContent class="p-4">
      <!-- Empty State -->
      <div v-if="store.transfers.length === 0" class="py-16 text-center">
        <div class="mx-auto w-14 h-14 rounded-full bg-surface-container-highest flex items-center justify-center text-on-surface-muted mb-3">
          <ArrowDownUp class="w-7 h-7 opacity-60" />
        </div>
        <p class="text-sm font-medium text-on-surface">当前暂无传输任务</p>
        <p class="text-xs text-on-surface-muted mt-1">点击右上角「发送文件」选择目标节点即可直传</p>
      </div>

      <!-- Transfer Job Items -->
      <div v-else class="space-y-3">
        <div
          v-for="job in store.transfers"
          :key="job.jobId"
          class="flex flex-col sm:flex-row sm:items-center justify-between p-4 rounded-2xl bg-surface-container-high/60 hover:bg-surface-container-high transition-colors border border-outline-variant/20 gap-3"
        >
          <!-- Left info -->
          <div class="flex items-center gap-4 min-w-0">
            <div class="w-11 h-11 rounded-2xl bg-secondary-container flex items-center justify-center text-secondary-on-container flex-shrink-0">
              <Files class="w-5 h-5" />
            </div>
            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <h4 class="text-sm font-semibold text-on-surface truncate">
                  {{ job.peerAlias || job.peerId }}
                </h4>
                <Badge :variant="stateVariant(job.state)" size="sm">
                  <span v-if="job.state === 'active'" class="w-1.5 h-1.5 rounded-full bg-primary animate-ping"></span>
                  <CheckCircle2 v-else-if="job.state === 'completed'" class="w-3 h-3 text-success" />
                  <AlertCircle v-else-if="job.state === 'failed'" class="w-3 h-3 text-error" />
                  <Clock v-else class="w-3 h-3 text-on-surface-muted" />
                  {{ stateLabel(job.state) }}
                </Badge>
              </div>

              <div class="flex items-center gap-2 text-xs text-on-surface-muted mt-1">
                <span>包含 {{ job.files.length }} 个文件</span>
                <span class="text-outline-variant">•</span>
                <span class="font-mono text-[11px] truncate max-w-[160px] opacity-70">任务: {{ job.jobId }}</span>
              </div>
            </div>
          </div>

          <!-- Right actions -->
          <div v-if="!isTerminal(job.state)" class="flex items-center gap-2 self-end sm:self-center">
            <Button
              v-if="job.state === 'pending'"
              variant="tonal"
              size="sm"
              @click="store.acceptTransfer(job.jobId)"
              class="gap-1.5"
            >
              <Check class="w-3.5 h-3.5" />
              接受接收
            </Button>
            <Button
              v-if="job.state === 'active'"
              variant="outlined"
              size="sm"
              @click="store.pauseTransfer(job.jobId)"
              class="gap-1.5"
            >
              <Pause class="w-3.5 h-3.5" />
              暂停
            </Button>
            <Button
              v-if="job.state === 'paused'"
              variant="tonal"
              size="sm"
              @click="store.resumeTransfer(job.jobId)"
              class="gap-1.5"
            >
              <Play class="w-3.5 h-3.5" />
              恢复
            </Button>
            <Button
              variant="ghost"
              size="sm"
              @click="store.cancelTransfer(job.jobId)"
              class="text-error hover:bg-error-container/20 gap-1"
            >
              <X class="w-3.5 h-3.5" />
              取消
            </Button>
          </div>
        </div>
      </div>
    </CardContent>
  </Card>
</template>