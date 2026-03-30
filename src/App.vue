<script setup lang="ts">
import { listen } from "@tauri-apps/api/event"
import { useColorMode } from "@vueuse/core"
import { onBeforeUnmount, onMounted } from "vue"
import { useRouter } from "vue-router"
import { Toaster } from "@/components/ui/sonner"
import { useNodeRuntimeMenu } from "@/composables/useNodeRuntimeMenu"
import { useUpdater } from "@/composables/useUpdater"
import UpdaterDialog from "@/features/updater/UpdaterDialog.vue"
import "vue-sonner/style.css"

const updater = useUpdater()
const colorMode = useColorMode()
const router = useRouter()

let unlistenSetTheme: null | (() => void) = null

useNodeRuntimeMenu(() => {
  router.push({ name: "node-runtime" })
})

onMounted(async () => {
  unlistenSetTheme = await listen<string>("app://set-theme", (event) => {
    const next = event.payload
    if (next === "light" || next === "dark" || next === "auto")
      colorMode.value = next
  })
})

onBeforeUnmount(() => {
  unlistenSetTheme?.()
  unlistenSetTheme = null
})
</script>

<template>
  <RouterView />

  <UpdaterDialog
    v-model:open="updater.dialogOpen.value"
    :title="updater.dialogTitle.value"
    :description="updater.dialogDescription.value"
    :progress="updater.progress.value"
    :busy="updater.busy.value"
    :pending-restart="updater.pendingRestart.value"
    @restart="updater.restartApp"
  />

  <Toaster />
</template>
