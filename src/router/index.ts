import { createRouter, createWebHashHistory } from "vue-router"
import HomePage from "@/views/HomePage.vue"
import NodeRuntimePage from "@/views/NodeRuntimePage.vue"

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "home", component: HomePage },
    { path: "/node-runtime", name: "node-runtime", component: NodeRuntimePage },
  ],
})
