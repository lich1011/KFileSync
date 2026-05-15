<script setup lang="ts">
import { ref } from 'vue'

const props = defineProps<{
  deviceId: string
  pin: string
}>()

const emit = defineEmits<{
  close: []
  confirm: [pin: string, certPem: string]
  reject: []
}>()

const inputPin = ref(props.pin)
const certPem = ref('')
</script>

<template>
  <div class="overlay" @click.self="emit('close')">
    <div class="dialog">
      <h3>设备配对</h3>
      <p class="hint">请在对方设备上确认以下配对码：</p>
      <div class="pin-display">{{ pin }}</div>

      <label class="field">
        <span>确认 PIN 码</span>
        <input v-model="inputPin" placeholder="输入配对码" />
      </label>

      <label class="field">
        <span>对方证书 (PEM)</span>
        <textarea v-model="certPem" rows="3" placeholder="粘贴对方设备证书" />
      </label>

      <div class="actions">
       <button class="ghost" @click="emit('reject')">拒绝</button>
        <button class="ghost" @click="emit('close')">取消</button>
        <button class="primary" @click="emit('confirm', inputPin, certPem)">确认配对</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.dialog {
  background: var(--bg-card);
  border-radius: 12px;
  padding: 24px;
  width: 380px;
  max-width: 90vw;
}

h3 {
  font-size: 18px;
  margin-bottom: 8px;
}

.hint {
  color: var(--text-muted);
  font-size: 13px;
  margin-bottom: 16px;
}

.pin-display {
  font-size: 28px;
  font-weight: 700;
  letter-spacing: 6px;
  text-align: center;
  padding: 16px;
  background: var(--bg);
  border-radius: 8px;
  margin-bottom: 20px;
  color: var(--accent);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 14px;
  font-size: 13px;
  color: var(--text-muted);
}

textarea {
  font-family: inherit;
  font-size: inherit;
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 8px 12px;
  color: var(--text);
  outline: none;
  resize: vertical;
}

textarea:focus {
  border-color: var(--accent);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
}
</style>