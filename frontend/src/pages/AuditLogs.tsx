import { useState, useMemo } from "react"
import { useQuery } from "@tanstack/react-query"
import { listAuditLogs, type AuditLogQuery } from "@/lib/api"
import { toast } from "@/hooks/use-toast"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardContent,
} from "@/components/ui/card"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Search,
  ArrowDownToLine,
  Clock,
  ShieldCheck,
  AlertCircle,
  ChevronLeft,
  ChevronRight,
  AlertTriangle,
  FileJson,
} from "lucide-react"
import { format } from "date-fns"

const PAGE_SIZE = 20

const RESOURCE_TYPES = ["all", "auth", "api_key", "backend", "route", "user", "webhook", "settings"]

export function AuditLogs() {
  const [offset, setOffset] = useState(0)
  const [search, setSearch] = useState("")
  const [resourceFilter, setResourceFilter] = useState("all")
  const [detailsLog, setDetailsLog] = useState<Record<string, unknown> | null>(null)

  const query: AuditLogQuery = useMemo(() => ({
    limit: PAGE_SIZE,
    offset,
    ...(resourceFilter !== "all" && { resource_type: resourceFilter }),
    ...(search && { action: search }),
  }), [offset, resourceFilter, search])

  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["audit-logs", query],
    queryFn: () => listAuditLogs(query),
  })

  const logs = data?.audit_logs ?? []
  const total = data?.total ?? 0
  const hasMore = data?.has_more ?? false
  const currentPage = Math.floor(offset / PAGE_SIZE) + 1
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  function handleSearch(value: string) {
    setSearch(value)
    setOffset(0)
  }

  function handleResourceFilter(value: string) {
    setResourceFilter(value)
    setOffset(0)
  }

  function exportCsv() {
    if (logs.length === 0) {
      toast({ title: "Nothing to export", description: "No logs match the current filters." })
      return
    }

    const headers = ["ID", "Action", "Resource Type", "Resource ID", "IP Address", "Created At"]
    const rows = logs.map((log) => [
      log.id,
      log.action,
      log.resource_type,
      log.resource_id ?? "",
      log.ip_address ?? "",
      log.created_at,
    ])

    const csv = [headers.join(","), ...rows.map((r) => r.join(","))].join("\n")
    const blob = new Blob([csv], { type: "text/csv" })
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url
    a.download = `audit-logs-${format(new Date(), "yyyy-MM-dd")}.csv`
    a.click()
    URL.revokeObjectURL(url)
    toast({ title: "Export complete", description: `${logs.length} records exported.` })
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load audit logs</h2>
        <p className="text-sm text-muted-foreground mt-1">
          {error instanceof Error ? error.message : "An unexpected error occurred"}
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Audit Logs</h1>
          <p className="text-muted-foreground">Immutable record of all gateway activity</p>
        </div>
        <Button variant="outline" onClick={exportCsv}>
          <ArrowDownToLine className="mr-2 h-4 w-4" /> Export CSV
        </Button>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3">
        <div className="relative flex-1 max-w-sm">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Filter by action..."
            className="pl-8"
            value={search}
            onChange={(e) => handleSearch(e.target.value)}
          />
        </div>
        <Select value={resourceFilter} onValueChange={handleResourceFilter}>
          <SelectTrigger className="w-[180px]">
            <SelectValue placeholder="Resource type" />
          </SelectTrigger>
          <SelectContent>
            {RESOURCE_TYPES.map((rt) => (
              <SelectItem key={rt} value={rt} className="capitalize">
                {rt === "all" ? "All Resources" : rt.replace(/_/g, " ")}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <Card>
        <CardContent className="pt-6">
          {isLoading ? (
            <div className="space-y-3">
              {Array.from({ length: 5 }).map((_, i) => <Skeleton key={i} className="h-12 w-full" />)}
            </div>
          ) : logs.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center text-muted-foreground">
              <ShieldCheck className="h-10 w-10 mb-4 opacity-30" />
              <p className="text-sm">No audit logs match the current filters.</p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Event</TableHead>
                  <TableHead>Resource</TableHead>
                  <TableHead>IP Address</TableHead>
                  <TableHead>Time</TableHead>
                  <TableHead className="text-right">Details</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {logs.map((log) => (
                  <TableRow key={log.id}>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        {log.action.toLowerCase().includes("failed") ? (
                          <AlertCircle className="h-4 w-4 text-destructive" />
                        ) : (
                          <ShieldCheck className="h-4 w-4 text-emerald-500" />
                        )}
                        <span className="font-medium text-sm capitalize">
                          {log.action.replace(/_/g, " ")}
                        </span>
                      </div>
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        <Badge variant="outline" className="text-xs capitalize">
                          {log.resource_type}
                        </Badge>
                        {log.resource_id && (
                          <span className="text-xs font-mono text-muted-foreground truncate max-w-[100px]">
                            {log.resource_id}
                          </span>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      {log.ip_address ?? "internal"}
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground whitespace-nowrap">
                      <div className="flex items-center gap-1.5">
                        <Clock className="h-3 w-3" />
                        {format(new Date(log.created_at), "MMM d, HH:mm:ss")}
                      </div>
                    </TableCell>
                    <TableCell className="text-right">
                      {log.details && (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setDetailsLog(log.details)}
                        >
                          <FileJson className="mr-1 h-3 w-3" /> View
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}

          {/* Pagination */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between pt-4 border-t mt-4">
              <span className="text-sm text-muted-foreground">
                {total} total records
              </span>
              <div className="flex items-center gap-2">
                <span className="text-sm text-muted-foreground">
                  Page {currentPage} of {totalPages}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setOffset((o) => Math.max(0, o - PAGE_SIZE))}
                  disabled={offset === 0}
                >
                  <ChevronLeft className="h-4 w-4" />
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setOffset((o) => o + PAGE_SIZE)}
                  disabled={!hasMore}
                >
                  <ChevronRight className="h-4 w-4" />
                </Button>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Details Dialog */}
      <Dialog open={!!detailsLog} onOpenChange={(open) => { if (!open) setDetailsLog(null) }}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Event Details</DialogTitle>
          </DialogHeader>
          <pre className="bg-muted rounded-lg p-4 text-xs font-mono overflow-auto max-h-[400px]">
            {JSON.stringify(detailsLog, null, 2)}
          </pre>
        </DialogContent>
      </Dialog>
    </div>
  )
}
