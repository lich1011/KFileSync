<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useShareStore } from '@/stores/shares'
import CreateShareDialog from '@/components/shares/CreateShareDialog.vue'
import InviteMemberDialog from '@/components/shares/InviteMemberDialog.vue'
import { Button, Badge, Card, CardHeader, CardTitle, CardContent } from '@/components/ui'
import {
  FolderSync,
  Plus,
  ChevronDown,
  ChevronUp,
  FolderOpen,
  Users,
  Play,
  UserPlus,
  Trash2,
  HardDrive,
} from 'lucide-vue-next'

const store = useShareStore()
const expandedShareId = ref<string | null>(null)

onMounted(() => {
  store.fetchShares()
})

function toggleExpand(shareId: string) {
  expandedShareId.value = expandedShareId.value === shareId ? null : shareId
}

function modeLabel(mode: string) {
  switch (mode) {
    case 'two_way':
      return '双向同步'
    case 'send_only':
      return '仅发送'
    case 'receive_only':
      return '仅接收'
    default:
      return mode
  }
}

function permLabel(perm: string) {
  switch (perm) {
    case 'read_write':
      return '读写权限'
    case 'read_only':
      return '只读权限'
    case 'send_only':
      return '仅发送'
    case 'receive_only':
      return '仅接收'
    default:
      return perm
  }
}
</script>

<template>
  <div class="space-y-6">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-on-surface">目录共享</h1>
        <p class="text-sm text-on-surface-muted mt-1">管理多设备协同同步的共享目录与成员权限</p>
      </div>

      <Button
        variant="filled"
        size="md"
        @click="store.showCreateDialog = true"
        class="gap-2 shadow-sm"
      >
        <Plus class="w-4 h-4" />
        创建共享目录
      </Button>
    </div>

    <!-- Shares List -->
    <Card variant="filled" class="bg-surface-container border border-outline-variant/20 shadow-sm">
      <CardHeader class="pb-3 border-b border-outline-variant/20 flex flex-row items-center justify-between">
        <CardTitle class="text-sm font-semibold tracking-wide text-on-surface-variant flex items-center gap-2">
          已配置的共享目录
          <Badge variant="surface" size="sm">{{ store.shares.length }} 个</Badge>
        </CardTitle>
      </CardHeader>

      <CardContent class="p-4">
        <!-- Empty State -->
        <div v-if="store.shares.length === 0" class="py-16 text-center">
          <div class="mx-auto w-14 h-14 rounded-full bg-surface-container-highest flex items-center justify-center text-on-surface-muted mb-3">
            <FolderOpen class="w-7 h-7 opacity-60" />
          </div>
          <p class="text-sm font-medium text-on-surface">暂无共享目录</p>
          <p class="text-xs text-on-surface-muted mt-1">点击右上角「创建共享目录」选择本地文件夹即可开启同步</p>
        </div>

        <!-- Share Items -->
        <div v-else class="space-y-3">
          <div
            v-for="share in store.shares"
            :key="share.shareId"
            class="rounded-2xl bg-surface-container-high/60 border border-outline-variant/20 overflow-hidden transition-all duration-200 hover:border-outline-variant/50"
          >
            <!-- Share Header Row -->
            <div
              class="flex flex-col sm:flex-row sm:items-center justify-between p-4 cursor-pointer select-none hover:bg-surface-container-high gap-3"
              @click="toggleExpand(share.shareId)"
            >
              <div class="flex items-center gap-3.5 min-w-0">
                <div class="w-11 h-11 rounded-2xl bg-tertiary-container flex items-center justify-center text-tertiary-on-container flex-shrink-0">
                  <FolderSync class="w-5 h-5" />
                </div>
                <div class="min-w-0">
                  <div class="flex items-center gap-2">
                    <h4 class="text-sm font-semibold text-on-surface truncate">{{ share.shareName }}</h4>
                    <Badge variant="primary" size="sm">{{ modeLabel(share.syncMode) }}</Badge>
                    <Badge
                      :variant="share.status === 'active' ? 'success' : 'secondary'"
                      size="sm"
                    >
                      {{ share.status === 'active' ? '实时监听中' : '已暂停' }}
                    </Badge>
                  </div>
                  <div class="flex items-center gap-2 text-xs text-on-surface-muted mt-1 truncate">
                    <span class="font-mono text-[11px] truncate">{{ share.localPath }}</span>
                    <span class="text-outline-variant">•</span>
                    <span class="flex items-center gap-1">
                      <Users class="w-3.5 h-3.5" /> {{ share.members.length }} 位成员
                    </span>
                  </div>
                </div>
              </div>

              <!-- Actions on right -->
              <div class="flex items-center gap-2 self-end sm:self-center flex-shrink-0">
                <Button
                  variant="tonal"
                  size="sm"
                  @click.stop="store.startWatching(share.shareId)"
                  class="gap-1.5"
                >
                  <Play class="w-3.5 h-3.5" />
                  开始同步
                </Button>
                <Button
                  variant="outlined"
                  size="sm"
                  @click.stop="store.invitingShareId = share.shareId"
                  class="gap-1.5"
                >
                  <UserPlus class="w-3.5 h-3.5" />
                  邀请成员
                </Button>
                <div class="p-1 text-on-surface-muted">
                  <ChevronUp v-if="expandedShareId === share.shareId" class="w-4 h-4" />
                  <ChevronDown v-else class="w-4 h-4" />
                </div>
              </div>
            </div>

            <!-- Expanded Members Area -->
            <div
              v-if="expandedShareId === share.shareId"
              class="px-5 py-4 border-t border-outline-variant/20 bg-surface-container-low/50 space-y-3"
            >
              <div class="text-xs font-semibold text-on-surface-variant flex items-center justify-between">
                <span>同步成员列表</span>
                <span class="text-[11px] text-on-surface-muted">共 {{ share.members.length }} 台设备协同</span>
              </div>

              <div v-if="share.members.length === 0" class="py-3 text-center text-xs text-on-surface-muted">
                当前暂无成员，点击右侧「邀请成员」添加局域网设备
              </div>

              <div v-else class="space-y-1.5">
                <div
                  v-for="member in share.members"
                  :key="member.deviceId"
                  class="flex items-center justify-between p-2.5 rounded-xl bg-surface-container-high/40 border border-outline-variant/15 text-xs"
                >
                  <div class="flex items-center gap-2.5">
                    <HardDrive class="w-4 h-4 text-primary" />
                    <span class="font-mono text-on-surface font-medium">{{ member.deviceId }}</span>
                    <Badge variant="surface" size="sm">{{ permLabel(member.permission) }}</Badge>
                  </div>

                  <Button
                    variant="ghost"
                    size="sm"
                    @click="store.removeMember(share.shareId, member.deviceId)"
                    class="text-error hover:bg-error-container/20 h-7 px-2.5 text-xs gap-1"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                    移除
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <!-- Dialogs -->
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