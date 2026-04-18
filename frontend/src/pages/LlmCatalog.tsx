import { useState, useMemo } from "react"
import { useQuery } from "@tanstack/react-query"
import { Link } from "react-router-dom"
import { listBackends, type Backend } from "@/lib/api"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
  CardFooter,
} from "@/components/ui/card"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Search,
  Zap,
  Brain,
  Shield,
  Server,
  ExternalLink,
  AlertTriangle,
} from "lucide-react"

const PROVIDER_LABELS: Record<string, string> = {
  open_ai: "OpenAI",
  anthropic: "Anthropic",
  google_vertex: "Google Vertex",
  aws_bedrock: "AWS Bedrock",
  ollama: "Ollama",
  vllm: "vLLM",
  open_ai_compatible: "OpenAI Compatible",
}

export function LlmCatalog() {
  const [search, setSearch] = useState("")
  const [providerFilter, setProviderFilter] = useState("all")
  const [statusFilter, setStatusFilter] = useState("all")
  const [detailsTarget, setDetailsTarget] = useState<Backend | null>(null)

  const { data: backends, isLoading, isError, error } = useQuery({
    queryKey: ["backends"],
    queryFn: listBackends,
  })

  const allProviders = useMemo(() => {
    const set = new Set((backends ?? []).map((b) => b.provider_type))
    return Array.from(set)
  }, [backends])

  const filtered = useMemo(() => {
    return (backends ?? []).filter((b) => {
      const matchSearch =
        !search ||
        b.name.toLowerCase().includes(search.toLowerCase()) ||
        b.provider_type.toLowerCase().includes(search.toLowerCase()) ||
        b.endpoint.toLowerCase().includes(search.toLowerCase())
      const matchProvider = providerFilter === "all" || b.provider_type === providerFilter
      const matchStatus =
        statusFilter === "all" ||
        (statusFilter === "healthy" && b.health_status === "healthy") ||
        (statusFilter === "unhealthy" && b.health_status !== "healthy")
      return matchSearch && matchProvider && matchStatus
    })
  }, [backends, search, providerFilter, statusFilter])

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load catalog</h2>
        <p className="text-sm text-muted-foreground mt-1">
          {error instanceof Error ? error.message : "An unexpected error occurred"}
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Model Catalog</h1>
        <p className="text-muted-foreground">Browse all available backends and models</p>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3 flex-wrap">
        <div className="relative flex-1 max-w-sm">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search by name, provider, or endpoint..."
            className="pl-8"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <Select value={providerFilter} onValueChange={setProviderFilter}>
          <SelectTrigger className="w-[180px]">
            <SelectValue placeholder="Provider" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All Providers</SelectItem>
            {allProviders.map((p) => (
              <SelectItem key={p} value={p} className="capitalize">
                {PROVIDER_LABELS[p] ?? p.replace(/_/g, " ")}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select value={statusFilter} onValueChange={setStatusFilter}>
          <SelectTrigger className="w-[150px]">
            <SelectValue placeholder="Status" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All Status</SelectItem>
            <SelectItem value="healthy">Healthy</SelectItem>
            <SelectItem value="unhealthy">Unhealthy</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* Grid */}
      {isLoading ? (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {[1, 2, 3].map((i) => (
            <Card key={i}>
              <CardContent className="p-6 space-y-3">
                <Skeleton className="h-6 w-32" />
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-2/3" />
              </CardContent>
            </Card>
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <Brain className="h-10 w-10 text-muted-foreground mb-4" />
            {(backends ?? []).length === 0 ? (
              <>
                <h3 className="text-lg font-semibold">No backends configured</h3>
                <p className="text-sm text-muted-foreground mt-1">
                  Add backends to see them in the catalog.
                </p>
                <Button className="mt-4" asChild>
                  <Link to="/backends">Go to Backends</Link>
                </Button>
              </>
            ) : (
              <>
                <h3 className="text-lg font-semibold">No results</h3>
                <p className="text-sm text-muted-foreground mt-1">
                  No backends match the current filters.
                </p>
              </>
            )}
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {filtered.map((b) => (
            <Card key={b.id} className="flex flex-col hover:border-primary/30 transition-colors">
              <CardHeader className="flex-1 pb-3">
                <div className="flex items-center justify-between mb-2">
                  <Badge variant="outline" className="capitalize">
                    {PROVIDER_LABELS[b.provider_type] ?? b.provider_type.replace(/_/g, " ")}
                  </Badge>
                  <Badge variant={b.health_status === "healthy" ? "success" : "destructive"}>
                    {b.health_status === "healthy" ? "Online" : "Offline"}
                  </Badge>
                </div>
                <CardTitle className="text-lg flex items-center gap-2">
                  <Brain className="h-5 w-5 text-primary" />
                  {b.name}
                </CardTitle>
                <CardDescription className="text-xs font-mono truncate mt-1">
                  {b.endpoint}
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-3 pt-0">
                <div className="grid grid-cols-2 gap-3 text-sm">
                  <div className="space-y-1">
                    <span className="text-muted-foreground text-xs flex items-center gap-1">
                      <Zap className="h-3 w-3" /> Priority
                    </span>
                    <span className="font-semibold">{b.priority}</span>
                  </div>
                  <div className="space-y-1">
                    <span className="text-muted-foreground text-xs flex items-center gap-1">
                      <Server className="h-3 w-3" /> Weight
                    </span>
                    <span className="font-semibold">{b.weight}</span>
                  </div>
                </div>
                <div className="flex items-center gap-2 text-xs text-muted-foreground bg-muted/30 p-2 rounded-md border border-border/50">
                  <Shield className="h-3.5 w-3.5 shrink-0" />
                  Timeout: {b.timeout_ms}ms | Retries: {b.max_retries}
                </div>
              </CardContent>
              <CardFooter className="border-t pt-3">
                <Button
                  variant="ghost"
                  size="sm"
                  className="w-full"
                  onClick={() => setDetailsTarget(b)}
                >
                  <ExternalLink className="h-3.5 w-3.5 mr-1.5" /> View Details
                </Button>
              </CardFooter>
            </Card>
          ))}
        </div>
      )}

      {/* Details Dialog */}
      <Dialog open={!!detailsTarget} onOpenChange={(open) => { if (!open) setDetailsTarget(null) }}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Brain className="h-5 w-5 text-primary" />
              {detailsTarget?.name}
            </DialogTitle>
          </DialogHeader>
          {detailsTarget && (
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <span className="text-muted-foreground text-xs block mb-1">Provider</span>
                  <Badge variant="outline" className="capitalize">
                    {PROVIDER_LABELS[detailsTarget.provider_type] ?? detailsTarget.provider_type}
                  </Badge>
                </div>
                <div>
                  <span className="text-muted-foreground text-xs block mb-1">Health</span>
                  <Badge variant={detailsTarget.health_status === "healthy" ? "success" : "destructive"}>
                    {detailsTarget.health_status}
                  </Badge>
                </div>
                <div>
                  <span className="text-muted-foreground text-xs block mb-1">Priority</span>
                  <span className="font-semibold">{detailsTarget.priority}</span>
                </div>
                <div>
                  <span className="text-muted-foreground text-xs block mb-1">Weight</span>
                  <span className="font-semibold">{detailsTarget.weight}</span>
                </div>
                <div>
                  <span className="text-muted-foreground text-xs block mb-1">Timeout</span>
                  <span className="font-semibold">{detailsTarget.timeout_ms}ms</span>
                </div>
                <div>
                  <span className="text-muted-foreground text-xs block mb-1">Max Retries</span>
                  <span className="font-semibold">{detailsTarget.max_retries}</span>
                </div>
              </div>
              <div>
                <span className="text-muted-foreground text-xs block mb-1">Endpoint</span>
                <code className="text-xs font-mono bg-muted p-2 rounded block break-all">
                  {detailsTarget.endpoint}
                </code>
              </div>
              {detailsTarget.last_health_check && (
                <div>
                  <span className="text-muted-foreground text-xs block mb-1">Last Health Check</span>
                  <span className="text-sm">
                    {new Date(detailsTarget.last_health_check).toLocaleString()}
                  </span>
                </div>
              )}
              <div className="flex items-center gap-2 pt-2">
                <Badge variant={detailsTarget.is_active ? "success" : "secondary"}>
                  {detailsTarget.is_active ? "Active" : "Inactive"}
                </Badge>
                <span className="text-xs text-muted-foreground">
                  Created {new Date(detailsTarget.created_at).toLocaleDateString()}
                </span>
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>
    </div>
  )
}
