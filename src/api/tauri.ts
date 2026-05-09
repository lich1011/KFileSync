import { invoke } from '@tauri-apps/api/core'
import type { Device, FileRequestDto } from '../types'

export async function discoverDevices(): Promise<Device[]> {
  const raw = await invoke<{ id: string; alias: string; address: string }[]>('discover_devices')
  return raw.map(d => ({ ...d, status: 'Discovered' as const }))
}

export async function requestPairing(targetId: string): Promise<string> {
  return invoke<string>('request_pairing', { targetId })
}

export async function confirmPairing(targetId: string, pinCode: string, certPem: string): Promise<void> {
  return invoke('confirm_pairing', { targetId, pinCode, certPem })
}

export async function sendFiles(peerId: string, files: FileRequestDto[]): Promise<string> {
  return invoke<string>('send_files', { peerId, files })
}

export async function acceptTransfer(jobId: string): Promise<void> {
  return invoke('accept_transfer', { jobId })
}

export async function pauseTransfer(jobId: string): Promise<void> {
  return invoke('pause_transfer', { jobId })
}

export async function resumeTransfer(jobId: string): Promise<void> {
  return invoke('resume_transfer', { jobId })
}

export async function cancelTransfer(jobId: string): Promise<void> {
  return invoke('cancel_transfer', { jobId })
}

export async function createShare(shareName: string, localPath: string, syncModeStr: string): Promise<string> {
  return invoke<string>('create_share', { shareName, localPath, syncModeStr })
}

export async function inviteToShare(shareId: string, peerId: string, permissionStr: string): Promise<void> {
  return invoke('invite_to_share', { shareId, peerId, permissionStr })
}

export async function removeShareMember(shareId: string, peerId: string): Promise<void> {
  return invoke('remove_share_member', { shareId, peerId })
}

export async function startWatchingShare(shareId: string): Promise<void> {
  return invoke('start_watching_share', { shareId })
}