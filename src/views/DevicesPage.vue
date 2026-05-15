<script setup lang="ts">
import DeviceList from '../components/DeviceList.vue'
import PairDialog from '../components/devices/PairDialog.vue'
import { useDeviceStore } from '../stores/devices'

const store = useDeviceStore()
</script>

<template>
  <div>
    <h1 class="page-title">设备发现</h1>
    <DeviceList />
    
    <PairDialog
      v-if="store.pairingDeviceId"
      :device-id="store.pairingDeviceId"
      :pin="store.pairingPin ?? ''"
      @close="store.closePairingDialog()"
      @confirm="(pin, cert) => store.confirmPairing(pin, cert)"
      @reject="store.rejectPairing()"
    />
  </div>
</template>

<style scoped>
.page-title {
  font-size: 22px;
  font-weight: 600;
  margin-bottom: 20px;
}
</style>