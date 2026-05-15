<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSyncStore } from '../stores/sync'
import { getPairedDevices } from '../api/tauri'
import type { PairedDevice, ShareInfo } from '../types'
import { useShareStore } from '../stores/shares'
import { useNotificationStore } from '../stores/notifications'

const syncStore = useSyncStore()
const sharesStore = useShareStore()
const notifyStore = useNotificationStore()
const pairedDevices = ref<PairedDevice[]>([])
const selectedShareId = ref<string>('')
const selectedPeerId = ref<string>('')

onMounted(async () => {
  try {
    pairedDevices.value = await getPairedDevices()
  } catch (_) {}
  // shares are populated as user creates them; no fetch needed
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
    notifyStore.add('success', result ?? 'Sync complete')
  } catch (e: any) {
    notifyStore.add('error', e.toString())
  }
}

async function onDismissConflict(conflictId: string) {
  await syncStore.dismissConflict(conflictId)
}
</script>

<template>
  <div class="sync-page">
    <h2>Sync</h2>

    <div class="sync-controls">
      <div class="form-group">
        <label>Share</label>
        <select v-model="selectedShareId" @change="onSelectShare(selectedShareId)">
          <option value="" disabled>Select a share...</option>
          <option v-for="s in sharesStore.shares" :key="s.shareId" :value="s.shareId">
            {{ s.shareName }}
          </option>
        </select>
      </div>

      <div class="form-group">
        <label>Peer Device</label>
        <select v-model="selectedPeerId">
          <option value="" disabled>Select a peer...</option>
          <option v-for="d in pairedDevices" :key="d.id" :value="d.id">
            {{ d.alias }} ({{ d.address }}) {{ d.online ? '●' : '○' }}
          </option>
        </select>
      </div>

      <button
        class="btn btn--primary"
        :disabled="!selectedShareId || !selectedPeerId || syncStore.loading"
        @click="onSync"
      >
        {{ syncStore.loading ? 'Syncing...' : 'Sync Now' }}
      </button>
    </div>

    <div v-if="syncStore.status" class="sync-status">
      <h3>Status</h3>
      <p>Total files: {{ syncStore.status.totalFiles }}</p>
      <p>Conflicts: {{ syncStore.status.conflicts }}</p>
    </div>

    <div v-if="syncStore.conflicts.length > 0" class="conflict-list">
      <h3>Conflicts</h3>
      <div v-for="c in syncStore.conflicts" :key="c.conflictId" class="conflict-item">
        <span class="conflict-path">{{ c.filePath }}</span>
        <span class="conflict-resolution">{{ c.resolution }}</span>
        <button class="btn btn--small" @click="onDismissConflict(c.conflictId)">Dismiss</button>
      </div>
    </div>

    <div v-if="syncStore.error" class="error">{{ syncStore.error }}</div>
  </div>
</template>

<style scoped>
.sync-page {
  padding: 24px;
  max-width: 700px;
}

.sync-controls {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-bottom: 24px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.form-group label {
  font-size: 13px;
  color: var(--text-muted);
}

.form-group select {
  padding: 8px 12px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg-card);
  color: var(--text);
}

.btn {
  padding: 8px 16px;
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
}

.btn--primary {
  background: var(--accent);
  color: #fff;
}

.btn--primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn--small {
  padding: 4px 8px;
  font-size: 12px;
  background: var(--bg-hover);
  color: var(--text);
}

.sync-status {
  padding: 16px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  margin-bottom: 16px;
}

.sync-status h3 {
  margin-bottom: 8px;
}

.conflict-list {
  margin-top: 16px;
}

.conflict-list h3 {
  margin-bottom: 8px;
}

.conflict-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 6px;
  margin-bottom: 8px;
}

.conflict-path {
  flex: 1;
  font-family: monospace;
  font-size: 13px;
}

.conflict-resolution {
  font-size: 12px;
  color: var(--text-muted);
}

.error {
  color: #e74c3c;
  margin-top: 12px;
}
</style>