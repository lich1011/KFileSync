<script setup lang="ts">
import { useTransferStore } from '../stores/transfers'
import TransferList from '../components/transfers/TransferList.vue'
import SendFileDialog from '../components/transfers/SendFileDialog.vue'

const store = useTransferStore()
</script>

<template>
  <div>
    <div class="page-header">
      <h1 class="page-title">文件传输</h1>
      <button class="primary" @click="store.showSendDialog = true">发送文件</button>
    </div>

    <TransferList />

    <SendFileDialog
      v-if="store.showSendDialog"
      @close="store.showSendDialog = false"
    />
  </div>
</template>

<style scoped>
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.page-title {
  font-size: 22px;
  font-weight: 600;
}
</style>