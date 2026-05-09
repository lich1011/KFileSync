<script setup lang="ts">
import { useTransferStore } from '../../stores/transfers'
import type { TransferJob } from '../../types'

const store = useTransferStore()

function stateLabel(state: TransferJob['state']): string {
  const map: Record<string, string> = {
    pending: '等待中',
    active: '传输中',
    paused: '已暂停',
    verifying: '校验中',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消',
  }
  return map[state] ?? state
}

function stateClass(state: TransferJob['state']): string {
  return `state--${state}`
}

function isTerminal(state: TransferJob['state']): boolean {
  return state === 'completed' || state === 'failed' || state === 'cancelled'
}
</script>

<template>
  <div class="transfer-list">
    <div v-if="store.transfers.length === 0" class="empty">
      暂无传输任务
    </div>

    <div v-for="job in store.transfers" :key="job.jobId" class="transfer-item">
      <div class="transfer-info">
        <div class="transfer-peer">{{ job.peerAlias || job.peerId }}</div>
        <div class="transfer-meta">
          {{ job.files.length }} 个文件
          <span class="transfer-state" :class="stateClass(job.state)">
            {{ stateLabel(job.state) }}
          </span>
        </div>
      </div>

      <div v-if="!isTerminal(job.state)" class="transfer-actions">
        <button
          v-if="job.state === 'pending'"
          class="ghost"
          @click="store.acceptTransfer(job.jobId)"
        >接受</button>
        <button
          v-if="job.state === 'active'"
          class="ghost"
          @click="store.pauseTransfer(job.jobId)"
        >暂停</button>
        <button
          v-if="job.state === 'paused'"
          class="ghost"
          @click="store.resumeTransfer(job.jobId)"
        >恢复</button>
        <button
          class="ghost danger-text"
          @click="store.cancelTransfer(job.jobId)"
        >取消</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.transfer-list {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 20px;
}

.empty {
  text-align: center;
  color: var(--text-muted);
  padding: 40px 0;
}

.transfer-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 0;
  border-bottom: 1px solid var(--border);
}

.transfer-item:last-child {
  border-bottom: none;
}

.transfer-peer {
  font-weight: 500;
}

.transfer-meta {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 4px;
}

.transfer-state {
  margin-left: 8px;
  font-weight: 500;
}

.state--active { color: var(--accent); }
.state--completed { color: var(--success); }
.state--paused { color: var(--warning); }
.state--failed { color: var(--danger); }
.state--cancelled { color: var(--text-muted); }

.transfer-actions {
  display: flex;
  gap: 4px;
}

.danger-text {
  color: var(--danger) !important;
}
</style>