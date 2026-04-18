# Prompt Management & Versioning

Sentinel Gateway ships a first-class prompt registry. Instead of hardcoding system prompts in your applications, store them in the gateway, version them, deploy specific versions to labels (prod/staging/canary/dev), and reference them from client requests.

## Why

- **Deploy prompts without redeploying code.** A bad system prompt can now be reverted in seconds.
- **A/B test prompts** — run two versions under the `prod` and `canary` labels, split traffic at the client.
- **Centralized audit** — every prompt change is recorded in the audit log.
- **Variable rendering** — parameterize prompts with `{{var_name}}` placeholders; the gateway substitutes them server-side.

---

## Data model

Two tables (`migrations/015_prompts.sql`):

**`prompts`** — each row is one version.
| Column | Purpose |
|---|---|
| `id` | Primary key |
| `tenant_id` | Tenant scoping (cascade delete) |
| `name` | Prompt name (unique per tenant+version) |
| `version` | Auto-incremented integer |
| `content` | The prompt text, including `{{placeholders}}` |
| `variables` | JSONB schema describing expected variables |
| `model_prefs` | JSONB defaults: `{"temperature": 0.7, "max_tokens": 2048}` |
| `default_model` | Optional model hint applied when client omits `model` |
| `metadata` | JSONB tags / description / author |
| `created_by` | User who created this version |

**`prompt_deployments`** — label → version mapping. One row per `(tenant_id, name, label)`.
| Column | Purpose |
|---|---|
| `label` | e.g. `prod`, `staging`, `canary`, `dev`, or any custom string |
| `version` | Which version this label points to |
| `deployed_by` | User who made the deployment |

---

## REST API

Base path: `/api/v1/prompts`

### Create (new version)

```bash
curl -X POST /api/v1/prompts \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "customer_support",
    "content": "You are a support agent for {{brand}}. Be concise and cite policy references.",
    "default_model": "gpt-4o",
    "model_prefs": {"temperature": 0.3, "max_tokens": 1024},
    "variables": {
      "brand": {"type": "string", "required": true}
    }
  }'
```

Each POST creates a **new version** — the gateway auto-increments.

### List versions

```bash
curl /api/v1/prompts/customer_support/versions -H "Authorization: Bearer $TOKEN"
```

### Deploy a version to a label

```bash
curl -X POST /api/v1/prompts/customer_support/deploy \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"label": "prod", "version": 3}'
```

### List deployments

```bash
curl /api/v1/prompts/customer_support/deployments -H "Authorization: Bearer $TOKEN"
```

Returns all labels pointing at any version of this prompt, so you can see prod=v3, staging=v4, canary=v4 at a glance.

### Resolve (test rendering)

```bash
curl -X POST /api/v1/prompts/customer_support/resolve \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "label": "prod",
    "variables": {"brand": "Acme"}
  }'
```

Returns the rendered content, version, `default_model`, and `model_prefs`. Useful for debugging without actually making an LLM call.

### Delete a specific version

```bash
curl -X DELETE /api/v1/prompts/customer_support/versions/2 \
  -H "Authorization: Bearer $TOKEN"
```

Note: deleting a version does **not** cascade to deployments. Delete or re-point the deployment first.

---

## Using prompts in chat completions

Once deployed, reference the prompt from any `/v1/chat/completions` call by adding a `prompt_ref` object:

```json
POST /v1/chat/completions
{
  "prompt_ref": {
    "name": "customer_support",
    "label": "prod",
    "variables": {"brand": "Acme"}
  },
  "messages": [
    {"role": "user", "content": "What's your return policy?"}
  ]
}
```

On resolution the gateway:
1. Loads the version deployed to `prod` (falls back to latest if no deployment for that label).
2. Renders the content by substituting `{{brand}}` → `Acme`.
3. Injects the rendered text as a **system message** at position 0 (replaces an existing system message, or prepends if none).
4. Applies `default_model` if the client didn't set `model`.
5. Applies each `model_prefs` key via `.entry().or_insert()` — client values always win.
6. Strips `prompt_ref` from the forwarded body.
7. Forwards to the LLM provider normally.

If the prompt doesn't exist, the gateway returns `404` with `error.type = prompt_not_found` before making any upstream call.

---

## Frontend UI

Navigate to **LLM Proxy → Prompts** in the admin UI.

- **Sidebar** — list of prompt names, click to drill in.
- **Versions tab** — history with badges showing which labels currently point at each version.
- **Deployments tab** — card per label with version number and deploy timestamp.
- **Actions per version** — view content, test render with custom variables, deploy to a label, delete (only if not deployed).

The test-render dialog lets you provide arbitrary variable JSON and see the output before making a real LLM call.

---

## Patterns

### Canary deploy

```bash
# Version 5 is the new prompt, version 4 is the baseline
curl -X POST /api/v1/prompts/cs/deploy -d '{"label":"prod",   "version":4}'   # keep prod on 4
curl -X POST /api/v1/prompts/cs/deploy -d '{"label":"canary", "version":5}'   # route 5% of traffic to canary in your app
```

### Instant rollback

```bash
# Oh no, v5 is worse — flip back
curl -X POST /api/v1/prompts/cs/deploy -d '{"label":"prod", "version":4}'
```

No code deploy. No restart. Change is audited.

### A/B testing a model alongside a prompt

Combine with fallback chains or multiple backends:

```json
{
  "prompt_ref": { "name": "cs", "label": "prod" },
  "model": "gpt-4o-or-claude-sonnet-4.5",  // your alias
  "messages": [...]
}
```

The alias resolves to different providers; the prompt stays consistent so your comparison is apples-to-apples.

---

## Caveats

- **Simple template engine** — only `{{var_name}}` substitution. No loops, conditionals, or filters. If you need Handlebars-level power, render client-side and pass the full text as `content`.
- **Undefined variables are preserved** — `{{unknown}}` stays literal in the output (doesn't error). This is intentional so a missing variable doesn't break prod at runtime, but validate your variable set before deploying.
- **No cross-tenant sharing** — prompts are strictly per-tenant. A central "prompt library" is out of scope for v1.
- **Label names are free-form** — nothing enforces that `prod` must exist. Use a naming convention your team sticks to.
- **Deleting a deployed version** fails today with no cascade. Unmap the label first.

---

## See also

- [Guardrails](guardrails.md) — add safety checks to rendered prompts
- [Observability](observability.md) — trace which prompt version produced each response
