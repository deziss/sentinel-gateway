import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  listOrganizations, createOrganization, deleteOrganization,
  type Organization, type CreateOrganizationInput,
} from "@/lib/api"
import { validate, required } from "@/lib/fp-validate"
import { toast } from "@/hooks/use-toast"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import {
  Card, CardContent, CardHeader, CardTitle, CardDescription,
} from "@/components/ui/card"
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter, DialogDescription,
} from "@/components/ui/dialog"
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select"
import {
  Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
} from "@/components/ui/table"
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import { format } from "date-fns"
import { Plus, Trash2, Building2, AlertTriangle } from "lucide-react"
import { FeatureGate } from "@/components/FeatureGate"

export function Organizations() {
  return (
    <FeatureGate
      feature="org_management_enabled"
      title="Organizations"
      description="Group multiple tenants under parent organizations for unified billing and admin."
      requiredPlan="enterprise"
    >
      <OrganizationsInner />
    </FeatureGate>
  )
}

function OrganizationsInner() {
  const queryClient = useQueryClient()
  const [addOpen, setAddOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<Organization | null>(null)
  const [form, setForm] = useState<CreateOrganizationInput>({
    slug: "",
    name: "",
    plan: "free",
  })
  const [errors, setErrors] = useState<Record<string, string>>({})

  const { data: orgs, isLoading, isError, error } = useQuery({
    queryKey: ["organizations"],
    queryFn: listOrganizations,
  })

  const createMut = useMutation({
    mutationFn: createOrganization,
    onSuccess: (org) => {
      queryClient.invalidateQueries({ queryKey: ["organizations"] })
      toast({ title: "Organization created", description: `"${org.name}" is ready.` })
      closeAdd()
    },
    onError: (err: Error) => {
      toast({ title: "Failed to create organization", description: err.message, variant: "destructive" })
    },
  })

  const deleteMut = useMutation({
    mutationFn: (id: string) => deleteOrganization(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["organizations"] })
      toast({ title: "Organization deleted" })
      setDeleteTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to delete organization", description: err.message, variant: "destructive" })
      setDeleteTarget(null)
    },
  })

  function closeAdd() {
    setAddOpen(false)
    setForm({ slug: "", name: "", plan: "free" })
    setErrors({})
  }

  function handleSubmit() {
    const fieldErrors: Record<string, string> = {}
    pipe(
      validate(form.name, required("Name")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["name"] = e.message))),
    )
    pipe(
      validate(form.slug, required("Slug")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["slug"] = e.message))),
    )
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
        <h2 className="text-lg font-semibold">Failed to load organizations</h2>
        <p className="text-sm text-muted-foreground mt-1">
          {error instanceof Error ? error.message : "An unexpected error occurred"}
        </p>
      </div>
    )
  }

  const rows = orgs ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Organizations</h1>
          <p className="text-muted-foreground">
            Parent groupings for multiple tenants (billing, views, cross-tenant access)
          </p>
        </div>
        <Button onClick={() => setAddOpen(true)}>
          <Plus className="mr-2 h-4 w-4" /> New Organization
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Building2 className="h-4 w-4" /> All organizations
          </CardTitle>
          <CardDescription>SuperAdmin-only</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2">
              {[1, 2, 3].map((i) => <Skeleton key={i} className="h-10 w-full" />)}
            </div>
          ) : rows.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <Building2 className="h-10 w-10 text-muted-foreground mb-4" />
              <h3 className="text-lg font-semibold">No organizations yet</h3>
              <p className="text-sm text-muted-foreground mt-1">
                Group related tenants (staging/prod, customer accounts) under an organization.
              </p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Slug</TableHead>
                  <TableHead>Plan</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="w-12" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((o) => (
                  <TableRow key={o.id}>
                    <TableCell className="font-medium">{o.name}</TableCell>
                    <TableCell className="font-mono text-xs">{o.slug}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{o.plan}</Badge>
                    </TableCell>
                    <TableCell>
                      {o.is_active ? (
                        <Badge variant="success">Active</Badge>
                      ) : (
                        <Badge variant="outline">Inactive</Badge>
                      )}
                    </TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      {format(new Date(o.created_at), "MMM d, yyyy")}
                    </TableCell>
                    <TableCell>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setDeleteTarget(o)}
                        className="text-destructive hover:text-destructive"
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {/* ── New Org Dialog ───────────────────────────────── */}
      <Dialog open={addOpen} onOpenChange={(v) => !v && closeAdd()}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>New organization</DialogTitle>
            <DialogDescription>Create a parent grouping for related tenants.</DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 py-2">
            <div>
              <Label>Name</Label>
              <Input
                value={form.name}
                onChange={(e) => setForm({ ...form, name: e.target.value })}
                placeholder="Acme Corp"
              />
              {errors.name && <p className="text-xs text-destructive mt-1">{errors.name}</p>}
            </div>
            <div>
              <Label>Slug</Label>
              <Input
                value={form.slug}
                onChange={(e) => setForm({ ...form, slug: e.target.value })}
                placeholder="acme"
              />
              {errors.slug && <p className="text-xs text-destructive mt-1">{errors.slug}</p>}
            </div>
            <div>
              <Label>Plan</Label>
              <Select
                value={form.plan ?? "free"}
                onValueChange={(v) => setForm({ ...form, plan: v })}
              >
                <SelectTrigger><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="free">Free</SelectItem>
                  <SelectItem value="starter">Starter</SelectItem>
                  <SelectItem value="pro">Pro</SelectItem>
                  <SelectItem value="enterprise">Enterprise</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <DialogFooter>
            <Button variant="ghost" onClick={closeAdd}>Cancel</Button>
            <Button onClick={handleSubmit} disabled={createMut.isPending}>
              {createMut.isPending ? "Creating..." : "Create"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(v) => !v && setDeleteTarget(null)}
        title="Delete organization?"
        description={`This will soft-delete "${deleteTarget?.name}". Tenants will keep their data but lose their organization link.`}
        confirmLabel="Delete"
        variant="destructive"
        onConfirm={() => deleteTarget && deleteMut.mutate(deleteTarget.id)}
      />
    </div>
  )
}
