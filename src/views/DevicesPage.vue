<script setup lang="ts">
import DeviceList from '../components/DeviceList.vue'
import PairDialog from '../components/devices/PairDialog.vue'
import { useDeviceStore } from '../stores/devices'
import { onMounted, onUnmounted } from 'vue'
import type { UnlistenFn } from '@tauri-apps/api/event'

const store = useDeviceStore()

let unlisten: UnlistenFn | null = null

onMounted(async () => {
    store.fetchLocalAddress()
    unlisten = await store.listenForIncomingPairing()
})

onUnmounted(() => {
    unlisten?.()
})
</script>



<template>
  <div class="space-y-6">
    <h1 class="page-title">发现设备</h1>
    <p v-if="store.localAddress" class="local-address">本机 IP: {{ store.localAddress }}</p>
    <DeviceList />

    <!-- M3 Pairing Dialog -->
    <PairDialog
      v-if="store.pairingDeviceId"
      :device-id="store.pairingDeviceId"
      :pin="store.pairingPin ?? ''"
      :from-alias="store.pairingFromAlias"
      @close="store.closePairingDialog()"
      @confirm="(peerPin) => store.confirmPairing(peerPin)"
      @reject="store.rejectPairing()"
    />
  </div>
</template>

<style scoped>
.page_title {
  font-size: 22px;
  font-weight: 600;
  margin-bottom: 20px;
}

.local-address {
    font-size: 13px;
    color: var(--text-muted);
    margin-top: -12px;
    margin-bottom: 16px;
}
</style>