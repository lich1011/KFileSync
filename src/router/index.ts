import { createRouter, createWebHashHistory } from 'vue-router'
import DevicesPage from '../views/DevicesPage.vue'
import TransfersPage from '../views/TransfersPage.vue'
import SharesPage from '../views/SharesPage.vue'
import SyncPage from '../views/SyncPage.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/devices' },
    { path: '/devices', component: DevicesPage },
    { path: '/transfers', component: TransfersPage },
    { path: '/shares', component: SharesPage },
  ],
})

export default router