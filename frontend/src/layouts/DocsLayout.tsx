import { Outlet, NavLink, Link } from "react-router-dom"
import { BookOpen, User, Building2, Code2, Terminal, ArrowLeft } from "lucide-react"
import { cn } from "@/lib/utils"
import { ThemeToggle } from "@/components/ThemeToggle"

const docsNav = [
  { to: "/docs", label: "Overview", icon: BookOpen, end: true },
  { to: "/docs/users", label: "User Guide", icon: User },
  { to: "/docs/tenants", label: "Tenant Admin Guide", icon: Building2 },
  { to: "/docs/developers", label: "Developer Guide", icon: Code2 },
  { to: "/docs/api", label: "API Reference", icon: Terminal },
]

export function DocsLayout() {
  return (
    <div className="flex min-h-screen bg-muted/20">
      <aside className="w-64 flex-col border-r bg-background sticky top-0 h-screen">
        <div className="flex h-16 items-center px-6 border-b justify-between">
          <Link to="/" className="flex items-center gap-2">
            <div className="size-8 rounded bg-primary text-primary-foreground flex items-center justify-center font-bold">
              SG
            </div>
            <span className="font-semibold text-lg tracking-tight">
              Sentinel Docs
            </span>
          </Link>
        </div>
        <nav className="flex-1 space-y-1 p-4 overflow-y-auto">
          <div className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2 px-3">
            Documentation
          </div>
          {docsNav.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              end={item.end}
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
            <Link
              to="/login"
              className="flex items-center gap-3 px-3 py-2 rounded-md text-sm font-medium text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
            >
              <ArrowLeft className="size-4" />
              Back to App
            </Link>
          </div>
        </nav>
      </aside>

      <main className="flex-1 flex flex-col">
        <header className="h-16 border-b bg-background flex items-center px-8 justify-between sticky top-0 z-10">
          <div className="text-sm text-muted-foreground">
            Public documentation — no sign-in required
          </div>
          <ThemeToggle />
        </header>
        <div className="flex-1 p-8 md:p-12 overflow-auto">
          <div className="max-w-4xl mx-auto">
            <Outlet />
          </div>
        </div>
      </main>
    </div>
  )
}
