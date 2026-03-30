import http from "node:http"
import { URL } from "node:url"

const portArgIndex = process.argv.findIndex(a => a === "--port")
const port = Number(
  (portArgIndex >= 0 && process.argv[portArgIndex + 1])
  || process.env.PORT
  || "3179",
)

const server = http.createServer((req, res) => {
  const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`)
  if (url.pathname === "/health") {
    res.writeHead(200, { "content-type": "application/json" })
    res.end(JSON.stringify({ ok: true }))
    return
  }

  res.writeHead(404, { "content-type": "application/json" })
  res.end(JSON.stringify({ error: "not_found" }))
})

server.on("error", (err) => {
  // Keep output single-line-ish for UI console readability.
  console.error(`server_error:${err?.code || "UNKNOWN"}:${err?.message || String(err)}`)
  process.exit(1)
})

server.listen(port, "127.0.0.1", () => {
  // Single-line for easy parsing
  console.log(`listening:${port}`)
})

