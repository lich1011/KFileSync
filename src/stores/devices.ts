import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Device } from '../types'
import * as api from '../api/tauri'
import { useNotificationStore } from './notifications'

export const useDeviceStore = defineStore('devices', () => {
  const devices = ref<Device[]>([])
  const loading = ref(false)
  const pairingDeviceId = ref<string | null>(null)
  const pairingPin = ref<string | null>(null)
  const pairingFromAlias = ref<string | null>(null)
  const localAddress = ref<string | null>(null)

  async function fetchLocalAddress() {
    try {
      localAddress.value = await api.getLocalAddress()
    } catch (e) {
      useNotificationStore().add('error', `获取本机 IP 失败: ${e}`)
    }
  }

  /** Zero-trust responder side (ADR-010): a peer initiated pairing against us.
   * Surface their PIN prompt the same way the initiator dialog does, so the
   * local user reads/compares PINs before confirming. Call once at app start. */
  function listenForIncomingPairing() {
    return api.onPairingRequestReceived(payload => {
      pairingDeviceId.value = payload.fromDeviceId
      pairingPin.value = payload.ourPin
      pairingFromAlias.value = payload.fromAlias
    })
  }

  async function fetchDevices() {
    loading.value = true
    try {
      const [discovered, paired] = await Promise.all([
        api.discoverDevices(),
        api.getPairedDevices().catch(() => []),
      ])

      const pairedMap = new Map(paired.map((p) => [p.id, p]))

      // 1. 扫描到的设备：若在已配对列表中，则状态置为 'Paired'
      const merged: Device[] = discovered.map((d) => ({
        ...d,
        status: pairedMap.has(d.id) ? 'Paired' : 'Discovered',
      }))

      // 2. 补全已配对但本次局域网广播未响应的离线设备
      for (const p of paired) {
        if (!merged.some((d) => d.id === p.id)) {
          merged.push({
            id: p.id,
            alias: p.alias,
            address: p.address,
            status: 'Paired',
          })
        }
      }

      devices.value = merged
    } catch (e) {
      useNotificationStore().add('error', `发现设备失败: ${e}`)
    } finally {
      loading.value = false
    }
  }

  async function requestPairing(deviceId: string) {
    try {
      const pin = await api.requestPairing(deviceId)
      pairingDeviceId.value = deviceId
      pairingPin.value = pin
    } catch (e) {
      useNotificationStore().add('error', `请求配对失败: ${e}`)
    }
  }

  async function confirmPairing(peerPin: string) {
    if (!pairingDeviceId.value) return
    try {
      await api.confirmPairing(pairingDeviceId.value, peerPin)
      const device = devices.value.find(d => d.id === pairingDeviceId.value)
      if (device) device.status = 'Paired'
      useNotificationStore().add('success', '配对成功')
    } catch (e) {
      useNotificationStore().add('error', `确认配对失败: ${e}`)
    } finally {
      pairingDeviceId.value = null
      pairingPin.value = null
    }
  }

  async function rejectPairing() {
    if (!pairingDeviceId.value) return
    try {
      await api.rejectPairing(pairingDeviceId.value)
      useNotificationStore().add('info', "已拒绝配对")
    } catch (e) {
      useNotificationStore().add('error', "拒绝配对失败")
    } finally {
      pairingDeviceId.value = null
      pairingPin.value = null
    }
  }

  function closePairingDialog() {
    pairingDeviceId.value = null
    pairingPin.value = null
  }

  return {
    devices,
    loading,
    pairingDeviceId,
    pairingPin,
    pairingFromAlias,
    localAddress,
    fetchLocalAddress,
    listenForIncomingPairing,
    fetchDevices,
    requestPairing,
    confirmPairing,
    rejectPairing,
    closePairingDialog
  }
})