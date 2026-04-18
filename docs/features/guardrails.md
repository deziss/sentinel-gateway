# Guardrails

A pluggable pipeline of policy checks applied to LLM requests and responses. Blocks jailbreak attempts, redacts PII, validates output schemas, and enforces length limits — all configurable per tenant without code changes.

## Design

- **Trait-based.** Every guardrail implements `gateway_policy::Guardrail { check(&self, ctx) -> GuardrailOutcome }`.
- **Stored per tenant.** Rules live in the `guardrail_rules` table; each row becomes a runtime guardrail at request time.
- **Pipeline stages.** A rule runs at `pre_call` (before LLM), `post_call` (after LLM), or `logging_only` (never blocks, just flags).
- **Chainable modifications.** A `redact` outcome passes modified content to the next rule in the stage — so you can chain "redact emails → redact phone numbers → enforce length".
- **Fail-open extensions.** External guardrails (e.g. an HTTP call to Azure Content Safety) that error out are logged and treated as pass — never block traffic due to an observability outage.

---

## Built-in guardrails

| Kind | What it does | Config JSON example |
|---|---|---|
| `regex` | Match one or more regex patterns | `{"patterns": ["\\bsecret_\\w+\\b", "(?i)password\\s*[:=]"]}` |
| `pii` | Predefined patterns for common PII types | `{"types": ["email", "phone", "ssn", "credit_card", "ipv4", "aws_key"]}` |
| `length` | Enforce max content length | `{"max_chars": 100000}` |
| `json_schema` | Validate post-call output is JSON matching a schema | `{"schema": {"type": "object", "required": ["answer"]}}` |

PII patterns expand at build time (see `guardrails_build.rs`):

| Type | Pattern |
|---|---|
| `email` | `\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b` |
| `phone` | `\b(?:\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}\b` |
| `ssn` | `\b\d{3}-\d{2}-\d{4}\b` |
| `credit_card` | `\b(?:\d[ -]*?){13,19}\b` |
| `ipv4` | `\b(?:\d{1,3}\.){3}\d{1,3}\b` |
| `aws_key` | `\bAKIA[0-9A-Z]{16}\b` |

---

## Modes

Every rule has a `mode` that controls what happens on match:

| Mode | Behavior |
|---|---|
| `block` | Return HTTP 400 with reason + category. Pipeline stops. |
| `redact` | Replace matched substrings with `[REDACTED]`. Content is passed to the next rule modified. |
| `flag` | Log a warning, allow through. Use for observing before enforcing. |

---

## REST API

Base path: `/api/v1/guardrails`

### Create a rule

```bash
curl -X POST /api/v1/guardrails \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "block-ssn",
    "kind": "regex",
    "stage": "pre_call",
    "mode": "block",
    "category": "pii",
    "priority": 50,
    "config": {"patterns": ["\\b\\d{3}-\\d{2}-\\d{4}\\b"]}
  }'
```

Valid values:
- `kind`: `regex` | `pii` | `length` | `json_schema`
- `stage`: `pre_call` | `post_call` | `logging_only`
- `mode`: `block` | `redact` | `flag`
- `priority`: 0-10000 (lower runs first)

### List rules

```bash
curl /api/v1/guardrails -H "Authorization: Bearer $TOKEN"
```

### Update a rule (including toggle active)

```bash
curl -X PUT /api/v1/guardrails/$ID \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"is_active": false}'
```

### Delete a rule

```bash
curl -X DELETE /api/v1/guardrails/$ID -H "Authorization: Bearer $TOKEN"
```

### Test the pipeline

Evaluate arbitrary content against every active rule without making an LLM call:

```bash
curl -X POST /api/v1/guardrails/test \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"content": "Hi, my SSN is 123-45-6789 and email is a@b.com"}'
```

Response:
```json
{
  "input": "Hi, my SSN is 123-45-6789 and email is a@b.com",
  "final_content": "Hi, my SSN is 123-45-6789 and email is [REDACTED]",
  "blocked": false,
  "results": [
    {"name": "redact-email", "outcome": "modify", "duration_ms": 0},
    {"name": "block-ssn", "outcome": "pass", "duration_ms": 0}
  ]
}
```

---

## Runtime pipeline

At request time the gateway:

1. Loads all active rules for the tenant via `GuardrailRuleRepository::list_active()` (indexed, fast).
2. Sorts by `priority` ascending.
3. For each `pre_call` rule in order:
   - Runs the guardrail's `check()`.
   - If `block` → return 400, stop.
   - If `modify` → update the current content and feed it to the next rule.
   - If `pass` / `flag` → continue.
4. Sends the (possibly modified) content to the LLM provider.
5. Repeats step 3 with `post_call` rules on the response.
6. Returns to the client.

Total overhead per rule: sub-millisecond for regex/length/json_schema. External HTTP guardrails (e.g. Azure Content Safety) add network latency — use carefully.

---

## Frontend UI

**LLM Proxy → Guardrails** has:

- Rule table with inline active/inactive toggle (switch component).
- "Test Pipeline" dialog with arbitrary input → real-time per-rule outcome trace.
- Create-rule dialog with per-kind config hints and JSON editor.
- Delete confirmation.

---

## Patterns

### Redact PII before it leaves the gateway

```json
{
  "name": "redact-all-pii",
  "kind": "pii",
  "stage": "pre_call",
  "mode": "redact",
  "priority": 10,
  "config": {"types": ["email", "phone", "ssn", "credit_card", "aws_key"]}
}
```

Priority 10 runs early so subsequent guardrails see redacted text.

### Detect jailbreaks (block)

```json
{
  "name": "block-jailbreak",
  "kind": "regex",
  "stage": "pre_call",
  "mode": "block",
  "priority": 5,
  "config": {
    "patterns": [
      "(?i)ignore\\s+(?:all\\s+)?previous\\s+instructions",
      "(?i)you\\s+are\\s+now\\s+(?:DAN|jailbroken|unfiltered)"
    ]
  }
}
```

For semantic detection (harder to evade), use the [Semantic Policy Engine](semantic-policies.md).

### Enforce JSON output schema

```json
{
  "name": "valid-json-answer",
  "kind": "json_schema",
  "stage": "post_call",
  "mode": "block",
  "priority": 100,
  "config": {"schema": {"type": "object", "required": ["answer", "confidence"]}}
}
```

If the model returns malformed JSON or misses a required field, client gets 400.

### Progressive rollout with `logging_only`

Start new rules in `logging_only` to see how often they would have fired without actually enforcing. Promote to `flag` for alerting, then `block` when confident.

---

## Extending

To add a new guardrail kind (e.g., Azure Content Safety):

1. Create a struct implementing `Guardrail` in `gateway-policy/src/`.
2. Register the kind string in `guardrails_build::build_pipeline()`.
3. Add the kind to `VALID_KINDS` in `handlers/guardrails.rs`.
4. Document the expected `config` JSON shape in this file.

The trait is intentionally small:

```rust
#[async_trait]
pub trait Guardrail: Send + Sync {
    fn name(&self) -> &str;
    fn stage(&self) -> GuardrailStage;
    async fn check(&self, ctx: &GuardrailContext<'_>) -> GuardrailOutcome;
}
```

---

## Caveats

- **Rules are loaded per request today.** If rule counts get large (>100) consider adding an in-process cache keyed by tenant with a bump-on-mutation.
- **`json_schema` is a minimal subset** — type check + required fields only. For full JSON Schema support use `jsonschema` crate (add it behind a feature flag if needed).
- **Regex DoS** — the `regex` crate is linear-time by design (no backtracking catastrophic blowups like PCRE). Still: keep patterns simple.
- **External provider errors fail open.** The gateway never blocks traffic because your Azure Content Safety quota ran out.

---

## See also

- [Semantic Policy Engine](semantic-policies.md) — for meaning-based (not keyword-based) policies
- [Rate Limiting](rate-limiting.md) — complementary abuse prevention
