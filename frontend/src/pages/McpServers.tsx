import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  listMcpServers,
  registerMcpServer,
  removeMcpServer,
  refreshMcpServer,
  listMcpTools,
  type McpServerInfo,
} from "@/lib/api"
import { validate, required, url as urlValidator } from "@/lib/fp-validate"
import { toast } from "@/hooks/use-toast"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogDescription,
} from "@/components/ui/dialog"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Plus,
  Trash2,
  RefreshCw,
  Loader2,
  AlertTriangle,
  Plug,
  Wrench,
  FileText,
  Sparkles,
} from "lucide-react"

export function McpServers() {
  const queryClient = useQueryClient()
  const [addOpen, setAddOpen] = useState(false)
  const [name, setName] = useState("")
  const [url, setUrl] = useState("")
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [deleteTarget, setDeleteTarget] = useState<McpServerInfo | null>(null)
  const [toolsOpen, setToolsOpen] = useState(false)

  const { data: serversData, isLoading, isError, error } = useQuery({
    queryKey: ["mcp-servers"],
    queryFn: listMcpServers,
  })

  const { data: toolsData, isLoading: toolsLoading } = useQuery({
    queryKey: ["mcp-tools"],
    queryFn: listMcpTools,
    enabled: toolsOpen,
  })

  const servers = serversData?.mcp_servers ?? []
  const tools = toolsData?.tools ?? []

  const registerMut = useMutation({
    mutationFn: ({ n, u }: { n: string; u: string }) => registerMcpServer(n, u),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["mcp-servers"] })
      queryClient.invalidateQueries({ queryKey: ["mcp-tools"] })
      toast({
        title: "MCP server connected",
        description: `"${data.name}" connected with ${data.tools_count} tools.`,
      })
      closeAdd()
    },
    onError: (err: Error) => {
      toast({ title: "Failed to connect MCP server", description: err.message, variant: "destructive" })
    },
  })

  const removeMut = useMutation({
    mutationFn: (id: string) => removeMcpServer(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["mcp-servers"] })
      queryClient.invalidateQueries({ queryKey: ["mcp-tools"] })
      toast({ title: "MCP server removed", description: `"${deleteTarget?.name}" disconnected.` })
      setDeleteTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to remove server", description: err.message, variant: "destructive" })
      setDeleteTarget(null)
    },
  })

  const refreshMut = useMutation({
    mutationFn: (id: string) => refreshMcpServer(id),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["mcp-servers"] })
      queryClient.invalidateQueries({ queryKey: ["mcp-tools"] })
      toast({ title: "Discovery refreshed", description: `Found ${data.tools_count} tools, ${data.resources_count} resources.` })
    },
    onError: (err: Error) => {
      toast({ title: "Refresh failed", description: err.message, variant: "destructive" })
    },
  })

  function closeAdd() {
    setAddOpen(false)
    setName("")
    setUrl("")
    setErrors({})
  }

  function handleRegister() {
    const fieldErrors: Record<string, string> = {}
    pipe(
      validate(name, required("Name")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["name"] = e.message)))
    )
    pipe(
      validate(url, required("URL"), urlValidator("URL")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["url"] = e.message)))
    )
    if (Object.keys(fieldErrors).length > 0) {
      setErrors(fieldErrors)
      return
    }
    setErrors({})
    registerMut.mutate({ n: name, u: url })
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load MCP servers</h2>
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
          <h1 className="text-2xl font-bold tracking-tight">MCP Servers</h1>
          <p className="text-muted-foreground">
            Connect to Model Context Protocol servers for tool and resource access
          </p>
        </div>
        <Button onClick={() => setAddOpen(true)}>
          <Plus className="mr-2 h-4 w-4" /> Connect Server
        </Button>
      </div>

      <Tabs defaultValue="servers" onValueChange={(v) => setToolsOpen(v === "tools")}>
        <TabsList>
          <TabsTrigger value="servers">Servers ({servers.length})</TabsTrigger>
          <TabsTrigger value="tools">Aggregated Tools</TabsTrigger>
        </TabsList>

        {/* ── Servers Tab ─────────────────────────────────────── */}
        <TabsContent value="servers" className="mt-4">
          {isLoading ? (
            <Card>
              <CardContent className="p-6 space-y-3">
                {[1, 2, 3].map((i) => <Skeleton key={i} className="h-14 w-full" />)}
              </CardContent>
            </Card>
          ) : servers.length === 0 ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-12 text-center">
                <Plug className="h-10 w-10 text-muted-foreground mb-4" />
                <h3 className="text-lg font-semibold">No MCP servers connected</h3>
                <p className="text-sm text-muted-foreground mt-1">
                  Connect an MCP server to expose its tools and resources through the gateway.
                </p>
                <Button className="mt-4" onClick={() => setAddOpen(true)}>
                  <Plus className="mr-2 h-4 w-4" /> Connect Server
                </Button>
              </CardContent>
            </Card>
          ) : (
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
              {servers.map((s) => (
                <Card key={s.id} className="flex flex-col">
                  <CardHeader className="pb-3">
                    <div className="flex items-center justify-between">
                      <CardTitle className="text-base flex items-center gap-2">
                        <Plug className="h-4 w-4 text-primary" />
                        {s.name}
                      </CardTitle>
                      <Badge variant={s.is_healthy ? "success" : "destructive"}>
                        {s.is_healthy ? "Connected" : "Disconnected"}
                      </Badge>
                    </div>
                    <CardDescription className="font-mono text-xs truncate">
                      {s.url}
                    </CardDescription>
                  </CardHeader>
                  <CardContent className="flex-1">
                    <div className="grid grid-cols-3 gap-3 text-center">
                      <div className="space-y-1">
                        <Wrench className="h-4 w-4 mx-auto text-muted-foreground" />
                        <p className="text-lg font-bold">{s.tools_count}</p>
                        <p className="text-xs text-muted-foreground">Tools</p>
                      </div>
                      <div className="space-y-1">
                        <FileText className="h-4 w-4 mx-auto text-muted-foreground" />
                        <p className="text-lg font-bold">{s.resources_count}</p>
                        <p className="text-xs text-muted-foreground">Resources</p>
                      </div>
                      <div className="space-y-1">
                        <Sparkles className="h-4 w-4 mx-auto text-muted-foreground" />
                        <p className="text-lg font-bold">{s.prompts_count}</p>
                        <p className="text-xs text-muted-foreground">Prompts</p>
                      </div>
                    </div>
                  </CardContent>
                  <div className="border-t p-3 flex justify-between">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => refreshMut.mutate(s.id)}
                      disabled={refreshMut.isPending}
                    >
                      <RefreshCw className={`h-3.5 w-3.5 mr-1.5 ${refreshMut.isPending ? "animate-spin" : ""}`} />
                      Refresh
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setDeleteTarget(s)}
                      className="text-destructive hover:text-destructive"
                    >
                      <Trash2 className="h-3.5 w-3.5 mr-1.5" />
                      Remove
                    </Button>
                  </div>
                </Card>
              ))}
            </div>
          )}
        </TabsContent>

        {/* ── Tools Tab ───────────────────────────────────────── */}
        <TabsContent value="tools" className="mt-4">
          <Card>
            <CardHeader>
              <CardTitle>All Aggregated Tools ({tools.length})</CardTitle>
              <CardDescription>
                Tools from all connected MCP servers, namespaced by server name
              </CardDescription>
            </CardHeader>
            <CardContent>
              {toolsLoading ? (
                <div className="space-y-3">
                  {[1, 2, 3, 4].map((i) => <Skeleton key={i} className="h-12 w-full" />)}
                </div>
              ) : tools.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-8 text-center text-muted-foreground">
                  <Wrench className="h-8 w-8 mb-3 opacity-30" />
                  <p className="text-sm">No tools available. Connect an MCP server first.</p>
                </div>
              ) : (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Tool Name</TableHead>
                      <TableHead>Description</TableHead>
                      <TableHead>Input Schema</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {tools.map((t) => {
                      const parts = t.name.split("__")
                      const server = parts.length > 1 ? parts[0] : ""
                      const toolName = parts.length > 1 ? parts.slice(1).join("__") : t.name
                      return (
                        <TableRow key={t.name}>
                          <TableCell>
                            <div className="flex items-center gap-2">
                              <Wrench className="h-3.5 w-3.5 text-primary shrink-0" />
                              <div>
                                <span className="font-medium text-sm">{toolName}</span>
                                {server && (
                                  <Badge variant="outline" className="ml-2 text-xs">
                                    {server}
                                  </Badge>
                                )}
                              </div>
                            </div>
                          </TableCell>
                          <TableCell className="text-sm text-muted-foreground max-w-[300px] truncate">
                            {t.description ?? "\u2014"}
                          </TableCell>
                          <TableCell>
                            <Badge variant="secondary" className="text-xs font-mono">
                              {Object.keys(t.inputSchema?.properties ?? {}).length} params
                            </Badge>
                          </TableCell>
                        </TableRow>
                      )
                    })}
                  </TableBody>
                </Table>
              )}
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>

      {/* Add Server Dialog */}
      <Dialog open={addOpen} onOpenChange={(open) => { if (!open) closeAdd() }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Connect MCP Server</DialogTitle>
            <DialogDescription>
              Enter the Streamable HTTP endpoint of the MCP server to connect.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="mcpName">Server Name</Label>
              <Input
                id="mcpName"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="github"
              />
              {errors["name"] && <p className="text-sm text-destructive">{errors["name"]}</p>}
            </div>
            <div className="grid gap-2">
              <Label htmlFor="mcpUrl">Endpoint URL</Label>
              <Input
                id="mcpUrl"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="http://localhost:3001/mcp"
              />
              {errors["url"] && <p className="text-sm text-destructive">{errors["url"]}</p>}
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeAdd}>Cancel</Button>
            <Button onClick={handleRegister} disabled={registerMut.isPending}>
              {registerMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Connect
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Remove Confirmation */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => { if (!open) setDeleteTarget(null) }}
        title="Remove MCP Server"
        description={`Are you sure you want to disconnect "${deleteTarget?.name}"? All tools and resources from this server will become unavailable. This action cannot be undone.`}
        confirmLabel="Remove"
        variant="destructive"
        loading={removeMut.isPending}
        onConfirm={() => deleteTarget && removeMut.mutate(deleteTarget.id)}
      />
    </div>
  )
}
