# HTTP & Networking

Perry natively implements HTTP servers, clients, and WebSocket support.

## Node.js compatibility — `node:http` / `node:https` / `node:http2`

Perry exposes a faithful subset of Node.js's stdlib HTTP server modules
on top of hyper + rustls + tokio-tungstenite. The whole shape — handler
signature, IncomingMessage / ServerResponse properties + methods,
TLS opts, ALPN-negotiated HTTP/2, WebSocket upgrade dispatch — works
unmodified, so unmodified Node servers (Express / Koa / Polka / hono via
`@hono/node-server` / etc.) compile and run natively (issue #577).

### `http.createServer(handler)`

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:node-http-server}}
```

Supported on `IncomingMessage`: `.method`, `.url`, `.headers`,
`.rawHeaders`, `.httpVersion`, `.complete`, `.aborted`, `.destroyed`,
`.socket.remoteAddress`, `.socket.remotePort`, `.on('data'|'end'|'close'|
'error', cb)`, `.read()`, `.pause()`, `.resume()`, `.destroy()`.

Supported on `ServerResponse`: `.statusCode` (get/set),
`.statusMessage` (set), `.setHeader/.getHeader/.removeHeader/.hasHeader/
.getHeaders/.getHeaderNames`, `.headersSent`, `.writableEnded`,
`.writableFinished`, `.writeHead(status, msg?, headers?)`,
`.write(chunk)`, `.end(chunk?)`, `.flushHeaders()`,
`.on('finish'|'close', cb)`. Auto Content-Length on `.end()` when no
`Transfer-Encoding` was set.

### `https.createServer({ key, cert }, handler)`

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:node-https-server}}
```

Both `key` and `cert` are PEM strings (PKCS#8 / RSA / EC keys + multi-cert
chains all parse). ALPN defaults to `http/1.1` only — programs that want
HTTP/2 should reach for `node:http2`'s `createSecureServer` (which always
advertises `[h2, http/1.1]`).

### `http2.createSecureServer({ key, cert }, handler)`

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:node-http2-server}}
```

Driven through `hyper-util`'s `auto::Builder`, so an HTTP/1.1 client
(curl without `--http2`) and an HTTP/2 client (curl with `--http2`)
hit the same handler over the same port.

### WebSocket upgrade — `Server.on('upgrade', (req, wsId, head) => …)`

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:node-http-ws-upgrade}}
```

The HTTP/1.1 server detects `Upgrade: websocket` in the request,
performs the handshake server-side (Sec-WebSocket-Accept derived via
tungstenite's `derive_accept_key`), then registers the upgraded stream
in perry-ext-ws's connection map. The TS-side `wsId` argument is
already a fully-connected client — drive it via the standard
`wsId.on('message', cb)` / `wsId.send(msg)` / `wsId.close()` surface
that standalone `WebSocketServer({ port })` clients use.

## Hono

[Hono](https://hono.dev/) is a runtime-agnostic web framework whose only
required interface is `app.fetch(req: Request) → Promise<Response>`. Add
it to `perry.compilePackages` and the entire `app.fetch` surface
including middleware (`hono/logger`, `hono/cors`, `hono/jwt`), route
groups, and JSON responses works unchanged (issues #421, #486, #487
closed). `app.fetch` is enough for testing, edge-runtime deployments
(Cloudflare Workers / Vercel Edge / AWS Lambda / Deno Deploy — those
runtimes call `app.fetch` themselves), and any scenario where some
outer host hands you a `Request`.

```typescript
import { Hono } from "hono"
import { logger } from "hono/logger"

const app = new Hono()
app.use("*", logger())
app.get("/", (c) => c.json({ message: "hello", ok: true }))

// app.fetch() works end-to-end — feed it a Request, get a Response.
const res = await app.fetch(new Request("http://localhost/"))
console.log(res.status, await res.text())

export default app  // for CF Workers / similar runtimes
```

`package.json`:

```json
{
  "perry": {
    "compilePackages": ["hono"]
  }
}
```

### Long-lived HTTP server (port-listening) — currently blocked

The canonical "deploy a hono app as a native binary on a Linux VM"
pattern — `serve({ fetch: app.fetch, port: 3000 })` via
`@hono/node-server`, or a hand-rolled `node:http` adapter that drives
`app.fetch` — currently fails to link because the Web Fetch FFIs
(`Headers` / `Response` constructors) aren't pulled in alongside
perry-ext-http-server. Tracked at [issue #589](https://github.com/PerryTS/perry/issues/589).

Workaround until #589 lands: deploy as an edge-runtime worker (CF
Workers / Vercel Edge), or use [perry's Fastify binding](#fastify-server)
with a single catch-all route delegating to `app.fetch`.

## Fastify Server

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:fastify-server}}
```

Perry's Fastify implementation is API-compatible with the npm package. Routes, request/reply objects, params, query strings, and JSON body parsing all work.

## Fetch API

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:fetch-api}}
```

## Axios

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:axios-client}}
```

## WebSocket

```typescript
{{#include ../../examples/stdlib/http/snippets.ts:websocket-client}}
```

## Next Steps

- [Databases](database.md)
- [Overview](overview.md) — All stdlib modules
