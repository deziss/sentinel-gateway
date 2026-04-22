# Sentinel Gateway Editions

Sentinel Gateway is available in three editions tailored for different deployment needs.

## Comparison Matrix

| Feature | Community (OSS) | PaaS (Self-Hosted) | Platform (SaaS) |
|---------|-----------------|-------------------|-----------------|
| **Availability** | Free | Licensed | Licensed / Managed |
| **Multi-tenancy** | No | Optional | Yes |
| **API Gateway** | Yes | Yes | Yes |
| **Load Balancing** | Yes | Yes | Yes |
| **Auth / RBAC** | Basic | Advanced | Advanced |
| **Observability** | Basic | Full (Enterprise) | Full (Enterprise) |
| **Usage Metering** | No | Yes | Yes (Real-time) |
| **SSO / OIDC** | No | Yes | Yes |
| **Audit Logs** | No | Yes | Yes |
| **Guardrails** | Basic | Advanced | Advanced |

## Deployment Modes

The edition is controlled by build-time feature flags and runtime configuration.

### 1. Community Edition (Local Mode)
*   **Binary**: Compiled with `#[cfg(not(feature = "saas"))]`.
*   **Config**: `DEPLOYMENT_MODE=local`.
*   **Behavior**: Single-tenant, offline, restricted to base OSS features. All licensing logic is physically stripped from the binary.

### 2. PaaS Edition (PaaS Mode)
*   **Binary**: Compiled with `#[cfg(feature = "saas")]`.
*   **Config**: `DEPLOYMENT_MODE=paas`.
*   **Behavior**: Unlocked via a `GATEWAY__SERVER__DEVELOPER_SECRET`. Used for local development of enterprise features or trusted self-hosted environments.

### 3. Platform Edition (Platform Mode)
*   **Binary**: Compiled with `#[cfg(feature = "saas")]`.
*   **Config**: `DEPLOYMENT_MODE=platform`.
*   **Behavior**: Multi-tenant, connected to the **Licencia** platform. Requires a valid per-tenant license. Features are gated by online/offline validation heartbeats.

## Upgrading

To upgrade from Community to PaaS or Platform:
1. Rebuild the Docker image with `EDITION=saas`.
2. Configure the `LICENCIA_URL` and `LICENCIA_API_KEY`.
3. Run the bootstrap script: `./scripts/licencia-bootstrap.sh`.
