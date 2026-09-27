<script setup lang="ts">
import { cn } from '@/lib/utils'

interface Props {
  variant?: 'elevated' | 'filled' | 'outlined'
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'filled',
})
</script>

<template>
  <div
    :class="
      cn(
        'rounded-3xl transition-all duration-200 overflow-hidden',
        props.variant === 'filled' && 'bg-surface-container',
        props.variant === 'elevated' && 'bg-surface-container-low shadow-m3-1 hover:shadow-m3-2',
        props.variant === 'outlined' && 'bg-surface border border-outline-variant/60',
        props.class
      )
    "
  >
    <slot />
  </div>
</template>
