import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { listFeedback, getFeedbackStats } from "@/lib/api"
import { FeatureGate } from "@/components/FeatureGate"
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table"
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select"
import { ThumbsUp, ThumbsDown, MessageSquare, AlertTriangle, TrendingUp } from "lucide-react"
import { format } from "date-fns"

export function Feedback() {
  return (
    <FeatureGate
      feature="feedback_enabled"
      title="LLM Feedback Collection"
      description="Capture thumbs-up/down ratings and comments from end-users on LLM responses."
      requiredPlan="professional"
    >
      <FeedbackInner />
    </FeatureGate>
  )
}

function FeedbackInner() {
  const [windowDays, setWindowDays] = useState<number>(30)

  const { data: stats, isLoading: statsLoading } = useQuery({
    queryKey: ["feedback-stats", windowDays],
    queryFn: () => getFeedbackStats(windowDays),
  })

  const { data: feedback, isLoading, isError, error } = useQuery({
    queryKey: ["feedback-list"],
    queryFn: () => listFeedback(100),
  })

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load feedback</h2>
        <p className="text-sm text-muted-foreground mt-1">
          {error instanceof Error ? error.message : "An unexpected error occurred"}
        </p>
      </div>
    )
  }

  const rows = feedback ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">LLM Feedback</h1>
          <p className="text-muted-foreground">
            End-user ratings and comments on LLM responses
          </p>
        </div>
        <Select value={String(windowDays)} onValueChange={(v) => setWindowDays(Number(v))}>
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="7">Last 7 days</SelectItem>
            <SelectItem value="30">Last 30 days</SelectItem>
            <SelectItem value="90">Last 90 days</SelectItem>
          </SelectContent>
        </Select>
      </div>

      {/* ── Stats cards ───────────────────────────────────── */}
      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Total feedback</CardDescription>
          </CardHeader>
          <CardContent>
            {statsLoading ? (
              <Skeleton className="h-8 w-16" />
            ) : (
              <div className="text-2xl font-bold flex items-center gap-2">
                <MessageSquare className="h-5 w-5 text-muted-foreground" />
                {stats?.total ?? 0}
              </div>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Positive</CardDescription>
          </CardHeader>
          <CardContent>
            {statsLoading ? (
              <Skeleton className="h-8 w-16" />
            ) : (
              <div className="text-2xl font-bold flex items-center gap-2 text-green-600">
                <ThumbsUp className="h-5 w-5" />
                {stats?.positive ?? 0}
              </div>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Negative</CardDescription>
          </CardHeader>
          <CardContent>
            {statsLoading ? (
              <Skeleton className="h-8 w-16" />
            ) : (
              <div className="text-2xl font-bold flex items-center gap-2 text-red-600">
                <ThumbsDown className="h-5 w-5" />
                {stats?.negative ?? 0}
              </div>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Positive ratio</CardDescription>
          </CardHeader>
          <CardContent>
            {statsLoading ? (
              <Skeleton className="h-8 w-16" />
            ) : (
              <div className="text-2xl font-bold flex items-center gap-2">
                <TrendingUp className="h-5 w-5 text-muted-foreground" />
                {stats ? `${Math.round(stats.positive_ratio * 100)}%` : "—"}
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* ── Recent feedback ──────────────────────────────── */}
      <Card>
        <CardHeader>
          <CardTitle>Recent feedback</CardTitle>
          <CardDescription>Most recent 100 entries</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2">
              {[1, 2, 3, 4, 5].map((i) => <Skeleton key={i} className="h-10 w-full" />)}
            </div>
          ) : rows.length === 0 ? (
            <div className="text-sm text-muted-foreground py-8 text-center">
              No feedback yet. End-users can submit feedback via the /feedback API.
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Rating</TableHead>
                  <TableHead>Comment</TableHead>
                  <TableHead>Reference</TableHead>
                  <TableHead>Submitted</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((f) => (
                  <TableRow key={f.id}>
                    <TableCell>
                      {f.rating === 1 && (
                        <Badge variant="success" className="gap-1">
                          <ThumbsUp className="h-3 w-3" /> +1
                        </Badge>
                      )}
                      {f.rating === -1 && (
                        <Badge variant="destructive" className="gap-1">
                          <ThumbsDown className="h-3 w-3" /> -1
                        </Badge>
                      )}
                      {f.rating === 0 && <Badge variant="outline">Neutral</Badge>}
                    </TableCell>
                    <TableCell className="max-w-md truncate">
                      {f.comment ?? <span className="text-muted-foreground italic">—</span>}
                    </TableCell>
                    <TableCell className="font-mono text-xs truncate max-w-[180px]">
                      {f.llm_log_id ?? f.request_id ?? "—"}
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      {format(new Date(f.created_at), "MMM d, HH:mm")}
                    </TableCell>
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
