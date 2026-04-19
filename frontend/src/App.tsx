import { Routes, Route, BrowserRouter, Navigate } from "react-router-dom"
import { DashboardLayout } from "./layouts/DashboardLayout"
import { Dashboard } from "./pages/Dashboard"
import { Backends } from "./pages/Backends"
import { ApiKeys } from "./pages/ApiKeys"
import { Users } from "./pages/Users"
import { ProxyRoutes } from "./pages/Routes"
import { AuditLogs } from "./pages/AuditLogs"
import { Settings } from "./pages/Settings"
import { LlmPlayground } from "./pages/LlmPlayground"
import { LlmAnalytics } from "./pages/LlmAnalytics"
import { LlmCatalog } from "./pages/LlmCatalog"
import { McpServers } from "./pages/McpServers"
import { Prompts } from "./pages/Prompts"
import { Guardrails } from "./pages/Guardrails"
import { Feedback } from "./pages/Feedback"
import { SsoProviders } from "./pages/SsoProviders"
import { Organizations } from "./pages/Organizations"
import { Billing } from "./pages/Billing"
import { Login } from "./pages/Login"
import { DocsLayout } from "./layouts/DocsLayout"
import { DocsIndex } from "./pages/docs/DocsIndex"
import { UserGuide } from "./pages/docs/UserGuide"
import { TenantGuide } from "./pages/docs/TenantGuide"
import { DeveloperGuide } from "./pages/docs/DeveloperGuide"
import { ApiReference } from "./pages/docs/ApiReference"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { useAuth, isRole, type Role } from "./lib/auth"
import { Toaster } from "./components/ui/toaster"

const queryClient = new QueryClient();

function PrivateRoute({ children }: { children: React.ReactNode }) {
  const isAuthenticated = useAuth((s) => s.isAuthenticated)
  if (!isAuthenticated) return <Navigate to="/login" replace />
  return <>{children}</>
}

function RoleRoute({ min, children }: { min: Role; children: React.ReactNode }) {
  const role = useAuth((s) => s.user?.role)
  if (!isRole(role, min)) return <Navigate to="/dashboard" replace />
  return <>{children}</>
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route path="/login" element={<Login />} />
          <Route path="/docs" element={<DocsLayout />}>
            <Route index element={<DocsIndex />} />
            <Route path="users" element={<UserGuide />} />
            <Route path="tenants" element={<TenantGuide />} />
            <Route path="developers" element={<DeveloperGuide />} />
            <Route path="api" element={<ApiReference />} />
          </Route>
          <Route
            path="/"
            element={
              <PrivateRoute>
                <DashboardLayout />
              </PrivateRoute>
            }
          >
            <Route index element={<Navigate to="/dashboard" replace />} />
            <Route path="dashboard" element={<Dashboard />} />
            <Route path="llm-playground" element={<LlmPlayground />} />
            <Route path="llm-analytics" element={<LlmAnalytics />} />
            <Route path="llm-catalog" element={<LlmCatalog />} />
            <Route path="mcp-servers" element={<McpServers />} />
            <Route path="prompts" element={<Prompts />} />
            <Route path="guardrails" element={<Guardrails />} />
            <Route path="feedback" element={<Feedback />} />

            {/* Tenant-admin-only */}
            <Route path="backends" element={<RoleRoute min="tenant_admin"><Backends /></RoleRoute>} />
            <Route path="api-keys" element={<RoleRoute min="tenant_admin"><ApiKeys /></RoleRoute>} />
            <Route path="users" element={<RoleRoute min="tenant_admin"><Users /></RoleRoute>} />
            <Route path="routes" element={<RoleRoute min="tenant_admin"><ProxyRoutes /></RoleRoute>} />
            <Route path="audit" element={<RoleRoute min="tenant_admin"><AuditLogs /></RoleRoute>} />
            <Route path="settings" element={<RoleRoute min="tenant_admin"><Settings /></RoleRoute>} />
            <Route path="sso-providers" element={<RoleRoute min="tenant_admin"><SsoProviders /></RoleRoute>} />

            {/* Super-admin-only */}
            <Route path="organizations" element={<RoleRoute min="super_admin"><Organizations /></RoleRoute>} />
            <Route path="billing" element={<RoleRoute min="super_admin"><Billing /></RoleRoute>} />
          </Route>
        </Routes>
      </BrowserRouter>
      <Toaster />
    </QueryClientProvider>
  )
}

export default App
