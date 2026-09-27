<script setup lang="ts">
import { ref } from 'vue'
import { Dialog, Button, Input } from '@/components/ui'
import { ShieldAlert, KeyRound } from 'lucide-vue-next'

const props = defineProps<{
  deviceId: string
  pin: string
  fromAlias?: string | null
}>()

const emit = defineEmits<{
  close: []
  confirm: [peerPin: string]
  reject: []
}>()

const peerPin = ref('')
</script>

<template>
  <Dialog :open="true" @close="emit('close')">
    <div class="space-y-5">
      <!-- Dialog Header -->
      <div class="flex items-center gap-3">
        <div class="w-10 h-10 rounded-2xl bg-primary-container flex items-center justify-center text-primary-on-container">
          <KeyRound class="w-5 h-5" />
        </div>
        <div>
          <h3 class="text-lg font-bold text-on-surface">设备安全配对</h3>
          <p class="hint" v-if="props.fromAlias">
            「{{ props.fromAlias }}」请求与本机配对。请将下方配对码告知对方，并输入对方的配对码：
          </p>
          <p class="hint" v-else>请将以下配对码告知对方，并让对方也将其实配对码告知你：</p>
        </div>
      </div>

      <!-- Local PIN Block (M3 Tonal Container) -->
      <div class="rounded-2xl bg-surface-container-lowest p-5 text-center border border-outline-variant/30">
        <span class="text-xs font-medium text-on-surface-variant tracking-wider uppercase">本机配对码 (告诉对方)</span>
        <div class="mt-2 text-3xl font-mono font-bold tracking-[0.35em] text-primary select-all">
          {{ pin }}
        </div>
      </div>

      <!-- Peer PIN Input -->
      <div class="space-y-2">
        <label class="text-xs font-semibold text-on-surface flex items-center gap-1.5">
          <ShieldAlert class="w-3.5 h-3.5 text-primary" />
          输入对方屏幕上显示的配对码
        </label>
        <Input
          v-model="peerPin"
          placeholder="例如 6 位数字或字母"
          @enter="emit('confirm', peerPin)"
          class="font-mono tracking-widest text-center"
        />
      </div>

      <!-- Dialog Action Buttons -->
      <div class="flex items-center justify-end gap-2 pt-2">
        <Button
          variant="ghost"
          size="md"
          @click="emit('reject')"
          class="text-error hover:bg-error-container/20"
        >
          拒绝
        </Button>
        <Button
          variant="outlined"
          size="md"
          @click="emit('close')"
        >
          取消
        </Button>
        <Button
          variant="filled"
          size="md"
          :disabled="!peerPin.trim()"
          @click="emit('confirm', peerPin)"
        >
          确认配对
        </Button>
      </div>
    </div>
  </Dialog>
</template>