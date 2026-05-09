import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ShareInfo, SyncMode } from '../types'
import * as api from '../api/tauri'
import { useNotificationStore } from './notifications'

export const useShareStore = defineStore('shares', () => {
  const shares = ref<ShareInfo[]>([])
  const showCreateDialog = ref(false)
  const invitingShareId = ref<string | null>(null)

  async function createShare(name: string, localPath: string, syncMode: SyncMode) {
    try {
      const shareId = await api.createShare(name, localPath, syncMode)
      shares.value.push({
        shareId,
        shareName: name,
        localPath,
        syncMode,
        status: 'active',
        members: [],
      })
      useNotificationStore().add('success', `共享目录 "${name}" 已创建`)
    } catch (e) {
      useNotificationStore().add('error', `创建共享失败: ${e}`)
    }
  }

  async function inviteMember(shareId: string, peerId: string, permission: string) {
    try {
      await api.inviteToShare(shareId, peerId, permission)
      const share = shares.value.find(s => s.shareId === shareId)
      if (share) {
        share.members.push({ deviceId: peerId, permission })
      }
      useNotificationStore().add('success', '已邀请成员')
    } catch (e) {
      useNotificationStore().add('error', `邀请成员失败: ${e}`)
    }
  }

  async function removeMember(shareId: string, peerId: string) {
    try {
      await api.removeShareMember(shareId, peerId)
      const share = shares.value.find(s => s.shareId === shareId)
      if (share) {
        share.members = share.members.filter(m => m.deviceId !== peerId)
      }
      useNotificationStore().add('info', '已移除成员')
    } catch (e) {
      useNotificationStore().add('error', `移除成员失败: ${e}`)
    }
  }

  async function startWatching(shareId: string) {
    try {
      await api.startWatchingShare(shareId)
      useNotificationStore().add('success', '已开始监控同步')
    } catch (e) {
      useNotificationStore().add('error', `启动监控失败: ${e}`)
    }
  }

  return { shares, showCreateDialog, invitingShareId, createShare, inviteMember, removeMember, startWatching }
})