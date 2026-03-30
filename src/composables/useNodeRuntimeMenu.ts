import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { onBeforeUnmount, onMounted } from "vue"
import { toast } from "vue-sonner"

type NavigateFn = () => void

export function useNodeRuntimeMenu(navigateToNodeRuntime: NavigateFn) {
  let unlistenOpen: null | (() => void) = null
  let unlistenStart: null | (() => void) = null
  let unlistenStop: null | (() => void) = null

  onMounted(async () => {
    unlistenOpen = await listen("app://open-node-runtime", async () => {
      navigateToNodeRuntime()
    })

    unlistenStart = await listen("app://node-server-start", async () => {
      navigateToNodeRuntime()
      try {
        await invoke("node_server_start")
        toast.success("Service started")
      }
      catch (e) {
        toast.error("Start failed", { description: String(e) })
      }
    })

    unlistenStop = await listen("app://node-server-stop", async () => {
      navigateToNodeRuntime()
      try {
        await invoke("node_server_stop")
        toast.success("Service stopped")
      }
      catch (e) {
        toast.error("Stop failed", { description: String(e) })
      }
    })
  })

  onBeforeUnmount(() => {
    unlistenOpen?.()
    unlistenOpen = null
    unlistenStart?.()
    unlistenStart = null
    unlistenStop?.()
    unlistenStop = null
  })
}

