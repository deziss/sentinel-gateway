import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  listBackends,
  createBackend,
  updateBackend,
  deleteBackend,
  type Backend,
  type CreateBackendInput,
} from "@/lib/api"
import { validate, required, url as urlValidator } from "@/lib/fp-validate"
import { toast } from "@/hooks/use-toast"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
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
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Plus,
  Pencil,
  Trash2,
  Server,
  Loader2,
  AlertTriangle,
} from "lucide-react"

const PROVIDERS = [
  "open_ai",
  "anthropic",
  "google_vertex",
  "aws_bedrock",
  "ollama",
  "vllm",
  "open_ai_compatible",
  "rest",
  "graphql",
  "grpc",
  "generic",
]

const emptyForm: CreateBackendInput = {
  name: "",
  provider_type: "open_ai",
  endpoint: "",
  priority: 1,
  weight: 100,
  timeout_ms: 30000,
  max_retries: 2,
}

export function Backends() {
  const queryClient = useQueryClient()
  const [formOpen, setFormOpen] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [form, setForm] = useState<CreateBackendInput>(emptyForm)
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [deleteTarget, setDeleteTarget] = useState<Backend | null>(null)

  const { data: backends, isLoading, isError, error } = useQuery({
    queryKey: ["backends"],
    queryFn: listBackends,
  })

  const createMut = useMutation({
    mutationFn: (input: CreateBackendInput) => createBackend(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["backends"] })
      toast({ title: "Backend created", description: `"${form.name}" has been added.` })
      closeForm()
    },
    onError: (err: Error) => {
      toast({ title: "Failed to create backend", description: err.message, variant: "destructive" })
    },
  })

  const updateMut = useMutation({
    mutationFn: ({ id, input }: { id: string; input: Partial<CreateBackendInput> }) =>
      updateBackend(id, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["backends"] })
      toast({ title: "Backend updated" })
      closeForm()
    },
    onError: (err: Error) => {
      toast({ title: "Failed to update backend", description: err.message, variant: "destructive" })
    },
  })

  const deleteMut = useMutation({
    mutationFn: (id: string) => deleteBackend(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["backends"] })
      toast({ title: "Backend deleted", description: `"${deleteTarget?.name}" has been removed.` })
      setDeleteTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to delete backend", description: err.message, variant: "destructive" })
      setDeleteTarget(null)
    },
  })

  function closeForm() {
    setFormOpen(false)
    setEditingId(null)
    setForm(emptyForm)
    setErrors({})
  }

  function openEdit(b: Backend) {
    setEditingId(b.id)
    setForm({
      name: b.name,
      provider_type: b.provider_type,
      endpoint: b.endpoint,
      priority: b.priority,
      weight: b.weight,
      timeout_ms: b.timeout_ms,
      max_retries: b.max_retries,
    })
    setFormOpen(true)
  }

  function validateAndSubmit() {
    const fieldErrors: Record<string, string> = {}

    pipe(
      validate(form.name, required("Name")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors[e.field.toLowerCase()] = e.message)))
    )
    pipe(
      validate(form.endpoint, required("Endpoint"), urlValidator("Endpoint")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["endpoint"] = e.message)))
    )
    if ((form.priority ?? 1) < 0) fieldErrors["priority"] = "Priority must be >= 0"
    if ((form.weight ?? 1) < 1) fieldErrors["weight"] = "Weight must be >= 1"

    if (Object.keys(fieldErrors).length > 0) {
      setErrors(fieldErrors)
      return
    }
    setErrors({})

    if (editingId) {
      updateMut.mutate({ id: editingId, input: form })
    } else {
      createMut.mutate(form)
    }
  }

  const isSaving = createMut.isPending || updateMut.isPending

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load backends</h2>
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
          <h1 className="text-2xl font-bold tracking-tight">Backends</h1>
          <p className="text-muted-foreground">Manage upstream service providers</p>
        </div>
        <Button onClick={() => { setForm(emptyForm); setFormOpen(true) }}>
          <Plus className="mr-2 h-4 w-4" /> Add Backend
        </Button>
      </div>

      {isLoading ? (
        <Card>
          <CardContent className="p-6 space-y-3">
            {[1, 2, 3].map((i) => <Skeleton key={i} className="h-12 w-full" />)}
          </CardContent>
        </Card>
      ) : backends && backends.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <Server className="h-10 w-10 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold">No backends configured</h3>
            <p className="text-sm text-muted-foreground mt-1">
              Add your first upstream provider to start routing requests.
            </p>
            <Button className="mt-4" onClick={() => setFormOpen(true)}>
              <Plus className="mr-2 h-4 w-4" /> Add Backend
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>All Backends ({backends?.length ?? 0})</CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Provider</TableHead>
                  <TableHead>Endpoint</TableHead>
                  <TableHead>Health</TableHead>
                  <TableHead>Priority</TableHead>
                  <TableHead>Weight</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {backends?.map((b) => (
                  <TableRow key={b.id}>
                    <TableCell className="font-medium">{b.name}</TableCell>
                    <TableCell>
                      <Badge variant="outline" className="capitalize">
                        {b.provider_type.replace(/_/g, " ")}
                      </Badge>
                    </TableCell>
                    <TableCell className="font-mono text-xs max-w-[200px] truncate">
                      {b.endpoint}
                    </TableCell>
                    <TableCell>
                      <Badge variant={b.health_status === "healthy" ? "success" : b.health_status === "degraded" ? "warning" : "destructive"}>
                        {b.health_status ?? "unknown"}
                      </Badge>
                    </TableCell>
                    <TableCell>{b.priority}</TableCell>
                    <TableCell>{b.weight}</TableCell>
                    <TableCell className="text-right">
                      <div className="flex justify-end gap-1">
                        <Button variant="ghost" size="icon" onClick={() => openEdit(b)}>
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button variant="ghost" size="icon" onClick={() => setDeleteTarget(b)}>
                          <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      {/* Create / Edit Dialog */}
      <Dialog open={formOpen} onOpenChange={(open) => { if (!open) closeForm() }}>
        <DialogContent className="sm:max-w-[500px]">
          <DialogHeader>
            <DialogTitle>{editingId ? "Edit Backend" : "Add Backend"}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="name">Name</Label>
              <Input
                id="name"
                value={form.name}
                onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
                placeholder="My OpenAI Backend"
              />
              {errors["name"] && <p className="text-sm text-destructive">{errors["name"]}</p>}
            </div>
            <div className="grid gap-2">
              <Label htmlFor="provider">Provider Type</Label>
              <Select
                value={form.provider_type}
                onValueChange={(v) => setForm((f) => ({ ...f, provider_type: v }))}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PROVIDERS.map((p) => (
                    <SelectItem key={p} value={p} className="capitalize">
                      {p.replace(/_/g, " ")}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="grid gap-2">
              <Label htmlFor="endpoint">Endpoint URL</Label>
              <Input
                id="endpoint"
                value={form.endpoint}
                onChange={(e) => setForm((f) => ({ ...f, endpoint: e.target.value }))}
                placeholder="https://api.openai.com/v1"
              />
              {errors["endpoint"] && <p className="text-sm text-destructive">{errors["endpoint"]}</p>}
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-2">
                <Label htmlFor="priority">Priority (lower = higher)</Label>
                <Input
                  id="priority"
                  type="number"
                  min={0}
                  value={form.priority}
                  onChange={(e) => setForm((f) => ({ ...f, priority: parseInt(e.target.value) || 0 }))}
                />
                {errors["priority"] && <p className="text-sm text-destructive">{errors["priority"]}</p>}
              </div>
              <div className="grid gap-2">
                <Label htmlFor="weight">Weight</Label>
                <Input
                  id="weight"
                  type="number"
                  min={1}
                  value={form.weight}
                  onChange={(e) => setForm((f) => ({ ...f, weight: parseInt(e.target.value) || 1 }))}
                />
                {errors["weight"] && <p className="text-sm text-destructive">{errors["weight"]}</p>}
              </div>
            </div>
            <div className="grid grid-cols-2 gap-4">
              <div className="grid gap-2">
                <Label htmlFor="timeout">Timeout (ms)</Label>
                <Input
                  id="timeout"
                  type="number"
                  min={1000}
                  value={form.timeout_ms}
                  onChange={(e) => setForm((f) => ({ ...f, timeout_ms: parseInt(e.target.value) || 30000 }))}
                />
              </div>
              <div className="grid gap-2">
                <Label htmlFor="retries">Max Retries</Label>
                <Input
                  id="retries"
                  type="number"
                  min={0}
                  max={5}
                  value={form.max_retries}
                  onChange={(e) => setForm((f) => ({ ...f, max_retries: parseInt(e.target.value) || 0 }))}
                />
              </div>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeForm}>Cancel</Button>
            <Button onClick={validateAndSubmit} disabled={isSaving}>
              {isSaving && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              {editingId ? "Update" : "Create"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete Confirmation */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => { if (!open) setDeleteTarget(null) }}
        title="Delete Backend"
        description={`Are you sure you want to delete "${deleteTarget?.name}"? Routes pointing to this backend will stop working. This action cannot be undone.`}
        confirmLabel="Delete"
        variant="destructive"
        loading={deleteMut.isPending}
        onConfirm={() => deleteTarget && deleteMut.mutate(deleteTarget.id)}
      />
    </div>
  )
}
