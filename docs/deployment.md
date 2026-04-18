# Sentinel Gateway — Deployment Guide

## Prerequisites

| Component | Required | Version |
|-----------|----------|---------|
| PostgreSQL | ✅ | 16+ |
| Redis | Required if `replicas > 1` | 7+ |
| OpenSSL | For key generation | any |
| OTEL Collector | Optional (for traces) | 0.115+ |

## Quick Start (Docker Compose)

Ships with everything prewired: Postgres, Redis, migrator sidecar, keygen sidecar, backend, OTEL collector, frontend.

```bash
git clone <repo>
cd sentinel-gateway
docker compose up -d
```

Then:
- Gateway API → `http://localhost:8080/api/v1/*`
- Frontend → `http://localhost:3005`
- Healthz → `http://localhost:8080/healthz`
- Metrics → `http://localhost:8080/metrics`

Create the first admin:
```bash
docker compose exec backend gateway-server create-admin \
    --email admin@localhost --password 'changeme' --tenant-slug local
```

## Docker (standalone)

The [backend/Dockerfile](../backend/Dockerfile) uses `cargo-chef` for dependency caching. Four stages: chef → planner → builder → slim runtime with non-root user.

```bash
# Build
docker build -t sentinel-gateway:latest ./backend

# Generate keys
mkdir keys
openssl genrsa -out keys/private.pem 2048
openssl rsa -in keys/private.pem -pubout -out keys/public.pem

# Run
docker run -d \
    -p 8080:8080 \
    -v $(pwd)/keys:/app/keys:ro \
    -e GATEWAY__DATABASE__URL=postgres://user:pass@db/app \
    -e GATEWAY__REDIS__URL=redis://redis:6379 \
    -e GATEWAY__SERVER__REQUIRE_TLS=true \
    -e GATEWAY__SERVER__ENCRYPTION_KEY=$(openssl rand -hex 32) \
    sentinel-gateway:latest
```

Image healthcheck hits `/healthz` every 10 s.

## Kubernetes (Helm)

The Helm chart is at [`deploy/helm/sentinel-gateway/`](../deploy/helm/sentinel-gateway/). Templates include Deployment, Service, Ingress, ConfigMap, Secret, HPA, PDB, NetworkPolicy, ServiceAccount + Role/RoleBinding, and frontend Deployment. PostgreSQL and Redis are subcharts from Bitnami.

```bash
helm dependency update ./deploy/helm/sentinel-gateway
helm install gateway ./deploy/helm/sentinel-gateway \
    --namespace sentinel \
    --create-namespace \
    --values production-values.yaml
```

### Minimum `production-values.yaml`

```yaml
replicaCount: 3

image:
  repository: your-registry/sentinel-gateway
  tag: 1.0.0

ingress:
  enabled: true
  className: nginx
  hosts:
    - host: gateway.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: gateway-tls
      hosts: [gateway.example.com]

config:
  server:
    deploymentMode: platform
    requireTls: true
    corsAllowAll: false
    corsOrigins: ["https://app.example.com"]

  database:
    autoMigrate: false        # migrator Job handles it
    enableQueryStats: true

  proxy:
    lbStrategy: inference_aware
    poolMaxIdlePerHost: 512

  telemetry:
    otlpEndpoint: http://otel-collector.observability:4317
    logLevel: "warn,gateway_server=info"

secrets:
  # openssl rand -hex 32
  encryptionKey: "..."
  # license key from your licencia platform
  licenseKey: "..."

postgresql:
  enabled: true
  auth:
    username: sentinel
    database: sentinel_gateway
    # Do NOT set password here — use existingSecret
    existingSecret: sentinel-pg-secret

redis:
  enabled: true
  auth:
    existingSecret: sentinel-redis-secret

resources:
  limits:
    cpu: 2000m
    memory: 2Gi
  requests:
    cpu: 500m
    memory: 512Mi

autoscaling:
  enabled: true
  minReplicas: 3
  maxReplicas: 20
  targetCPUUtilizationPercentage: 60

# Env-only variable not in TOML
env:
  - name: GATEWAY__REPLICAS
    value: "3"            # must match replicaCount
```

### Liveness + Readiness Probes

The chart configures:
- `livenessProbe` → `GET /healthz` — restarts the pod if process is wedged
- `readinessProbe` → `GET /readyz` — removes from load-balancer if DB or Redis is unreachable

Do not combine these into one endpoint — they serve different Kubernetes behaviors.

### Migration Strategy

`auto_migrate=false` in production. The chart ships a pre-install/pre-upgrade Kubernetes Job that runs `gateway-server migrate` before pods roll. This avoids the race where concurrent replica starts both try to migrate.

### Scale-out Checklist

Before setting `replicaCount > 1`:
- ☐ Redis is available and `redis.enabled=true`
- ☐ `GATEWAY__REPLICAS` env matches `replicaCount` (so the server fails fast if misconfigured)
- ☐ `autoMigrate=false` (use the migrator Job)
- ☐ PgBouncer recommended at `max_connections * replicaCount > 150`
- ☐ PodDisruptionBudget set to prevent all replicas from going down during upgrades

## Bare Metal / VM

```bash
# Build
cd backend
cargo build --release
install -m 0755 target/release/gateway-server /usr/local/bin/

# systemd unit at /etc/systemd/system/sentinel-gateway.service:
[Unit]
Description=Sentinel Gateway
After=network.target postgresql.service

[Service]
Type=simple
User=sentinel
EnvironmentFile=/etc/sentinel-gateway/env
ExecStart=/usr/local/bin/gateway-server serve
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
systemctl daemon-reload
systemctl enable --now sentinel-gateway
```

## CLI Reference

```
gateway-server serve                  Start the server (reads config + env)
gateway-server migrate                Run DB migrations only (for a migrator job)
gateway-server create-admin           Create a SuperAdmin user
    --email <email>
    --password <password>
    --tenant-slug <slug>    (default: "default")

gateway-server validate-license       Offline + online license check
    --key <license-key>

gateway-server generate-keys          Generate RSA 2048 JWT signing keys
    --output-dir <dir>      (default: "keys")
```

## Observability Setup

### Prometheus
The Prometheus scrape config and 10 alert rules are at [deploy/prometheus/](../deploy/prometheus/):
- `prometheus.yml` — scrape config for Docker + Kubernetes SD
- `alerts.yml` — SLO alerts (error rate, p95/p99 latency, backend health, budget)

```bash
# Docker
docker run -v $(pwd)/deploy/prometheus:/etc/prometheus prom/prometheus

# Kubernetes
kubectl create configmap prometheus-rules --from-file=deploy/prometheus/alerts.yml
```

### Grafana
Import [deploy/grafana/sentinel-gateway-dashboard.json](../deploy/grafana/sentinel-gateway-dashboard.json) — 10-panel dashboard with request rate, error percentage, latency p50/p95/p99, status codes, proxy latency per backend, token usage, cost/hour, rate limit rejections, and error budget burn rate.

### OTEL Collector
[deploy/docker/otel-collector.yaml](../deploy/docker/otel-collector.yaml) is preconfigured. Set `GATEWAY__TELEMETRY__OTLP_ENDPOINT=http://otel-collector:4317` to enable.

## Post-Deploy Verification

```bash
# 1. Healthz + readyz
curl -f https://gateway.example.com/healthz | jq .
curl -f https://gateway.example.com/readyz | jq .

# 2. Metrics endpoint
curl -s https://gateway.example.com/metrics | head -20

# 3. Login and call a protected endpoint
TOKEN=$(curl -s https://gateway.example.com/api/v1/auth/login \
    -H "Content-Type: application/json" \
    -d '{"tenant_slug":"default","email":"admin@localhost","password":"..."}' \
    | jq -r .access_token)

curl -s https://gateway.example.com/api/v1/users \
    -H "Authorization: Bearer $TOKEN" | jq .

# 4. Verify rate limiter headers
curl -v https://gateway.example.com/api/v1/auth/login \
    -d '{"tenant_slug":"default","email":"wrong","password":"wrong"}' 2>&1 \
    | grep -Ei "x-ratelimit|retry-after"
```

## Troubleshooting

See [docs/operations.md](operations.md) for runbooks.
