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

  async function fetchDevices() {
    loading.value = true
    try {
      devices.value = await api.discoverDevices()
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

  async function confirmPairing(pin: string, certPem: string) {
    if (!pairingDeviceId.value) return
    try {
      await api.confirmPairing(pairingDeviceId.value, pin, certPem)
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
    if(!pairingDeviceId.value) return
    try{
      await api.rejectPairing(pairingDeviceId.value)
      useNotificationStore().add('info',"已拒绝配对")
    }catch(e){
      useNotificationStore().add('error',"已拒绝配对")
    }finally{
      pairingDeviceId.value = null
      pairingPin.value =null
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
    fetchDevices, 
    requestPairing, 
    confirmPairing, 
    rejectPairing,
    closePairingDialog 
  }
})