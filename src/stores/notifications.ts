import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Notification, NotificationType } from '../types'

export const useNotificationStore = defineStore('notifications', () => {
  const notifications = ref<Notification[]>([])

  function add(type: NotificationType, message: string) {
    const id = Date.now().toString(36) + Math.random().toString(36).slice(2, 6)
    const n: Notification = { id, type, message, timestamp: Date.now() }
    notifications.value.push(n)
    setTimeout(() => remove(id), 5000)
  }

  function remove(id: string) {
    notifications.value = notifications.value.filter(n => n.id !== id)
  }

  return { notifications, add, remove }
})