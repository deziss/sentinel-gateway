import { useQuery } from "@tanstack/react-query"
import { getUsageSummary, listBackends } from "@/lib/api"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Skeleton } from "@/components/ui/skeleton"
import {
  DollarSign,
  Zap,
  Hash,
  Activity,
  AlertTriangle,
  Server,
} from "lucide-react"

export function LlmAnalytics() {
  const {
    data: usage,
    isLoading: usageLoading,
    isError: usageError,
    error: usageErr,
  } = useQuery({
    queryKey: ["usage-summary"],
    queryFn: getUsageSummary,
    refetchInterval: 30_000,
  })

  const { data: backends, isLoading: backendsLoading } = useQuery({
    queryKey: ["backends"],
    queryFn: listBackends,
  })

  const llmBackends = (backends ?? []).filter((b) =>
    ["open_ai", "anthropic", "google_vertex", "aws_bedrock", "ollama", "vllm", "open_ai_compatible"].includes(
      b.provider_type
    )
  )

  if (usageError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load analytics</h2>
        <p className="text-sm text-muted-foreground mt-1">
          {usageErr instanceof Error ? usageErr.message : "An unexpected error occurred"}
        </p>
      </div>
    )
  }

  const metrics = [
    {
      title: "Total Cost",
      value: usage?.total_cost_usd ?? 0,
      format: (v: number) => `$${v.toFixed(2)}`,
      icon: DollarSign,
      accent: "text-emerald-500",
    },
    {
      title: "Total Requests",
      value: usage?.total_requests ?? 0,
      format: (v: number) => v.toLocaleString(),
      icon: Activity,
      accent: "text-blue-500",
    },
    {
      title: "Input Tokens",
      value: usage?.total_tokens_input ?? 0,
      format: (v: number) => v.toLocaleString(),
      icon: Zap,
      accent: "text-amber-500",
    },
    {
      title: "Output Tokens",
      value: usage?.total_tokens_output ?? 0,
      format: (v: number) => v.toLocaleString(),
      icon: Hash,
      accent: "text-purple-500",
    },
  ]

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">LLM Analytics</h1>
        <p className="text-muted-foreground">
          Token usage and cost insights{usage?.period ? ` (${usage.period})` : ""}
        </p>
      </div>

      {/* KPI Cards */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        {metrics.map((m) => (
          <Card key={m.title}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">{m.title}</CardTitle>
              <m.icon className={`h-4 w-4 ${m.accent}`} />
            </CardHeader>
            <CardContent>
              {usageLoading ? (
                <Skeleton className="h-8 w-24" />
              ) : (
                <div className="text-2xl font-bold">{m.format(m.value)}</div>
              )}
            </CardContent>
          </Card>
        ))}
      </div>

      {/* Token Breakdown */}
      {!usageLoading && usage && (
        <Card>
          <CardHeader>
            <CardTitle>Token Breakdown</CardTitle>
            <CardDescription>Input vs output token distribution</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              {/* Visual bar */}
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">Input Tokens</span>
                  <span className="font-mono">{(usage.total_tokens_input ?? 0).toLocaleString()}</span>
                </div>
                <div className="h-3 w-full bg-muted rounded-full overflow-hidden">
                  <div
                    className="h-full bg-blue-500 rounded-full transition-all"
                    style={{
                      width: `${usage.total_tokens > 0 ? (usage.total_tokens_input / usage.total_tokens) * 100 : 0}%`,
                    }}
                  />
                </div>
              </div>
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">Output Tokens</span>
                  <span className="font-mono">{(usage.total_tokens_output ?? 0).toLocaleString()}</span>
                </div>
                <div className="h-3 w-full bg-muted rounded-full overflow-hidden">
                  <div
                    className="h-full bg-purple-500 rounded-full transition-all"
                    style={{
                      width: `${usage.total_tokens > 0 ? (usage.total_tokens_output / usage.total_tokens) * 100 : 0}%`,
                    }}
                  />
                </div>
              </div>
              <div className="flex justify-between text-sm font-medium pt-2 border-t">
                <span>Total Tokens</span>
                <span className="font-mono">{(usage.total_tokens ?? 0).toLocaleString()}</span>
              </div>
            </div>
          </CardContent>
        </Card>
      )}

      {/* Backend / Provider Table */}
      <Card>
        <CardHeader>
          <CardTitle>Active LLM Providers</CardTitle>
          <CardDescription>Backend services handling LLM requests</CardDescription>
        </CardHeader>
        <CardContent>
          {backendsLoading ? (
            <div className="space-y-3">
              {[1, 2, 3].map((i) => (
                <Skeleton key={i} className="h-12 w-full" />
              ))}
            </div>
          ) : llmBackends.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-8 text-center text-muted-foreground">
              <Server className="h-8 w-8 mb-3 opacity-30" />
              <p className="text-sm">No LLM backends configured</p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Provider</TableHead>
                  <TableHead>Health</TableHead>
                  <TableHead>Priority</TableHead>
                  <TableHead>Weight</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {llmBackends.map((b) => (
                  <TableRow key={b.id}>
                    <TableCell className="font-medium">{b.name}</TableCell>
                    <TableCell>
                      <Badge variant="outline" className="capitalize">
                        {b.provider_type.replace(/_/g, " ")}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <Badge
                        variant={
                          b.health_status === "healthy"
                            ? "success"
                            : b.health_status === "degraded"
                              ? "warning"
                              : "destructive"
                        }
                      >
                        {b.health_status ?? "unknown"}
                      </Badge>
                    </TableCell>
                    <TableCell>{b.priority}</TableCell>
                    <TableCell>{b.weight}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
