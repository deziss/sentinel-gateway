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
} from "lucide-react"
import { cn } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { useAuth } from "@/lib/auth"
import { UserMenu } from "@/components/UserMenu"

const navItems = [
  { to: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
  { to: "/backends", label: "Backends", icon: Server },
  { to: "/routes", label: "Routes", icon: Route },
  { to: "/api-keys", label: "API Keys", icon: Key },
  { to: "/users", label: "Users", icon: Users },
  { to: "/audit", label: "Audit Logs", icon: ScrollText },
  { to: "/settings", label: "Settings", icon: Settings },
]

const llmItems = [
  { to: "/llm-playground", label: "Playground", icon: Sparkles },
  { to: "/llm-analytics", label: "LLM Analytics", icon: BarChart2 },
  { to: "/llm-catalog", label: "Model Catalog", icon: Library },
  { to: "/mcp-servers", label: "MCP Servers", icon: Plug },
  { to: "/prompts", label: "Prompts", icon: FileText },
  { to: "/guardrails", label: "Guardrails", icon: Shield },
]

export function DashboardLayout() {
  const tenantId = useAuth((s) => s.tenantId)

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
          {navItems.map((item) => (
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
          {llmItems.map((item) => (
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
