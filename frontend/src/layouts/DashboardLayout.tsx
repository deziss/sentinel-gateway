import { Outlet, NavLink } from "react-router-dom"
import {
  LayoutDashboard,
  Server,
  Key,
  Users,
  Route,
  ScrollText,
  Settings,
  Sparkles,
  BarChart2,
  Library,
  Plug,
  FileText,
  Shield,
  BookOpen,
  MessageSquare,
  KeyRound,
  Building2,
  CreditCard,
} from "lucide-react"
import type { FeatureFlags } from "@/lib/api"
import { cn } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { useAuth, isRole, type Role } from "@/lib/auth"
import { UserMenu } from "@/components/UserMenu"
import { PlanBadge } from "@/components/PlanBadge"
import { usePlan } from "@/hooks/use-plan"

type NavItem = {
  to: string
  label: string
  icon: React.ComponentType<{ className?: string }>
  /** Feature flag to check — if undefined, always visible. */
  feature?: keyof FeatureFlags
  /** Minimum role to see this item — if undefined, any authenticated user. */
  minRole?: Role
}

const navItems: NavItem[] = [
  { to: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
  { to: "/backends", label: "Backends", icon: Server, minRole: "tenant_admin" },
  { to: "/routes", label: "Routes", icon: Route, minRole: "tenant_admin" },
  { to: "/api-keys", label: "API Keys", icon: Key, minRole: "tenant_admin" },
  { to: "/users", label: "Users", icon: Users, minRole: "tenant_admin" },
  { to: "/audit", label: "Audit Logs", icon: ScrollText, minRole: "tenant_admin" },
  { to: "/settings", label: "Settings", icon: Settings, minRole: "tenant_admin" },
]

const llmItems: NavItem[] = [
  { to: "/llm-playground", label: "Playground", icon: Sparkles, feature: "playground_enabled" },
  { to: "/llm-analytics", label: "LLM Analytics", icon: BarChart2, feature: "logs_enabled" },
  { to: "/llm-catalog", label: "Model Catalog", icon: Library },
  { to: "/mcp-servers", label: "MCP Servers", icon: Plug },
  { to: "/prompts", label: "Prompts", icon: FileText, feature: "prompt_templates_enabled" },
  { to: "/guardrails", label: "Guardrails", icon: Shield, feature: "deterministic_guardrails" },
  { to: "/feedback", label: "Feedback", icon: MessageSquare, feature: "feedback_enabled" },
]

const adminItems: NavItem[] = [
  { to: "/sso-providers", label: "SSO Providers", icon: KeyRound, feature: "sso_enabled", minRole: "tenant_admin" },
  { to: "/organizations", label: "Organizations", icon: Building2, feature: "org_management_enabled", minRole: "super_admin" },
  { to: "/billing", label: "Billing & Plans", icon: CreditCard, minRole: "super_admin" },
]

export function DashboardLayout() {
  const tenantId = useAuth((s) => s.tenantId)
  const role = useAuth((s) => s.user?.role)
  const { has, isLoading: planLoading } = usePlan()

  const visibleByRole = (i: NavItem) => !i.minRole || isRole(role, i.minRole)
  const visibleByFeature = (i: NavItem) => !i.feature || planLoading || has(i.feature)
  const isVisible = (i: NavItem) => visibleByRole(i) && visibleByFeature(i)

  const visibleNavItems = navItems.filter(isVisible)
  const visibleLlmItems = llmItems.filter(isVisible)
  const visibleAdminItems = adminItems.filter(isVisible)

  return (
    <div className="flex min-h-screen bg-muted/20">
      {/* Sidebar */}
      <aside className="w-64 flex-col border-r bg-background">
        <div className="flex h-16 items-center px-6 border-b">
          <div className="flex items-center gap-2">
            <div className="size-8 rounded bg-primary text-primary-foreground flex items-center justify-center font-bold">
              SG
            </div>
            <span className="font-semibold text-lg tracking-tight">
              Sentinel
            </span>
          </div>
        </div>
        <nav className="flex-1 space-y-1 p-4">
          <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2 px-3">
            Gateway
          </div>
          {visibleNavItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 px-3 py-2 rounded-md text-sm font-medium transition-colors",
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground"
                )
              }
            >
              <item.icon className="size-4" />
              {item.label}
            </NavLink>
          ))}
          <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mt-6 mb-2 px-3">
            LLM Proxy
          </div>
          {visibleLlmItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 px-3 py-2 rounded-md text-sm font-medium transition-colors",
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground"
                )
              }
            >
              <item.icon className="size-4" />
              {item.label}
            </NavLink>
          ))}
          {visibleAdminItems.length > 0 && (
            <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mt-6 mb-2 px-3">
              Administration
            </div>
          )}
          {visibleAdminItems.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 px-3 py-2 rounded-md text-sm font-medium transition-colors",
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground"
                )
              }
            >
              <item.icon className="size-4" />
              {item.label}
            </NavLink>
          ))}
          <div className="pt-6 mt-6 border-t">
            <NavLink
              to="/docs"
              className={({ isActive }) =>
                cn(
                  "flex items-center gap-3 px-3 py-2 rounded-md text-sm font-medium transition-colors",
                  isActive
                    ? "bg-primary text-primary-foreground"
                    : "text-muted-foreground hover:bg-muted hover:text-foreground"
                )
              }
            >
              <BookOpen className="size-4" />
              Documentation
            </NavLink>
          </div>
        </nav>
      </aside>

      {/* Main Content */}
      <main className="flex-1 flex flex-col">
        <header className="h-16 border-b bg-background flex items-center px-8 justify-between">
          <div className="flex items-center gap-2">
            <Badge variant="outline" className="font-mono text-xs">
              {tenantId
                ? `Tenant: ${tenantId.slice(0, 8)}...`
                : "No Tenant"}
            </Badge>
            <PlanBadge />
          </div>
          <UserMenu />
        </header>
        <div className="flex-1 p-8 overflow-auto">
          <Outlet />
        </div>
      </main>
    </div>
  )
}
