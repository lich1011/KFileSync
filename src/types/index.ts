export interface Device {
  id: string
  alias: string
  address: string
  status: 'Unknown' | 'Discovered' | 'Paired' | 'Revoked'
}

export interface FileRequestDto {
  filePath: string
  fileSize: number
  sha256: string
}

export type TransferState = 'pending' | 'active' | 'paused' | 'verifying' | 'completed' | 'failed' | 'cancelled'

export interface TransferJob {
  jobId: string
  peerId: string
  peerAlias: string
  files: FileRequestDto[]
  state: TransferState
  createdAt: number
}

export type SyncMode = 'two_way' | 'send_only' | 'receive_only'

export interface ShareMember {
  deviceId: string
  permission: string
}

export interface ShareInfo {
  shareId: string
  shareName: string
  localPath: string
  syncMode: SyncMode
  status: string
  members: ShareMember[]
}

export interface PairedDevice{
  id: string
  alias: string
  address: string
  pairedAt: number
  lastSeenAt: number | null
  online: boolean
}

export interface SyncStatus{
  shareId: string
  totalFiles: number
  conflicts: number
}

export interface SyncConflict{
  conflictId: string
  shareId: string
  filePath: string
  resolution: string
}

export type NotificationType = 'success' | 'error' | 'warning' | 'info'

export interface Notification {
  id: string
  type: NotificationType
  message: string
  timestamp: number
}