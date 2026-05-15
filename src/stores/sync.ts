import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { SyncStatus, SyncConflict } from '../types'
import { getSyncStatus, getConflicts, resolveConflict, triggerSync } from '../api/tauri'

export const useSyncStore = defineStore('sync', () => {
  const status = ref<SyncStatus | null>(null)
  const conflicts = ref<SyncConflict[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchStatus(shareId: string) {
    loading.value = true
    error.value = null
    try {
      status.value = await getSyncStatus(shareId)
    } catch (e: any) {
      error.value = e.toString()
    } finally {
      loading.value = false
    }
  }

  async function fetchConflicts(shareId: string) {
    try {
      conflicts.value = await getConflicts(shareId)
    } catch (e: any) {
      error.value = e.toString()
    }
  }

  async function dismissConflict(conflictId: string) {
    try {
      await resolveConflict(conflictId, 'delete')
      conflicts.value = conflicts.value.filter(c => c.conflictId !== conflictId)
    } catch (e: any) {
      error.value = e.toString()
    }
  }

  async function syncNow(shareId: string, peerId: string) {
    loading.value = true
    error.value = null
    try {
      const result = await triggerSync(shareId, peerId)
      await fetchStatus(shareId)
      await fetchConflicts(shareId)
      return result
    } catch (e: any) {
      error.value = e.toString()
      throw e
    } finally {
      loading.value = false
    }
  }

  return { 
    status, 
    conflicts, 
    loading, 
    error, 
    fetchStatus, 
    fetchConflicts, 
    dismissConflict, 
    syncNow 
  }
})