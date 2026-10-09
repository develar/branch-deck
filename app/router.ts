import { createRouter, createWebHistory } from "vue-router"
import IndexPage from "./pages/index.vue"

// Sub-windows are opened by the backend as `WebviewUrl::App("/conflicting-files")` and `/missing-commits`
export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", component: IndexPage },
    { path: "/conflicting-files", component: () => import("./pages/conflicting-files.vue") },
    { path: "/missing-commits", component: () => import("./pages/missing-commits.vue") },
  ],
})
