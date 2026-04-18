import { useQuery } from "@tanstack/react-query"
import { getUsageSummary } from "@/lib/api"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Activity,
  DollarSign,
  Zap,
  Hash,
  AlertTriangle,
} from "lucide-react"

export function Dashboard() {
  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["usage-summary"],
    queryFn: getUsageSummary,
    refetchInterval: 10_000,
  })

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load dashboard</h2>
        <p className="text-sm text-muted-foreground mt-1">
          {error instanceof Error ? error.message : "An unexpected error occurred"}
        </p>
      </div>
    )
  }

  const metrics = [
    {
      title: "Total Requests",
      value: data?.total_requests ?? 0,
      format: (v: number) => v.toLocaleString(),
      icon: Activity,
    },
    {
      title: "Total Cost",
      value: data?.total_cost_usd ?? 0,
      format: (v: number) => `$${v.toFixed(2)}`,
      icon: DollarSign,
    },
    {
      title: "Input Tokens",
      value: data?.total_tokens_input ?? 0,
      format: (v: number) => v.toLocaleString(),
      icon: Zap,
    },
    {
      title: "Output Tokens",
      value: data?.total_tokens_output ?? 0,
      format: (v: number) => v.toLocaleString(),
      icon: Hash,
    },
  ]

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Dashboard</h1>
        <p className="text-muted-foreground">
          Usage overview{data?.period ? ` (${data.period})` : ""}
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        {metrics.map((metric) => (
          <Card key={metric.title}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium">
                {metric.title}
              </CardTitle>
              <metric.icon className="h-4 w-4 text-muted-foreground" />
            </CardHeader>
            <CardContent>
              {isLoading ? (
                <Skeleton className="h-8 w-24" />
              ) : (
                <div className="text-2xl font-bold">
                  {metric.format(metric.value)}
                </div>
              )}
            </CardContent>
          </Card>
        ))}
      </div>

      {!isLoading && data && data.total_requests === 0 && (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <Activity className="h-10 w-10 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold">No activity yet</h3>
            <p className="text-sm text-muted-foreground mt-1">
              Configure backends and routes to start proxying requests.
            </p>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
