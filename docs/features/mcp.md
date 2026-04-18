# MCP Gateway

Sentinel Gateway is a **dual-role Model Context Protocol (MCP) proxy**. It:

- **Acts as an MCP server** to downstream AI agents (Claude Desktop, custom agents) — they talk to one endpoint and see a unified toolset.
- **Acts as an MCP client** to upstream MCP servers (GitHub, Slack, DBs, whatever) — the gateway aggregates their tools, resources, and prompts.

Tools are namespaced by backend name: `github__create_issue`, `slack__send_message`, `postgres__query`. No collisions, no confusion.

## Protocol

Implements MCP **2025-06-18** over **Streamable HTTP transport**. JSON-RPC 2.0 with session IDs via the `Mcp-Session-Id` header.

Supported methods:

| Method | Behavior in gateway |
|---|---|
| `initialize` | Creates a session, returns gateway's server info + aggregated capabilities |
| `tools/list` | Returns all tools across all healthy backends, namespaced |
| `tools/call` | Resolves the namespace, proxies the call to the right backend |
| `resources/list` | All resources, URIs namespaced (`mcp://backend/original_uri`) |
| `resources/read` | Proxied to the owning backend |
| `prompts/list` / `prompts/get` | Proxied to the owning backend |
| `ping` | Responds `{}` immediately |
| `notifications/*` | Accepts silently (notifications don't return a response) |

Supported on upstream clients: full client-side of the above, plus `sampling/createMessage` request/response passthrough (server-to-client).

---

## REST API (gateway-side management)

All endpoints are under `/api/v1/mcp/*` and require admin auth.

### Register an upstream MCP server

```bash
curl -X POST /api/v1/mcp/servers \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "github",
    "url": "http://localhost:3001/mcp"
  }'
```

The gateway immediately:
1. Opens an HTTP connection to the upstream.
2. Sends `initialize`.
3. Discovers tools (`tools/list`), resources (`resources/list`), prompts (`prompts/list`).
4. Records counts + health status, attaches client to the registry.

Response:
```json
{
  "id": "9f...uuid",
  "name": "github",
  "url": "http://localhost:3001/mcp",
  "tools_count": 12,
  "resources_count": 3,
  "prompts_count": 0
}
```

If the upstream is unreachable or returns non-200, you get `502 Bad Gateway` with the reason. Registration is atomic — a failed upstream doesn't leak a zombie client.

### List configured servers

```bash
curl /api/v1/mcp/servers -H "Authorization: Bearer $TOKEN"
```

### Refresh discovery (re-poll tools/resources/prompts)

```bash
curl -X POST /api/v1/mcp/servers/$ID/refresh -H "Authorization: Bearer $TOKEN"
```

Useful when an upstream MCP server adds or removes tools at runtime. The MCP protocol has `tools/list_changed` notifications — we support those too, but manual refresh is the reliable path.

### Remove a server

```bash
curl -X DELETE /api/v1/mcp/servers/$ID -H "Authorization: Bearer $TOKEN"
```

Disconnects the client, removes from the registry, strips its tools from future `tools/list` responses. Active tool calls in flight complete; new ones fail with "Backend not connected".

### List aggregated tools

```bash
curl /api/v1/mcp/tools -H "Authorization: Bearer $TOKEN"
```

Returns every tool across all healthy backends, already namespaced. This is what AI agents see when they call `tools/list`.

---

## MCP endpoint (agent-facing)

```
POST /api/v1/mcp
```

Takes a JSON-RPC 2.0 request, returns a JSON-RPC 2.0 response. Session ID returned in the response header after the first `initialize` call; clients send it back on subsequent calls.

Example flow from an AI agent's perspective:

```bash
# 1. Initialize — server responds with capabilities + Mcp-Session-Id header
curl -X POST /api/v1/mcp \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "initialize",
    "params": {
      "protocolVersion": "2025-06-18",
      "capabilities": {"sampling": {}},
      "clientInfo": {"name": "my-agent", "version": "1.0"}
    }
  }'

# Response headers include: Mcp-Session-Id: 7c8...
# Response body:
# {
#   "jsonrpc": "2.0",
#   "id": 1,
#   "result": {
#     "protocolVersion": "2025-06-18",
#     "capabilities": { "tools": {"listChanged": true}, ... },
#     "serverInfo": { "name": "sentinel-gateway-mcp", "version": "0.1.0" },
#     "instructions": "Tools are namespaced by backend: {backend}__{tool_name}..."
#   }
# }

# 2. List tools
curl -X POST /api/v1/mcp \
  -H "Authorization: Bearer $API_KEY" \
  -H "Mcp-Session-Id: 7c8..." \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'

# 3. Call a namespaced tool
curl -X POST /api/v1/mcp \
  -H "Authorization: Bearer $API_KEY" \
  -H "Mcp-Session-Id: 7c8..." \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
      "name": "github__create_issue",
      "arguments": {"repo": "acme/gateway", "title": "Bug", "body": "..."}
    }
  }'
```

The gateway routes `github__create_issue` to the GitHub MCP server, strips the namespace before forwarding (`create_issue`), then relays the response back.

---

## Namespacing details

The registry uses `__` (double underscore) as the separator. It's unlikely to appear in a real MCP tool name, and it reads cleanly:

- Tool `create_issue` on server named `github` → exposed as `github__create_issue`
- Resource `file:///path/to/doc` on server named `docs` → exposed as `mcp://docs/file:///path/to/doc`

If you have two upstreams exposing a tool called `search`, they become `notion__search` and `confluence__search`. Agents see both, no collision.

---

## Frontend UI

**LLM Proxy → MCP Servers** provides:

- **Servers tab** — card per registered server with tool/resource/prompt counts, connected/disconnected badge, actions to refresh discovery or disconnect.
- **Aggregated Tools tab** — table of every tool available through the gateway, with namespace badge, description, and parameter count.
- **Connect dialog** — name + URL inputs with URL validation.
- **Remove confirmation** — warns that tools from this server will become unavailable.

---

## Session management

- **Created on `initialize`.** Sessions are stored in `SessionStore` (in-memory DashMap) with client info, negotiated capabilities, tenant ID, user ID, timestamps.
- **Touched on every request** — `last_activity` bumps.
- **Expire after 1 hour of inactivity** (configurable via `SessionStore::new(ttl_secs)`).
- **Not persisted across restarts.** Clients transparently re-initialize. If you need persistent sessions, stick Redis behind the store.

---

## Integration with existing auth

The MCP endpoint uses the same auth middleware as everything else. Agents authenticate with a **Bearer JWT** or a gateway **API key** (`sg_...`). Every tool call is audited (who called what, when, with what args, at what cost).

This is the core value prop vs. running individual MCP servers: one auth layer, one audit trail, one place to set rate limits.

---

## Patterns

### Aggregating a toolkit

```bash
# Register 4 upstreams; agents see one combined toolset
curl -X POST /api/v1/mcp/servers -d '{"name":"github","url":"http://gh:3001/mcp"}'
curl -X POST /api/v1/mcp/servers -d '{"name":"slack","url":"http://sl:3002/mcp"}'
curl -X POST /api/v1/mcp/servers -d '{"name":"db","url":"http://pg:3003/mcp"}'
curl -X POST /api/v1/mcp/servers -d '{"name":"linear","url":"http://lin:3004/mcp"}'
```

Agent sees: `github__create_issue`, `slack__send_message`, `db__query`, `linear__create_ticket`, ...

### Rate limit per tool (future)

Combine with the rate limiter to put different budgets on different tools. Until first-class support lands, enforce at the MCP backend or via an API-key scope.

### Per-tenant isolation

MCP servers are registered **per tenant**. Tenant A's MCP tools are not visible to tenant B. This enables SaaS deployments where each customer brings their own tool integrations.

---

## Caveats

- **Session state is in-memory.** Not ideal for horizontal scale across multiple replicas. Future work: Redis-backed session store.
- **No stdio transport.** This is a networked gateway — we only support Streamable HTTP. Run stdio-based MCP servers behind a sidecar wrapper that exposes HTTP.
- **No sampling API implementation yet.** Server-to-client `sampling/createMessage` is parsed but the gateway doesn't route it back — would require long-lived SSE connections, which is future work.
- **Discovery is cached on register/refresh.** If an upstream is hot-swapping tools without notifications, you'll need to hit `/refresh`.

---

## See also

- [MCP specification](https://modelcontextprotocol.io/specification/2025-06-18)
- [Architecture](../architecture.md) — see the dual-role diagram
