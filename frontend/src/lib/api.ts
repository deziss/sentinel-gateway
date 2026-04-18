import axios, { AxiosError } from "axios";

const BASE_URL = import.meta.env.VITE_API_URL ?? "http://localhost:8080/api/v1";

const api = axios.create({
  baseURL: BASE_URL,
  headers: {
    "Content-Type": "application/json",
  },
});

export function getToken(): string | null {
  return localStorage.getItem("access_token");
}

export function getRefreshToken(): string | null {
  return localStorage.getItem("refresh_token");
}

export function getTenantId(): string | null {
  return localStorage.getItem("tenant_id");
}

export function setTokens(access: string, refresh: string) {
  localStorage.setItem("access_token", access);
  localStorage.setItem("refresh_token", refresh);
}

export function setTenantId(id: string) {
  localStorage.setItem("tenant_id", id);
}

export function clearAuth() {
  localStorage.removeItem("access_token");
  localStorage.removeItem("refresh_token");
  localStorage.removeItem("tenant_id");
}

// Request Interceptor: Inject Token and Tenant ID
api.interceptors.request.use((config) => {
  const token = getToken();
  const tenantId = getTenantId();

  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  if (tenantId) {
    config.headers["X-Tenant-ID"] = tenantId;
  }
  return config;
});

// Response Interceptor: Handle Token Refresh
api.interceptors.response.use(
  (response) => response,
  async (error: AxiosError) => {
    const originalRequest = error.config as any;

    if (error.response?.status === 401 && !originalRequest._retry) {
      originalRequest._retry = true;
      const refreshToken = getRefreshToken();

      if (refreshToken) {
        try {
          const resp = await axios.post(`${BASE_URL}/auth/refresh`, {
            refresh_token: refreshToken,
          });
          const { access_token, refresh_token } = resp.data;
          setTokens(access_token, refresh_token);
          
          originalRequest.headers.Authorization = `Bearer ${access_token}`;
          return api(originalRequest);
        } catch (refreshError) {
          clearAuth();
          window.location.href = "/login";
          return Promise.reject(refreshError);
        }
      } else {
        clearAuth();
        window.location.href = "/login";
      }
    }

    return Promise.reject(error);
  }
);

export default api;

// ── Error Sanitization ───────────────────────────────────────────────────────
export function sanitizeApiError(error: unknown): string {
  if (error instanceof AxiosError) {
    const status = error.response?.status ?? 0;
    const data = error.response?.data as Record<string, string> | undefined;

    // Use backend error message if it provides one (these are intentionally safe)
    if (data?.error) return data.error;

    switch (status) {
      case 400: return "Invalid request. Please check your input.";
      case 401: return "Authentication required. Please log in.";
      case 403: return "You don't have permission for this action.";
      case 404: return "The requested resource was not found.";
      case 409: return "A conflict occurred. The resource may already exist.";
      case 422: return "Validation failed. Please check your input.";
      case 423: return "Account temporarily locked. Try again later.";
      case 429: return "Too many requests. Please try again later.";
      default: return status >= 500
        ? "A server error occurred. Please try again."
        : "An unexpected error occurred.";
    }
  }
  return error instanceof Error ? error.message : "An unexpected error occurred.";
}

// ── Auth ──────────────────────────────────────────────────────────────────────
export interface LoginResponse {
  access_token: string;
  refresh_token: string;
  tenant_id: string;
}

export async function login(tenant_slug: string, email: string, password: string) {
  const { data } = await api.post<LoginResponse>("/auth/login", {
    tenant_slug,
    email,
    password,
  });
  setTokens(data.access_token, data.refresh_token);
  setTenantId(data.tenant_id);
  return data;
}

// ── API Keys ──────────────────────────────────────────────────────────────────
export interface ApiKeyMeta {
  id: string;
  name: string;
  scopes: string[];
  rate_limit_rpm: number | null;
  budget_daily: number | null;
  budget_monthly: number | null;
  is_active: boolean;
  expires_at: string | null;
  last_used_at: string | null;
  created_at: string;
}

export interface CreateApiKeyInput {
  name: string;
  scopes: string[];
  rate_limit_rpm?: number | null;
  budget_daily?: number | null;
  budget_monthly?: number | null;
}

export async function listApiKeys() {
  const { data } = await api.get<{ api_keys: ApiKeyMeta[] }>("/api-keys");
  return data.api_keys;
}

export async function createApiKey(input: CreateApiKeyInput) {
  const { data } = await api.post<{ key: string; metadata: ApiKeyMeta }>("/api-keys", input);
  return data;
}

export async function revokeApiKey(id: string) {
  await api.delete(`/api-keys/${id}`);
}

// ── Backends ──────────────────────────────────────────────────────────────────
export interface Backend {
  id: string;
  name: string;
  provider_type: string;
  endpoint: string;
  priority: number;
  weight: number;
  timeout_ms: number;
  max_retries: number;
  is_active: boolean;
  health_status: string;
  last_health_check: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateBackendInput {
  name: string;
  provider_type: string;
  endpoint: string;
  credentials?: string;
  priority?: number;
  weight?: number;
  timeout_ms?: number;
  max_retries?: number;
}

export async function listBackends() {
  const { data } = await api.get<{ backends: Backend[] }>("/backends");
  return data.backends;
}

export async function getBackend(id: string) {
  const { data } = await api.get<Backend>(`/backends/${id}`);
  return data;
}

export async function createBackend(input: CreateBackendInput) {
  const { data } = await api.post<Backend>("/backends", input);
  return data;
}

export async function updateBackend(id: string, input: Partial<CreateBackendInput>) {
  const { data } = await api.put<Backend>(`/backends/${id}`, input);
  return data;
}

export async function deleteBackend(id: string) {
  await api.delete(`/backends/${id}`);
}

// ── Audit Logs ────────────────────────────────────────────────────────────────
export interface AuditLog {
  id: string;
  tenant_id: string;
  user_id: string | null;
  action: string;
  resource_type: string;
  resource_id: string | null;
  details: Record<string, unknown> | null;
  ip_address: string | null;
  user_agent: string | null;
  created_at: string;
}

export interface AuditLogQuery {
  user_id?: string;
  action?: string;
  resource_type?: string;
  limit?: number;
  offset?: number;
}

export interface AuditLogResponse {
  audit_logs: AuditLog[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

export async function listAuditLogs(params: AuditLogQuery = {}) {
  const { data } = await api.get<AuditLogResponse>("/audit-logs", { params });
  return data;
}

// ── Proxy Routes ─────────────────────────────────────────────────────────────
export interface ProxyRoute {
  id: string;
  name: string;
  protocol: string;
  path_pattern: string;
  backend_id: string;
  strip_prefix: boolean;
  rewrite_rules: Record<string, string>;
  is_active: boolean;
  priority: number;
  created_at: string;
  updated_at: string;
}

export interface CreateRouteInput {
  name: string;
  protocol: string;
  path_pattern: string;
  backend_id: string;
  strip_prefix?: boolean;
  rewrite_rules?: Record<string, string>;
  priority?: number;
}

export async function listRoutes() {
  const { data } = await api.get<{ routes: ProxyRoute[] }>("/routes");
  return data.routes;
}

export async function createRoute(input: CreateRouteInput) {
  const { data } = await api.post<ProxyRoute>("/routes", input);
  return data;
}

export async function deleteRoute(id: string) {
  await api.delete(`/routes/${id}`);
}

// ── Users ────────────────────────────────────────────────────────────────────
export interface User {
  id: string;
  tenant_id: string;
  email: string;
  role: string;
  status: string;
  last_login_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface InviteUserInput {
  email: string;
  password: string;
  role?: string;
}

export interface UpdateUserInput {
  role?: string;
  status?: string;
}

export async function listUsers() {
  const { data } = await api.get<{ users: User[]; total: number }>("/users");
  return data;
}

export async function inviteUser(input: InviteUserInput) {
  const { data } = await api.post<User>("/users", input);
  return data;
}

export async function updateUser(id: string, input: UpdateUserInput) {
  const { data } = await api.put<User>(`/users/${id}`, input);
  return data;
}

export async function deactivateUser(id: string) {
  await api.delete(`/users/${id}`);
}

// ── Usage & Analytics ─────────────────────────────────────────────────────────
export interface UsageSummary {
  period: string;
  total_requests: number;
  total_tokens_input: number;
  total_tokens_output: number;
  total_tokens: number;
  total_cost_usd: number;
}

export async function getUsageSummary() {
  const { data } = await api.get<UsageSummary>("/usage");
  return data;
}

// ── Settings ─────────────────────────────────────────────────────────────────
export async function getSettings() {
  const { data } = await api.get<{ settings: Record<string, string> }>("/settings");
  return data.settings;
}

export async function updateSettings(settings: Record<string, string>) {
  const { data } = await api.put<{ settings: Record<string, string> }>("/settings", { settings });
  return data.settings;
}

export async function deleteSettingKey(key: string) {
  await api.delete(`/settings/${key}`);
}

// ── Webhooks ─────────────────────────────────────────────────────────────────
export interface Webhook {
  id: string;
  url: string;
  events: string[];
  secret?: string;
  is_active: boolean;
  last_sent_at: string | null;
  created_at: string;
}

export async function listWebhooks() {
  const { data } = await api.get<{ webhooks: Webhook[]; total: number }>("/webhooks");
  return data;
}

export async function createWebhook(url: string, events: string[]) {
  const { data } = await api.post<Webhook>("/webhooks", { url, events });
  return data;
}

export async function deleteWebhook(id: string) {
  await api.delete(`/webhooks/${id}`);
}

export async function testWebhook(id: string) {
  const { data } = await api.post<{ status: string; message: string }>(`/webhooks/${id}/test`);
  return data;
}

// ── Prompts (Management & Versioning) ───────────────────────────────────────

export interface Prompt {
  id: string;
  tenant_id: string;
  name: string;
  version: number;
  content: string;
  variables: Record<string, unknown>;
  model_prefs: Record<string, unknown>;
  default_model: string | null;
  metadata: Record<string, unknown>;
  created_by: string | null;
  created_at: string;
  updated_at: string;
}

export interface PromptDeployment {
  id: string;
  prompt_name: string;
  label: string;
  version: number;
  deployed_by: string | null;
  deployed_at: string;
}

export interface CreatePromptInput {
  name: string;
  content: string;
  variables?: Record<string, unknown>;
  model_prefs?: Record<string, unknown>;
  default_model?: string;
  metadata?: Record<string, unknown>;
}

export async function listPromptNames() {
  const { data } = await api.get<{ prompts: string[]; total: number }>("/prompts");
  return data;
}

export async function createPrompt(input: CreatePromptInput) {
  const { data } = await api.post<Prompt>("/prompts", input);
  return data;
}

export async function listPromptVersions(name: string) {
  const { data } = await api.get<{ versions: Prompt[]; total: number }>(
    `/prompts/${encodeURIComponent(name)}/versions`,
  );
  return data;
}

export async function getPromptVersion(name: string, version: number) {
  const { data } = await api.get<Prompt>(
    `/prompts/${encodeURIComponent(name)}/versions/${version}`,
  );
  return data;
}

export async function deletePromptVersion(name: string, version: number) {
  await api.delete(`/prompts/${encodeURIComponent(name)}/versions/${version}`);
}

export async function deployPrompt(name: string, label: string, version: number) {
  const { data } = await api.post<PromptDeployment>(
    `/prompts/${encodeURIComponent(name)}/deploy`,
    { label, version },
  );
  return data;
}

export async function listPromptDeployments(name: string) {
  const { data } = await api.get<{ deployments: PromptDeployment[]; total: number }>(
    `/prompts/${encodeURIComponent(name)}/deployments`,
  );
  return data;
}

export async function resolvePrompt(
  name: string,
  label: string | null,
  variables: Record<string, unknown>,
) {
  const { data } = await api.post<{
    name: string;
    version: number;
    content: string;
    model_prefs: Record<string, unknown>;
    default_model: string | null;
  }>(`/prompts/${encodeURIComponent(name)}/resolve`, {
    label,
    variables,
  });
  return data;
}

// ── Guardrails ────────────────────────────────────────────────────────────────

export type GuardrailKind = "regex" | "length" | "json_schema" | "pii";
export type GuardrailStage = "pre_call" | "post_call" | "logging_only";
export type GuardrailMode = "block" | "redact" | "flag";

export interface GuardrailRule {
  id: string;
  name: string;
  kind: GuardrailKind;
  stage: GuardrailStage;
  mode: GuardrailMode;
  category: string;
  config: Record<string, unknown>;
  priority: number;
  is_active: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateGuardrailInput {
  name: string;
  kind: GuardrailKind;
  stage: GuardrailStage;
  mode: GuardrailMode;
  category?: string;
  config?: Record<string, unknown>;
  priority?: number;
}

export async function listGuardrails() {
  const { data } = await api.get<{ rules: GuardrailRule[]; total: number }>("/guardrails");
  return data;
}

export async function createGuardrail(input: CreateGuardrailInput) {
  const { data } = await api.post<GuardrailRule>("/guardrails", input);
  return data;
}

export async function updateGuardrail(id: string, input: Partial<CreateGuardrailInput> & { is_active?: boolean }) {
  const { data } = await api.put<GuardrailRule>(`/guardrails/${id}`, input);
  return data;
}

export async function deleteGuardrail(id: string) {
  await api.delete(`/guardrails/${id}`);
}

export interface GuardrailTestResult {
  input: string;
  final_content: string;
  blocked: boolean;
  results: Array<{
    name: string;
    outcome: string;
    duration_ms: number;
  }>;
}

export async function testGuardrails(content: string) {
  const { data } = await api.post<GuardrailTestResult>("/guardrails/test", { content });
  return data;
}

// ── MCP (Model Context Protocol) ─────────────────────────────────────────────

export interface McpServerInfo {
  id: string;
  name: string;
  url: string;
  is_healthy: boolean;
  tools_count: number;
  resources_count: number;
  prompts_count: number;
}

export async function listMcpServers() {
  const { data } = await api.get<{ mcp_servers: McpServerInfo[]; total: number }>("/mcp/servers");
  return data;
}

export async function registerMcpServer(name: string, url: string) {
  const { data } = await api.post<McpServerInfo>("/mcp/servers", { name, url });
  return data;
}

export async function removeMcpServer(id: string) {
  await api.delete(`/mcp/servers/${id}`);
}

export async function refreshMcpServer(id: string) {
  const { data } = await api.post<{ status: string; tools_count: number; resources_count: number; prompts_count: number }>(`/mcp/servers/${id}/refresh`);
  return data;
}

export interface McpTool {
  name: string;
  title?: string;
  description?: string;
  inputSchema: Record<string, unknown>;
}

export async function listMcpTools() {
  const { data } = await api.get<{ tools: McpTool[]; total: number }>("/mcp/tools");
  return data;
}

// ── LLM Gateway ──────────────────────────────────────────────────────────────

function gatewayHeaders() {
  return {
    Authorization: `Bearer ${getToken()}`,
    "Content-Type": "application/json",
    "X-Tenant-ID": getTenantId() ?? "",
  }
}

function gatewayUrl(path: string) {
  return `${BASE_URL.replace("/api/v1", "")}${path}`
}

export interface LlmModel {
  id: string
  object: string
  owned_by: string
}

export async function listModels() {
  const { data } = await axios.get<{ data: LlmModel[] }>(
    gatewayUrl("/v1/models"),
    { headers: gatewayHeaders() }
  )
  return data.data
}

export interface ChatMessage {
  role: "system" | "user" | "assistant"
  content: string
}

export interface ChatCompletionOptions {
  temperature?: number
  max_tokens?: number
  top_p?: number
  stream?: boolean
}

export interface ChatCompletionResponse {
  id: string
  choices: Array<{
    index: number
    message: { role: string; content: string }
    finish_reason: string
  }>
  usage?: {
    prompt_tokens: number
    completion_tokens: number
    total_tokens: number
  }
}

export async function chatCompletion(
  model: string,
  messages: ChatMessage[],
  options: ChatCompletionOptions = {}
) {
  const { data } = await axios.post<ChatCompletionResponse>(
    gatewayUrl("/v1/chat/completions"),
    { model, messages, ...options, stream: false },
    { headers: gatewayHeaders() }
  )
  return data
}

export function streamChatCompletion(
  model: string,
  messages: ChatMessage[],
  options: Omit<ChatCompletionOptions, "stream"> = {}
): { reader: ReadableStreamDefaultReader<Uint8Array>; abort: () => void } {
  const controller = new AbortController()

  const response = fetch(gatewayUrl("/v1/chat/completions"), {
    method: "POST",
    headers: gatewayHeaders(),
    body: JSON.stringify({ model, messages, ...options, stream: true }),
    signal: controller.signal,
  })

  const reader = response.then((r) => {
    if (!r.ok) throw new Error(`HTTP ${r.status}`)
    return r.body!.getReader()
  })

  // Return a proxy reader that resolves the fetch first
  const lazyReader = {
    read: async () => {
      const r = await reader
      return r.read()
    },
    cancel: async () => {
      const r = await reader
      return r.cancel()
    },
    releaseLock: () => {},
    closed: Promise.resolve(undefined),
  } as unknown as ReadableStreamDefaultReader<Uint8Array>

  return { reader: lazyReader, abort: () => controller.abort() }
}

export interface EmbeddingResponse {
  data: Array<{
    index: number
    embedding: number[]
  }>
  usage?: {
    prompt_tokens: number
    total_tokens: number
  }
  model: string
}

export async function createEmbedding(model: string, input: string) {
  const { data } = await axios.post<EmbeddingResponse>(
    gatewayUrl("/v1/embeddings"),
    { model, input },
    { headers: gatewayHeaders() }
  )
  return data
}

// Legacy alias
export async function proxyRequest(modelId: string, messages: ChatMessage[], options: ChatCompletionOptions = {}) {
  return chatCompletion(modelId, messages, options)
}
