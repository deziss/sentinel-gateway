import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  listApiKeys,
  createApiKey,
  revokeApiKey,
  type ApiKeyMeta,
  type CreateApiKeyInput,
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
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Plus,
  Key,
  Copy,
  Check,
  Trash2,
  Loader2,
  AlertTriangle,
} from "lucide-react"
import { format } from "date-fns"

const SCOPES = ["read", "write", "admin", "proxy"]

export function ApiKeys() {
  const queryClient = useQueryClient()
  const [createOpen, setCreateOpen] = useState(false)
  const [form, setForm] = useState<CreateApiKeyInput>({ name: "", scopes: ["proxy"] })
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [revokeTarget, setRevokeTarget] = useState<ApiKeyMeta | null>(null)

  const { data: keys, isLoading, isError, error } = useQuery({
    queryKey: ["api-keys"],
    queryFn: listApiKeys,
  })

  const createMut = useMutation({
    mutationFn: (input: CreateApiKeyInput) => createApiKey(input),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["api-keys"] })
      setCreatedKey(data.key)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to create API key", description: err.message, variant: "destructive" })
    },
  })

  const revokeMut = useMutation({
    mutationFn: (id: string) => revokeApiKey(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["api-keys"] })
      toast({ title: "API key revoked", description: `"${revokeTarget?.name}" has been revoked.` })
      setRevokeTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to revoke key", description: err.message, variant: "destructive" })
      setRevokeTarget(null)
    },
  })

  function closeCreate() {
    setCreateOpen(false)
    setCreatedKey(null)
    setForm({ name: "", scopes: ["proxy"] })
    setErrors({})
    setCopied(false)
  }

  function handleCreate() {
    const fieldErrors: Record<string, string> = {}
    pipe(
      validate(form.name, required("Name")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["name"] = e.message)))
    )
    if (form.scopes.length === 0) fieldErrors["scopes"] = "Select at least one scope"
    if (Object.keys(fieldErrors).length > 0) {
      setErrors(fieldErrors)
      return
    }
    setErrors({})
    createMut.mutate(form)
  }

  function toggleScope(scope: string) {
    setForm((f) => ({
      ...f,
      scopes: f.scopes.includes(scope)
        ? f.scopes.filter((s) => s !== scope)
        : [...f.scopes, scope],
    }))
  }

  async function copyKey() {
    if (!createdKey) return
    await navigator.clipboard.writeText(createdKey)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load API keys</h2>
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
          <h1 className="text-2xl font-bold tracking-tight">API Keys</h1>
          <p className="text-muted-foreground">Manage virtual API keys for gateway access</p>
        </div>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="mr-2 h-4 w-4" /> Create Key
        </Button>
      </div>

      {isLoading ? (
        <Card>
          <CardContent className="p-6 space-y-3">
            {[1, 2, 3].map((i) => <Skeleton key={i} className="h-12 w-full" />)}
          </CardContent>
        </Card>
      ) : keys && keys.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <Key className="h-10 w-10 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold">No API keys</h3>
            <p className="text-sm text-muted-foreground mt-1">
              Create an API key to authenticate gateway requests.
            </p>
            <Button className="mt-4" onClick={() => setCreateOpen(true)}>
              <Plus className="mr-2 h-4 w-4" /> Create Key
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>All Keys ({keys?.length ?? 0})</CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Scopes</TableHead>
                  <TableHead>Rate Limit</TableHead>
                  <TableHead>Budget</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {keys?.map((k) => (
                  <TableRow key={k.id}>
                    <TableCell className="font-medium">{k.name}</TableCell>
                    <TableCell>
                      <div className="flex gap-1 flex-wrap">
                        {k.scopes.map((s) => (
                          <Badge key={s} variant="secondary" className="text-xs">
                            {s}
                          </Badge>
                        ))}
                      </div>
                    </TableCell>
                    <TableCell className="text-sm">
                      {k.rate_limit_rpm ? `${k.rate_limit_rpm} rpm` : "\u2014"}
                    </TableCell>
                    <TableCell className="text-sm">
                      {k.budget_monthly ? `$${k.budget_monthly}/mo` : k.budget_daily ? `$${k.budget_daily}/day` : "\u2014"}
                    </TableCell>
                    <TableCell>
                      <Badge variant={k.is_active ? "success" : "destructive"}>
                        {k.is_active ? "Active" : "Revoked"}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      {format(new Date(k.created_at), "MMM d, yyyy")}
                    </TableCell>
                    <TableCell className="text-right">
                      {k.is_active && (
                        <Button variant="ghost" size="icon" onClick={() => setRevokeTarget(k)}>
                          <Trash2 className="h-4 w-4 text-destructive" />
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      {/* Create Key Dialog */}
      <Dialog open={createOpen} onOpenChange={(open) => { if (!open) closeCreate() }}>
        <DialogContent className="sm:max-w-[450px]">
          <DialogHeader>
            <DialogTitle>{createdKey ? "API Key Created" : "Create API Key"}</DialogTitle>
            {createdKey && (
              <DialogDescription>
                Copy this key now. It will not be shown again.
              </DialogDescription>
            )}
          </DialogHeader>

          {createdKey ? (
            <div className="space-y-4 py-4">
              <div className="flex items-center gap-2">
                <Input value={createdKey} readOnly className="font-mono text-xs" />
                <Button variant="outline" size="icon" onClick={copyKey}>
                  {copied ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
                </Button>
              </div>
              <Button className="w-full" onClick={closeCreate}>Done</Button>
            </div>
          ) : (
            <>
              <div className="grid gap-4 py-4">
                <div className="grid gap-2">
                  <Label htmlFor="keyName">Key Name</Label>
                  <Input
                    id="keyName"
                    value={form.name}
                    onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
                    placeholder="production-api-key"
                  />
                  {errors["name"] && <p className="text-sm text-destructive">{errors["name"]}</p>}
                </div>
                <div className="grid gap-2">
                  <Label>Scopes</Label>
                  <div className="flex gap-2 flex-wrap">
                    {SCOPES.map((s) => (
                      <Badge
                        key={s}
                        variant={form.scopes.includes(s) ? "default" : "outline"}
                        className="cursor-pointer"
                        onClick={() => toggleScope(s)}
                      >
                        {s}
                      </Badge>
                    ))}
                  </div>
                  {errors["scopes"] && <p className="text-sm text-destructive">{errors["scopes"]}</p>}
                </div>
                <div className="grid grid-cols-2 gap-4">
                  <div className="grid gap-2">
                    <Label htmlFor="rateLimit">Rate Limit (rpm)</Label>
                    <Input
                      id="rateLimit"
                      type="number"
                      min={0}
                      placeholder="Optional"
                      value={form.rate_limit_rpm ?? ""}
                      onChange={(e) =>
                        setForm((f) => ({
                          ...f,
                          rate_limit_rpm: e.target.value ? parseInt(e.target.value) : null,
                        }))
                      }
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor="budgetMonthly">Monthly Budget ($)</Label>
                    <Input
                      id="budgetMonthly"
                      type="number"
                      min={0}
                      step={0.01}
                      placeholder="Optional"
                      value={form.budget_monthly ?? ""}
                      onChange={(e) =>
                        setForm((f) => ({
                          ...f,
                          budget_monthly: e.target.value ? parseFloat(e.target.value) : null,
                        }))
                      }
                    />
                  </div>
                </div>
              </div>
              <DialogFooter>
                <Button variant="outline" onClick={closeCreate}>Cancel</Button>
                <Button onClick={handleCreate} disabled={createMut.isPending}>
                  {createMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                  Create
                </Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>

      {/* Revoke Confirmation */}
      <ConfirmDialog
        open={!!revokeTarget}
        onOpenChange={(open) => { if (!open) setRevokeTarget(null) }}
        title="Revoke API Key"
        description={`Are you sure you want to revoke "${revokeTarget?.name}"? Any applications using this key will immediately lose access. This action cannot be undone.`}
        confirmLabel="Revoke"
        variant="destructive"
        loading={revokeMut.isPending}
        onConfirm={() => revokeTarget && revokeMut.mutate(revokeTarget.id)}
      />
    </div>
  )
}
