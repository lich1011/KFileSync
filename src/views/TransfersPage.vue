<script setup lang="ts">
import { useTransferStore } from '@/stores/transfers'
import TransferList from '@/components/transfers/TransferList.vue'
import SendFileDialog from '@/components/transfers/SendFileDialog.vue'
import { Button } from '@/components/ui'
import { SendHorizontal } from 'lucide-vue-next'

const store = useTransferStore()
</script>

<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-on-surface">文件传输</h1>
        <p class="text-sm text-on-surface-muted mt-1">局域网点对点点对点高速直传，无需经过互联网中心服务器</p>
      </div>

      <Button
        variant="filled"
        size="md"
        @click="store.showSendDialog = true"
        class="gap-2 shadow-sm"
      >
        <SendHorizontal class="w-4 h-4" />
        发送文件
      </Button>
    </div>

    <!-- Transfer Jobs List -->
    <TransferList />

    <!-- Send File Modal Dialog -->
    <SendFileDialog
      v-if="store.showSendDialog"
      @close="store.showSendDialog = false"
    />
  </div>
</template>