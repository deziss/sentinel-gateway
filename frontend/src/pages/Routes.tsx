import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  listRoutes,
  listBackends,
  createRoute,
  deleteRoute,
  type ProxyRoute,
  type CreateRouteInput,
} from "@/lib/api"
import { validate, required } from "@/lib/fp-validate"
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
} from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
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
import { Switch } from "@/components/ui/switch"
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Plus,
  Trash2,
  Route,
  Loader2,
  AlertTriangle,
} from "lucide-react"

const PROTOCOLS = ["rest", "graphql", "grpc", "generic"]

const emptyForm: CreateRouteInput = {
  name: "",
  protocol: "rest",
  path_pattern: "",
  backend_id: "",
  strip_prefix: false,
  priority: 10,
}

export function ProxyRoutes() {
  const queryClient = useQueryClient()
  const [formOpen, setFormOpen] = useState(false)
  const [form, setForm] = useState<CreateRouteInput>(emptyForm)
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [deleteTarget, setDeleteTarget] = useState<ProxyRoute | null>(null)

  const { data: routes, isLoading, isError, error } = useQuery({
    queryKey: ["routes"],
    queryFn: listRoutes,
  })

  const { data: backends } = useQuery({
    queryKey: ["backends"],
    queryFn: listBackends,
  })

  const createMut = useMutation({
    mutationFn: (input: CreateRouteInput) => createRoute(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["routes"] })
      toast({ title: "Route created", description: `"${form.name}" has been added.` })
      closeForm()
    },
    onError: (err: Error) => {
      toast({ title: "Failed to create route", description: err.message, variant: "destructive" })
    },
  })

  const deleteMut = useMutation({
    mutationFn: (id: string) => deleteRoute(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["routes"] })
      toast({ title: "Route deleted", description: `"${deleteTarget?.name}" has been removed.` })
      setDeleteTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to delete route", description: err.message, variant: "destructive" })
      setDeleteTarget(null)
    },
  })

  function closeForm() {
    setFormOpen(false)
    setForm(emptyForm)
    setErrors({})
  }

  function handleCreate() {
    const fieldErrors: Record<string, string> = {}
    pipe(
      validate(form.name, required("Name")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["name"] = e.message)))
    )
    pipe(
      validate(form.path_pattern, required("Path pattern")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["path_pattern"] = e.message)))
    )
    if (!form.backend_id) fieldErrors["backend_id"] = "Select a backend"
    if (Object.keys(fieldErrors).length > 0) {
      setErrors(fieldErrors)
      return
    }
    setErrors({})
    createMut.mutate(form)
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load routes</h2>
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
          <h1 className="text-2xl font-bold tracking-tight">Proxy Routes</h1>
          <p className="text-muted-foreground">Map request paths to upstream backends</p>
        </div>
        <Button onClick={() => setFormOpen(true)}>
          <Plus className="mr-2 h-4 w-4" /> Create Route
        </Button>
      </div>

      {isLoading ? (
        <Card>
          <CardContent className="p-6 space-y-3">
            {[1, 2, 3].map((i) => <Skeleton key={i} className="h-12 w-full" />)}
          </CardContent>
        </Card>
      ) : routes && routes.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <Route className="h-10 w-10 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold">No routes defined</h3>
            <p className="text-sm text-muted-foreground mt-1">
              Requests will use the default proxy handler. Create routes for path-based routing.
            </p>
            <Button className="mt-4" onClick={() => setFormOpen(true)}>
              <Plus className="mr-2 h-4 w-4" /> Create Route
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>Routing Table ({routes?.length ?? 0})</CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Path Pattern</TableHead>
                  <TableHead>Protocol</TableHead>
                  <TableHead>Backend</TableHead>
                  <TableHead>Priority</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {routes?.map((r) => {
                  const backend = backends?.find((b) => b.id === r.backend_id)
                  return (
                    <TableRow key={r.id}>
                      <TableCell className="font-medium">{r.name}</TableCell>
                      <TableCell className="font-mono text-xs">{r.path_pattern}</TableCell>
                      <TableCell>
                        <Badge variant="outline" className="uppercase text-xs">{r.protocol}</Badge>
                      </TableCell>
                      <TableCell className="text-sm">
                        {backend?.name ?? r.backend_id.slice(0, 8) + "..."}
                      </TableCell>
                      <TableCell>{r.priority}</TableCell>
                      <TableCell>
                        <Badge variant={r.is_active ? "success" : "secondary"}>
                          {r.is_active ? "Active" : "Disabled"}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right">
                        <Button variant="ghost" size="icon" onClick={() => setDeleteTarget(r)}>
                          <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      {/* Create Route Dialog */}
      <Dialog open={formOpen} onOpenChange={(open) => { if (!open) closeForm() }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Create Route</DialogTitle>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="routeName">Name</Label>
              <Input
                id="routeName"
                value={form.name}
                onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
                placeholder="chat-completions"
              />
              {errors["name"] && <p className="text-sm text-destructive">{errors["name"]}</p>}
            </div>
            <div className="grid gap-2">
              <Label htmlFor="pathPattern">Path Pattern</Label>
              <Input
                id="pathPattern"
                value={form.path_pattern}
                onChange={(e) => setForm((f) => ({ ...f, path_pattern: e.target.value }))}
                placeholder="/v1/chat/completions"
                className="font-mono"
              />
              {errors["path_pattern"] && <p className="text-sm text-destructive">{errors["path_pattern"]}</p>}
            </div>
            <div className="grid gap-2">
              <Label>Protocol</Label>
              <Select
                value={form.protocol}
                onValueChange={(v) => setForm((f) => ({ ...f, protocol: v }))}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PROTOCOLS.map((p) => (
                    <SelectItem key={p} value={p} className="uppercase">{p}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-2">
              <Label>Target Backend</Label>
              <Select
                value={form.backend_id}
                onValueChange={(v) => setForm((f) => ({ ...f, backend_id: v }))}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select a backend" />
                </SelectTrigger>
                <SelectContent>
                  {(backends ?? []).map((b) => (
                    <SelectItem key={b.id} value={b.id}>{b.name}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {errors["backend_id"] && <p className="text-sm text-destructive">{errors["backend_id"]}</p>}
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-2">
                <Label htmlFor="routePriority">Priority</Label>
                <Input
                  id="routePriority"
                  type="number"
                  min={0}
                  value={form.priority}
                  onChange={(e) => setForm((f) => ({ ...f, priority: parseInt(e.target.value) || 0 }))}
                />
              </div>
              <div className="flex items-center gap-3 pt-6">
                <Switch
                  checked={form.strip_prefix ?? false}
                  onCheckedChange={(v) => setForm((f) => ({ ...f, strip_prefix: v }))}
                />
                <Label>Strip prefix</Label>
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeForm}>Cancel</Button>
            <Button onClick={handleCreate} disabled={createMut.isPending}>
              {createMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirmation */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => { if (!open) setDeleteTarget(null) }}
        title="Delete Route"
        description={`Are you sure you want to delete route "${deleteTarget?.name}" (${deleteTarget?.path_pattern})? Traffic matching this pattern will fall through to the default handler. This action cannot be undone.`}
        confirmLabel="Delete"
        variant="destructive"
        loading={deleteMut.isPending}
        onConfirm={() => deleteTarget && deleteMut.mutate(deleteTarget.id)}
      />
    </div>
  )
}
