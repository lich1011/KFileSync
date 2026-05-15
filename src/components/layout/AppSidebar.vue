<script setup lang="ts">
import { useRoute } from 'vue-router'

const route = useRoute()

const navItems = [
  { path: '/devices', label: '设备', icon: '📱' },
  { path: '/transfers', label: '传输', icon: '📁' },
  { path: '/shares', label: '共享', icon: '🔗' },
  { path: '/sync', label: '同步', icon: '' },
]
</script>

<template>
  <nav class="sidebar">
    <div class="sidebar__brand">KFileSync</div>
    <ul class="sidebar__nav">
      <li v-for="item in navItems" :key="item.path">
        <router-link
          :to="item.path"
          class="sidebar__link"
          :class="{ active: route.path === item.path }"
        >
          <span class="sidebar__icon">{{ item.icon }}</span>
          <span>{{ item.label }}</span>
        </router-link>
      </li>
    </ul>
  </nav>
</template>

<style scoped>
.sidebar {
  width: 180px;
  min-height: 100vh;
  background: var(--bg-card);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sidebar__brand {
  padding: 20px 16px;
  font-size: 18px;
  font-weight: 700;
  background: linear-gradient(135deg, #3498db, #8e44ad);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.sidebar__nav {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 0 8px;
}

.sidebar__link {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 8px;
  color: var(--text-muted);
  text-decoration: none;
  transition: all 0.15s;
}

.sidebar__link:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.sidebar__link.active {
  background: var(--accent);
  color: #fff;
}

.sidebar__icon {
  font-size: 16px;
}
</style>