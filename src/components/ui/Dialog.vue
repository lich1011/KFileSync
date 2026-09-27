<script setup lang="ts">
import { cn } from '@/lib/utils'

interface Props {
  open?: boolean
  class?: string
}

withDefaults(defineProps<Props>(), {
  open: true,
})

const emit = defineEmits<{
  close: []
}>()
</script>

<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 z-50 flex items-center justify-center p-4"
    >
      <!-- Backdrop with subtle blur -->
      <div
        class="fixed inset-0 bg-black/60 backdrop-blur-[2px] transition-opacity animate-in fade-in"
        @click="emit('close')"
      />

      <!-- M3 Dialog Surface Container -->
      <div
        :class="
          cn(
            'relative z-50 w-full max-w-md rounded-3xl bg-surface-container-high p-6 shadow-m3-3 border border-outline-variant/30',
            'transition-all duration-200 animate-in zoom-in-95',
            $props.class
          )
        "
      >
        <slot />
      </div>
    </div>
  </Teleport>
</template>
