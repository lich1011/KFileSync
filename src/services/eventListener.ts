import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useTransferStore } from '@/stores/transfers'
import { useNotificationStore } from '@/stores/notifications'
import { useSyncStore } from '@/stores/sync'

interface TauriEventPayload {
  aggregateId: string
  eventType: string
}

export async function setupEventListener(): Promise<() => void> {
  const unlisteners: UnlistenFn[] = []

  try {
    const notify = useNotificationStore()
    const transferStore = useTransferStore()
    const syncStore = useSyncStore()

    // 1. 传输完成
    const unlistenCompleted = await listen<TauriEventPayload>('TransferCompleted', (event) => {
      const jobId = event.payload.aggregateId
      const job = transferStore.transfers.find((t) => t.jobId === jobId)
      if (job) {
        job.state = 'completed'
      }
      notify.add('success', `文件传输任务 [${jobId.slice(0, 8)}] 已圆满完成！`)
    })
    unlisteners.push(unlistenCompleted)

    // 2. 传输失败
    const unlistenFailed = await listen<TauriEventPayload>('TransferFailed', (event) => {
      const jobId = event.payload.aggregateId
      const job = transferStore.transfers.find((t) => t.jobId === jobId)
      if (job) {
        job.state = 'failed'
      }
      notify.add('error', `文件传输任务 [${jobId.slice(0, 8)}] 传输中断或失败`)
    })
    unlisteners.push(unlistenFailed)

    // 3. 传输进度更新
    const unlistenProgress = await listen<TauriEventPayload>('TransferProgressUpdated', (event) => {
      const jobId = event.payload.aggregateId
      const job = transferStore.transfers.find((t) => t.jobId === jobId)
      if (job && job.state !== 'completed') {
        job.state = 'active'
      }
    })
    unlisteners.push(unlistenProgress)

    // 4. 收到对端发送的文件请求
    const unlistenRequested = await listen<TauriEventPayload>('TransferRequested', (event) => {
      const jobId = event.payload.aggregateId
      if (!transferStore.transfers.some((t) => t.jobId === jobId)) {
        transferStore.transfers.unshift({
          jobId,
          peerId: '远端设备',
          peerAlias: '远端对端设备',
          files: [],
          state: 'pending',
          createdAt: Date.now(),
        })
      }
      notify.add('info', `收到来自远端的新传输请求，请在传输列表查看并处理`)
    })
    unlisteners.push(unlistenRequested)

    // 5. 检测到版本冲突
    const unlistenConflict = await listen<TauriEventPayload>('ConflictDetected', (event) => {
      const shareId = event.payload.aggregateId
      notify.add('warning', `检测到同步文件版本冲突，请至「数据同步」页面进行版本决策`)
      syncStore.fetchConflicts(shareId).catch(() => {})
      syncStore.fetchStatus(shareId).catch(() => {})
    })
    unlisteners.push(unlistenConflict)

    // 6. 目录同步完成
    const unlistenSync = await listen<TauriEventPayload>('SyncCompleted', (event) => {
      const shareId = event.payload.aggregateId
      notify.add('success', `共享目录 [${shareId.slice(0, 8)}] 同步已完成！`)
      syncStore.fetchStatus(shareId).catch(() => {})
    })
    unlisteners.push(unlistenSync)
  } catch (err) {
    console.warn('Tauri event listeners could not be initialized in non-Tauri environment:', err)
  }

  // 返回清理函数
  return () => {
    for (const unlisten of unlisteners) {
      unlisten()
    }
  }
}
