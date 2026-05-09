<script setup lang="ts">
import { ref } from 'vue'
import { useShareStore } from '../stores/shares'
import CreateShareDialog from '../components/shares/CreateShareDialog.vue'
import InviteMemberDialog from '../components/shares/InviteMemberDialog.vue'

const store = useShareStore()
const expandedShareId = ref<string | null>(null)

function toggleExpand(shareId: string) {
  expandedShareId.value = expandedShareId.value === shareId ? null : shareId
}
</script>

<template>
  <div>
    <div class="page-header">
      <h1 class="page-title">目录共享</h1>
      <button class="primary" @click="store.showCreateDialog = true">创建共享</button>
    </div>

    <div class="share-list">
      <div v-if="store.shares.length === 0" class="empty">
        暂无共享目录
      </div>

      <div v-for="share in store.shares" :key="share.shareId" class="share-card">
        <div class="share-header" @click="toggleExpand(share.shareId)">
          <div class="share-info">
            <div class="share-name">{{ share.shareName }}</div>
            <div class="share-meta">
              {{ share.localPath }} · {{ share.syncMode }} · {{ share.members.length }} 成员
            </div>
          </div>
          <div class="share-actions">
            <span class="share-status" :class="share.status">{{ share.status }}</span>
            <button class="ghost" @click.stop="store.startWatching(share.shareId)">开始同步</button>
            <button class="ghost" @click.stop="store.invitingShareId = share.shareId">邀请</button>
          </div>
        </div>

        <div v-if="expandedShareId === share.shareId" class="share-members">
          <div class="members-title">成员列表</div>
          <div v-if="share.members.length === 0" class="empty-small">暂无成员</div>
          <div v-for="member in share.members" :key="member.deviceId" class="member-item">
            <span>{{ member.deviceId }}</span>
            <span class="member-perm">({{ member.permission }})</span>
            <button class="ghost danger-text" @click="store.removeMember(share.shareId, member.deviceId)">移除</button>
          </div>
        </div>
      </div>
    </div>

    <CreateShareDialog
      v-if="store.showCreateDialog"
      @close="store.showCreateDialog = false"
    />

    <InviteMemberDialog
      v-if="store.invitingShareId"
      :share-id="store.invitingShareId"
      @close="store.invitingShareId = null"
    />
  </div>
</template>

<style scoped>
.page-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.page-title {
  font-size: 22px;
  font-weight: 600;
}

.share-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.empty {
  text-align: center;
  color: var(--text-muted);
  padding: 40px;
  background: var(--bg-card);
  border-radius: 12px;
}

.share-card {
  background: var(--bg-card);
  border-radius: 12px;
  overflow: hidden;
}

.share-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 16px 20px;
  cursor: pointer;
  transition: background 0.15s;
}

.share-header:hover {
  background: var(--bg-hover);
}

.share-name {
  font-weight: 600;
}

.share-meta {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 4px;
}

.share-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.share-status {
  border-radius: 10px;
  padding: 2px 8px; /* 根据视觉推断 */
  font-size: 12px;
}

.share-status.active {
  background: rgba(46, 204, 113, 0.15);
  color: var(--success);
}

.share-status.paused {
  background: rgba(243, 156, 18, 0.15);
  color: var(--warning);
}

.share-members {
  border-top: 1px solid var(--border);
  padding: 16px 20px;
}

.members-title {
  font-size: 13px;
  color: var(--text-muted);
  margin-bottom: 10px;
}

.empty-small {
  font-size: 13px;
  color: var(--text-muted);
}

.member-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 0;
  font-size: 13px;
  border-bottom: 1px solid var(--border);
}

.member-item:last-child {
  border-bottom: none;
}

.member-perm {
  color: var(--text-muted);
  font-size: 12px;
}

.danger-text {
  color: var(--danger) !important;
}
</style>