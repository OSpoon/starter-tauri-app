<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue"
import { useRouter } from "vue-router"
import { toast } from "vue-sonner"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip"

interface NodeRuntimeStatus {
  installed: boolean
  nodePath: string | null
  version: string | null
  serverRunning: boolean
  serverPort: number
}

const status = ref<NodeRuntimeStatus | null>(null)
const busy = ref(false)
const logs = ref<string[]>([])
const downloaded = ref(0)
const total = ref<number | null>(null)
const logEl = ref<HTMLElement | null>(null)
const autoScroll = ref(true)
const paused = ref(false)
const router = useRouter()

function statusBadgeVariant() {
  return status.value?.serverRunning ? "default" : "secondary"
}

function statusBadgeText() {
  return status.value?.serverRunning ? "Running" : "Stopped"
}

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    toast.success("Copied")
  }
  catch (e) {
    toast.error("Copy failed", { description: String(e) })
  }
}

async function copyNodePath() {
  if (!status.value?.nodePath)
    return
  await copy(status.value.nodePath)
}

let unlistenLog: null | (() => void) = null
let unlistenProgress: null | (() => void) = null

async function appendLog(line: string) {
  if (paused.value)
    return
  const ts = new Date().toLocaleTimeString()
  logs.value.push(`[${ts}] ${line}`)
  if (logs.value.length > 800)
    logs.value.splice(0, logs.value.length - 800)
  if (autoScroll.value) {
    await nextTick()
    logEl.value?.scrollTo({ top: logEl.value.scrollHeight })
  }
}

function goBack() {
  router.push({ name: "home" })
}

async function refresh() {
  status.value = await invoke<NodeRuntimeStatus>("node_runtime_status")
}

async function install() {
  busy.value = true
  try {
    status.value = await invoke<NodeRuntimeStatus>("node_runtime_install")
    toast.success("Node installed", { description: status.value.version ?? "OK" })
  }
  catch (e) {
    toast.error("Node install failed", { description: String(e) })
  }
  finally {
    busy.value = false
  }
}

async function uninstall() {
  busy.value = true
  try {
    status.value = await invoke<NodeRuntimeStatus>("node_runtime_uninstall")
    toast.success("Node uninstalled")
  }
  catch (e) {
    toast.error("Uninstall failed", { description: String(e) })
  }
  finally {
    busy.value = false
  }
}

async function startServer() {
  busy.value = true
  try {
    status.value = await invoke<NodeRuntimeStatus>("node_server_start")
    toast.success("Service started", { description: `127.0.0.1:${status.value.serverPort}` })
  }
  catch (e) {
    toast.error("Start failed", { description: String(e) })
  }
  finally {
    busy.value = false
  }
}

async function stopServer() {
  busy.value = true
  try {
    status.value = await invoke<NodeRuntimeStatus>("node_server_stop")
    toast.success("Service stopped")
  }
  catch (e) {
    toast.error("Stop failed", { description: String(e) })
  }
  finally {
    busy.value = false
  }
}

onMounted(async () => {
  unlistenLog = await listen<string>("node-runtime://log", async (event) => {
    await appendLog(event.payload)
  })
  unlistenProgress = await listen<{ downloaded: number, total: number | null }>(
    "node-runtime://download-progress",
    (event) => {
      downloaded.value = event.payload.downloaded
      total.value = event.payload.total
    },
  )
  await refresh()
})

onBeforeUnmount(() => {
  unlistenLog?.()
  unlistenLog = null
  unlistenProgress?.()
  unlistenProgress = null
})
</script>

<template>
  <TooltipProvider>
    <main class="min-h-screen w-full bg-background p-6">
      <div class="mx-auto flex w-full max-w-3xl flex-col gap-4">
        <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div class="min-w-0">
            <div class="flex flex-wrap items-center gap-3">
              <h1 class="text-2xl font-semibold tracking-tight">
                Node Runtime
              </h1>
              <Badge :variant="statusBadgeVariant()">
                {{ statusBadgeText() }}
              </Badge>
              <Badge :variant="status?.installed ? 'secondary' : 'destructive'">
                Installed: {{ status?.installed ? "Yes" : "No" }}
              </Badge>
              <Badge variant="outline">
                Version: {{ status?.version ?? "-" }}
              </Badge>
            </div>
            <p class="mt-1 text-sm text-muted-foreground">
              App-private Node runtime and local service manager (does not affect system Node).
            </p>

            <Tooltip v-if="status?.nodePath">
              <TooltipTrigger as-child>
                <button
                  type="button"
                  class="mt-2 w-full truncate rounded-md px-2 py-1 text-left font-mono text-xs text-muted-foreground hover:bg-muted"
                  :title="status.nodePath"
                  @click="copyNodePath"
                >
                  {{ status.nodePath }}
                </button>
              </TooltipTrigger>
              <TooltipContent class="max-w-[720px]">
                <span class="font-mono text-xs">{{ status.nodePath }}</span>
              </TooltipContent>
            </Tooltip>
          </div>
          <div class="flex items-center gap-2">
            <Button variant="outline" :disabled="busy" @click="goBack">
              Back
            </Button>
            <Button variant="outline" :disabled="busy" @click="refresh">
              Refresh
            </Button>
          </div>
        </div>

        <div class="flex items-center justify-between gap-3 rounded-lg border bg-card px-4 py-3">
          <div class="min-w-0">
            <div class="text-sm font-medium">
              Install Node (app-private)
            </div>
            <div v-if="total" class="mt-0.5 text-xs text-muted-foreground">
              Download: <span class="font-mono">{{ downloaded }} / {{ total }}</span>
            </div>
          </div>
          <div class="flex items-center gap-2">
            <Button class="h-9" :disabled="busy || status?.installed" @click="install">
              Install
            </Button>
            <Button
              variant="outline"
              class="h-9"
              :disabled="busy || !status?.installed"
              @click="uninstall"
            >
              Uninstall
            </Button>
          </div>
        </div>

        <Card>
          <CardHeader class="pb-3">
            <CardTitle class="text-base">
              Service & runtime
            </CardTitle>
            <CardDescription>
              Start/stop the local service.
            </CardDescription>
          </CardHeader>
          <CardContent class="space-y-4">
            <div class="flex flex-wrap items-center gap-2">
              <Button class="h-9" :disabled="busy || !status?.installed || status?.serverRunning" @click="startServer">
                Start service
              </Button>
              <Button variant="outline" class="h-9" :disabled="busy || !status?.serverRunning" @click="stopServer">
                Stop service
              </Button>
            </div>

            <div class="grid grid-cols-[72px_1fr] items-center gap-x-3 gap-y-2 text-sm">
              <div class="text-muted-foreground">
                URL
              </div>
              <div class="min-w-0 font-mono text-xs text-muted-foreground">
                http://127.0.0.1:{{ status?.serverPort ?? 3179 }}/health
              </div>
            </div>
          </CardContent>
        </Card>

        <Card class="overflow-hidden">
          <CardHeader class="pb-3">
            <CardTitle class="text-base">
              Console
            </CardTitle>
            <CardDescription class="flex items-center justify-between gap-3">
              <span>Live logs (pause and auto-scroll).</span>
              <label class="flex select-none items-center gap-2 text-xs text-muted-foreground">
                <input v-model="autoScroll" type="checkbox" class="accent-zinc-600">
                Auto-scroll
              </label>
            </CardDescription>
          </CardHeader>
          <CardContent class="p-0">
            <div class="bg-zinc-950 text-zinc-100">
              <div class="flex items-center justify-between border-b border-zinc-800 px-3 py-2">
                <div class="text-xs text-zinc-400">
                  {{ paused ? "Paused" : "Streaming" }}
                </div>
                <div class="flex items-center gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    class="h-8 border-zinc-700 bg-transparent text-zinc-200 hover:bg-zinc-900"
                    :disabled="busy || logs.length === 0"
                    @click="logs = []"
                  >
                    Clear
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    class="h-8 border-zinc-700 bg-transparent text-zinc-200 hover:bg-zinc-900"
                    :disabled="busy"
                    @click="paused = !paused"
                  >
                    {{ paused ? "Resume" : "Pause" }}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    class="h-8 border-zinc-700 bg-transparent text-zinc-200 hover:bg-zinc-900"
                    :disabled="busy || logs.length === 0"
                    @click="copy(logs.join('\n'))"
                  >
                    Copy all
                  </Button>
                </div>
              </div>
              <div ref="logEl" class="max-h-80 overflow-auto px-3 py-2 font-mono text-xs leading-relaxed">
                <div v-if="logs.length === 0" class="py-8 text-center text-zinc-500">
                  No logs yet
                </div>
                <div v-for="(l, i) in logs" :key="i" class="whitespace-pre-wrap">
                  {{ l }}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </main>
  </TooltipProvider>
</template>

