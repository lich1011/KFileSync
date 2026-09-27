<script setup lang="ts">
import { cn } from '@/lib/utils'

interface Props {
  modelValue?: string | number
  type?: string
  placeholder?: string
  disabled?: boolean
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: '',
  type: 'text',
  disabled: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
  enter: []
}>()

function onInput(e: Event) {
  const target = e.target as HTMLInputElement
  emit('update:modelValue', target.value)
}
</script>

<template>
  <div :class="cn('relative flex items-center w-full', props.class)">
    <div v-if="$slots.prefix" class="absolute left-3.5 flex items-center text-on-surface-variant pointer-events-none">
      <slot name="prefix" />
    </div>
    <input
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      @input="onInput"
      @keyup.enter="emit('enter')"
      :class="
        cn(
          'w-full h-10 rounded-2xl bg-surface-container-high px-4 text-sm text-on-surface placeholder:text-on-surface-muted',
          'border border-outline-variant/50 focus:border-primary focus:outline-none focus:ring-2 focus:ring-primary/20',
          'transition-all duration-150 disabled:opacity-50 disabled:cursor-not-allowed',
          $slots.prefix ? 'pl-10' : '',
          $slots.suffix ? 'pr-10' : ''
        )
      "
    />
    <div v-if="$slots.suffix" class="absolute right-3.5 flex items-center text-on-surface-variant">
      <slot name="suffix" />
    </div>
  </div>
</template>
