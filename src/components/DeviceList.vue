<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useDeviceStore } from '@/stores/devices'
import { addManualDevice } from '@/api/tauri'
import { useNotificationStore } from '@/stores/notifications'
import { Button, Input, Badge, Card, CardHeader, CardTitle, CardContent } from '@/components/ui'
import {
  Laptop,
  RotateCw,
  Plus,
  Radio,
  CheckCircle2,
  ShieldCheck,
  Globe,
} from 'lucide-vue-next'

const store = useDeviceStore()
const manualIp = ref('')
const addingManual = ref(false)

onMounted(() => {
  store.fetchDevices()
})

async function onAddManualIp() {
  if (!manualIp.value.trim()) return
  addingManual.value = true
  try {
    const device = await addManualDevice(manualIp.value.trim())
    if (!store.devices.find((existing) => existing.id === device.id)) {
      store.devices.push(device)
    }
    manualIp.value = ''
    useNotificationStore().add('success', `发现并添加设备: ${device.alias}`)
  } catch (e: any) {
    useNotificationStore().add('error', `添加失败: ${e}`)
  } finally {
    addingManual.value = false
  }
}
</script>

<template>
  <div class="space-y-6">
    <!-- Header Area -->
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-2xl font-bold tracking-tight text-on-surface flex items-center gap-2.5">
          局域网设备
          <span
            v-if="store.loading"
            class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-medium bg-primary-container text-primary-on-container animate-pulse"
          >
            <Radio class="w-3 h-3 animate-spin" /> 正在广播探测
          </span>
        </h1>
        <p class="text-sm text-on-surface-muted mt-1">自动搜寻同局域网内运行 KFileSync 的电脑与手机</p>
      </div>

      <Button
        variant="tonal"
        size="md"
        :disabled="store.loading"
        @click="store.fetchDevices()"
        class="gap-2"
      >
        <RotateCw class="w-4 h-4" :class="{ 'animate-spin': store.loading }" />
        {{ store.loading ? '正在扫描...' : '重新扫描' }}
      </Button>
    </div>

    <!-- Manual IP Add Card -->
    <Card variant="filled" class="bg-surface-container-low border border-outline-variant/30">
      <div class="p-4 flex flex-col sm:flex-row gap-3 items-center">
        <div class="flex-1 w-full">
          <Input
            v-model="manualIp"
            placeholder="输入目标设备 IP 与端口（例如 192.168.1.100）"
            @enter="onAddManualIp"
          >
            <template #prefix>
              <Globe class="w-4 h-4 text-on-surface-muted" />
            </template>
          </Input>
        </div>
        <Button
          variant="filled"
          size="md"
          :disabled="addingManual || !manualIp.trim()"
          @click="onAddManualIp"
          class="w-full sm:w-auto"
        >
          <Plus class="w-4 h-4 mr-1" />
          {{ addingManual ? '正在连接...' : '手动添加' }}
        </Button>
      </div>
    </Card>

    <!-- Discovered Devices List -->
    <Card variant="filled" class="bg-surface-container border border-outline-variant/20 shadow-sm">
      <CardHeader class="pb-3 border-b border-outline-variant/20 flex flex-row items-center justify-between">
        <CardTitle class="text-sm font-semibold tracking-wide text-on-surface-variant flex items-center gap-2">
          发现的设备清单
          <Badge variant="surface" size="sm">{{ store.devices.length }} 台</Badge>
        </CardTitle>
      </CardHeader>

      <CardContent class="p-4">
        <!-- Empty / Scanning States -->
        <div v-if="store.loading && store.devices.length === 0" class="py-14 text-center">
          <div class="mx-auto w-14 h-14 rounded-full bg-primary-container flex items-center justify-center text-primary-on-container mb-3 animate-radar">
            <Radio class="w-7 h-7 animate-pulse" />
          </div>
          <p class="text-sm font-medium text-on-surface">正在向局域网广播搜寻节点...</p>
          <p class="text-xs text-on-surface-muted mt-1">请确保双方处于同一 Wi-Fi 或有线网络中</p>
        </div>

        <div v-else-if="store.devices.length === 0" class="py-14 text-center">
          <div class="mx-auto w-14 h-14 rounded-full bg-surface-container-highest flex items-center justify-center text-on-surface-muted mb-3">
            <Laptop class="w-7 h-7 opacity-60" />
          </div>
          <p class="text-sm font-medium text-on-surface">未探测到任何在线设备</p>
          <p class="text-xs text-on-surface-muted mt-1">可点击右上角「重新扫描」或通过上方直接指定 IP 连接</p>
        </div>

        <!-- Devices Grid / List -->
        <div v-else class="space-y-2.5">
          <div
            v-for="device in store.devices"
            :key="device.id"
            class="flex items-center justify-between p-4 rounded-2xl bg-surface-container-high/60 hover:bg-surface-container-high transition-colors border border-outline-variant/20"
          >
            <!-- Left: Device Icon & Info -->
            <div class="flex items-center gap-4 min-w-0">
              <div class="w-11 h-11 rounded-2xl bg-primary-container/60 flex items-center justify-center text-primary flex-shrink-0">
                <Laptop class="w-6 h-6" />
              </div>
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <h4 class="text-sm font-semibold text-on-surface truncate">{{ device.alias }}</h4>
                  <Badge
                    v-if="device.status === 'Paired'"
                    variant="success"
                    size="sm"
                  >
                    <CheckCircle2 class="w-3 h-3" /> 已配对安全连接
                  </Badge>
                  <Badge
                    v-else-if="device.status === 'Discovered'"
                    variant="primary"
                    size="sm"
                  >
                    等待配对
                  </Badge>
                  <Badge
                    v-else
                    variant="surface"
                    size="sm"
                  >
                    {{ device.status }}
                  </Badge>
                </div>
                <div class="flex items-center gap-3 text-xs text-on-surface-muted mt-1">
                  <span>{{ device.address }}</span>
                  <span class="text-outline-variant">•</span>
                  <span class="font-mono text-[11px] truncate max-w-[140px] opacity-70">ID: {{ device.id }}</span>
                </div>
              </div>
            </div>

            <!-- Right Action -->
            <div class="flex items-center gap-2 flex-shrink-0">
              <Button
                v-if="device.status === 'Discovered'"
                variant="filled"
                size="sm"
                @click="store.requestPairing(device.id)"
                class="gap-1.5"
              >
                <ShieldCheck class="w-4 h-4" />
                发起配对
              </Button>
              <div
                v-else-if="device.status === 'Paired'"
                class="px-3 py-1.5 rounded-full text-xs font-medium text-success bg-success-container/40 flex items-center gap-1.5"
              >
                <span class="w-2 h-2 rounded-full bg-success"></span>
                可进行安全同步
              </div>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  </div>
</template>