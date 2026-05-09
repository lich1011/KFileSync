import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { TransferJob, FileRequestDto } from '../types'
import * as api from '../api/tauri'
import { useNotificationStore } from './notifications'

export const useTransferStore = defineStore('transfers', () => {
  const transfers = ref<TransferJob[]>([])
  const showSendDialog = ref(false)

  async function sendFiles(peerId: string, peerAlias: string, files: FileRequestDto[]) {
    try {
      const jobId = await api.sendFiles(peerId, files)
      transfers.value.push({
        jobId,
        peerId,
        peerAlias,
        files,
        state: 'pending',
        createdAt: Date.now(),
      })
      useNotificationStore().add('success', `已发起传输到 ${peerAlias}`)
    } catch (e) {
      useNotificationStore().add('error', `发送文件失败: ${e}`)
    }
  }

  async function acceptTransfer(jobId: string) {
    try {
      await api.acceptTransfer(jobId)
      updateState(jobId, 'active')
      useNotificationStore().add('info', '已接受传输')
    } catch (e) {
      useNotificationStore().add('error', `接受传输失败: ${e}`)
    }
  }

  async function pauseTransfer(jobId: string) {
    try {
      await api.pauseTransfer(jobId)
      updateState(jobId, 'paused')
    } catch (e) {
      useNotificationStore().add('error', `暂停失败: ${e}`)
    }
  }

  async function resumeTransfer(jobId: string) {
    try {
      await api.resumeTransfer(jobId)
      updateState(jobId, 'active')
    } catch (e) {
      useNotificationStore().add('error', `恢复失败: ${e}`)
    }
  }

  async function cancelTransfer(jobId: string) {
    try {
      await api.cancelTransfer(jobId)
      updateState(jobId, 'cancelled')
    } catch (e) {
      useNotificationStore().add('error', `取消失败: ${e}`)
    }
  }

  function updateState(jobId: string, state: TransferJob['state']) {
    const job = transfers.value.find(t => t.jobId === jobId)
    if (job) job.state = state
  }

  return { transfers, showSendDialog, sendFiles, acceptTransfer, pauseTransfer, resumeTransfer, cancelTransfer }
})