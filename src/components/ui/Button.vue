<script setup lang="ts">
import { computed } from 'vue'
import { cn } from '@/lib/utils'

interface Props {
  variant?: 'filled' | 'tonal' | 'outlined' | 'text' | 'danger' | 'ghost'
  size?: 'sm' | 'md' | 'lg' | 'icon'
  disabled?: boolean
  type?: 'button' | 'submit' | 'reset'
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'filled',
  size: 'md',
  disabled: false,
  type: 'button',
})

const variantClasses = computed(() => {
  switch (props.variant) {
    case 'filled':
      return 'bg-primary text-primary-foreground hover:brightness-110 active:brightness-95 shadow-sm'
    case 'tonal':
      return 'bg-primary-container text-primary-on-container hover:brightness-110 active:brightness-95'
    case 'outlined':
      return 'border border-outline/40 text-primary hover:bg-primary/10 active:bg-primary/15'
    case 'text':
      return 'text-primary hover:bg-primary/10 active:bg-primary/15'
    case 'danger':
      return 'bg-error text-error-foreground hover:brightness-110 active:brightness-95'
    case 'ghost':
      return 'text-on-surface hover:bg-on-surface/8 active:bg-on-surface/12'
    default:
      return 'bg-primary text-primary-foreground'
  }
})

const sizeClasses = computed(() => {
  switch (props.size) {
    case 'sm':
      return 'h-8 px-3.5 text-xs gap-1.5'
    case 'lg':
      return 'h-12 px-6 text-base gap-2.5'
    case 'icon':
      return 'h-9 w-9 p-0 justify-center'
    case 'md':
    default:
      return 'h-10 px-5 text-sm gap-2'
  }
})
</script>

<template>
  <button
    :type="type"
    :disabled="disabled"
    :class="
      cn(
        'inline-flex items-center justify-center font-medium rounded-full select-none outline-none transition-all duration-150',
        'active:scale-[0.98] disabled:opacity-40 disabled:pointer-events-none disabled:active:scale-100',
        variantClasses,
        sizeClasses,
        props.class
      )
    "
  >
    <slot />
  </button>
</template>
