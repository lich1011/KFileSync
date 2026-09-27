<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/lib/utils'

interface Props {
  variant?: 'primary' | 'secondary' | 'success' | 'error' | 'outline' | 'surface'
  size?: 'sm' | 'md'
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'primary',
  size: 'md',
})

const variantClasses = computed(() => {
  switch (props.variant) {
    case 'primary':
      return 'bg-primary-container text-primary-on-container'
    case 'secondary':
      return 'bg-secondary-container text-secondary-on-container'
    case 'success':
      return 'bg-success-container text-success-on-container'
    case 'error':
      return 'bg-error-container text-error-on-container'
    case 'outline':
      return 'border border-outline/50 text-on-surface'
    case 'surface':
    default:
      return 'bg-surface-container-highest text-on-surface-variant'
  }
})

const sizeClasses = computed(() => {
  switch (props.size) {
    case 'sm':
      return 'px-2 py-0.5 text-[11px] font-medium'
    case 'md':
    default:
      return 'px-3 py-1 text-xs font-medium'
  }
})
</script>

<template>
  <span
    :class="
      cn(
        'inline-flex items-center gap-1.5 rounded-full select-none tracking-wide',
        variantClasses,
        sizeClasses,
        props.class
      )
    "
  >
    <slot />
  </span>
</template>
