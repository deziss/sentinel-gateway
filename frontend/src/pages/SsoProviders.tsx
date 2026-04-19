import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  listSsoProviders, createSsoProvider, deleteSsoProvider,
  type SsoProvider, type SsoKind, type CreateSsoProviderInput,
} from "@/lib/api"
import { validate, required } from "@/lib/fp-validate"
import { toast } from "@/hooks/use-toast"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Badge } from "@/components/ui/badge"
import { Switch } from "@/components/ui/switch"
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
import { Plus, Trash2, KeyRound, AlertTriangle, Shield } from "lucide-react"
import { FeatureGate } from "@/components/FeatureGate"

const PROVIDER_KINDS: Array<{ value: SsoKind; label: string; requiresIssuer: boolean }> = [
  { value: "keycloak", label: "Keycloak (OIDC)", requiresIssuer: true },
  { value: "okta", label: "Okta (OIDC)", requiresIssuer: true },
  { value: "google", label: "Google", requiresIssuer: false },
  { value: "github", label: "GitHub", requiresIssuer: false },
  { value: "microsoft", label: "Microsoft / Entra", requiresIssuer: false },
  { value: "oidc_generic", label: "Generic OIDC", requiresIssuer: true },
]

export function SsoProviders() {
  return (
    <FeatureGate
      feature="sso_enabled"
      title="Single Sign-On (SSO)"
      description="OAuth2 / OIDC providers: Keycloak, Okta, Google, GitHub, Microsoft Entra."
      requiredPlan="enterprise"
    >
      <SsoProvidersInner />
    </FeatureGate>
  )
}

function SsoProvidersInner() {
  const queryClient = useQueryClient()
  const [addOpen, setAddOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<SsoProvider | null>(null)
  const [form, setForm] = useState<CreateSsoProviderInput>({
    kind: "keycloak",
    display_name: "",
    slug: "",
    client_id: "",
    client_secret: "",
    issuer_url: "",
    scopes: "openid profile email",
    default_role: "user",
    auto_provision: true,
  })
  const [errors, setErrors] = useState<Record<string, string>>({})

  const { data: providers, isLoading, isError, error } = useQuery({
    queryKey: ["sso-providers"],
    queryFn: listSsoProviders,
  })

  const createMut = useMutation({
    mutationFn: createSsoProvider,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sso-providers"] })
      toast({ title: "SSO provider created", description: `Users can now sign in via ${form.display_name}` })
      closeAdd()
    },
    onError: (err: Error) => {
      toast({ title: "Failed to create provider", description: err.message, variant: "destructive" })
    },
  })

  const deleteMut = useMutation({
    mutationFn: (id: string) => deleteSsoProvider(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sso-providers"] })
      toast({ title: "SSO provider deleted" })
      setDeleteTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to delete provider", description: err.message, variant: "destructive" })
    },
  })

  function closeAdd() {
    setAddOpen(false)
    setForm({
      kind: "keycloak",
      display_name: "",
      slug: "",
      client_id: "",
      client_secret: "",
      issuer_url: "",
      scopes: "openid profile email",
      default_role: "user",
      auto_provision: true,
    })
    setErrors({})
  }

  function handleSubmit() {
    const fieldErrors: Record<string, string> = {}
    pipe(
      validate(form.display_name, required("Display name")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["display_name"] = e.message))),
    )
    pipe(
      validate(form.slug, required("Slug")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["slug"] = e.message))),
    )
    pipe(
      validate(form.client_id, required("Client ID")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["client_id"] = e.message))),
    )
    pipe(
      validate(form.client_secret, required("Client secret")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["client_secret"] = e.message))),
    )
    const kindMeta = PROVIDER_KINDS.find((k) => k.value === form.kind)
    if (kindMeta?.requiresIssuer && !form.issuer_url) {
      fieldErrors["issuer_url"] = "Issuer URL required for this provider"
    }
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
        <h2 className="text-lg font-semibold">Failed to load SSO providers</h2>
        <p className="text-sm text-muted-foreground mt-1">
          {error instanceof Error ? error.message : "An unexpected error occurred"}
        </p>
      </div>
    )
  }

  const rows = providers ?? []
  const kindMeta = PROVIDER_KINDS.find((k) => k.value === form.kind)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">SSO Providers</h1>
          <p className="text-muted-foreground">
            OAuth2 / OIDC identity providers for this tenant
          </p>
        </div>
        <Button onClick={() => setAddOpen(true)}>
          <Plus className="mr-2 h-4 w-4" /> Add Provider
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield className="h-4 w-4" /> Configured providers
          </CardTitle>
          <CardDescription>
            Active providers are listed on the login page at <code>/login</code>
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2">
              {[1, 2, 3].map((i) => <Skeleton key={i} className="h-10 w-full" />)}
            </div>
          ) : rows.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <KeyRound className="h-10 w-10 text-muted-foreground mb-4" />
              <h3 className="text-lg font-semibold">No SSO providers configured</h3>
              <p className="text-sm text-muted-foreground mt-1">
                Add a provider to let users sign in with Keycloak, Okta, Google, GitHub, or Microsoft.
              </p>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Slug</TableHead>
                  <TableHead>Kind</TableHead>
                  <TableHead>Auto-provision</TableHead>
                  <TableHead>Default role</TableHead>
                  <TableHead className="w-12" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((p) => (
                  <TableRow key={p.id}>
                    <TableCell className="font-medium">{p.display_name}</TableCell>
                    <TableCell className="font-mono text-xs">{p.slug}</TableCell>
                    <TableCell>
                      <Badge variant="outline">{p.kind}</Badge>
                    </TableCell>
                    <TableCell>
                      {p.auto_provision ? (
                        <Badge variant="success">Yes</Badge>
                      ) : (
                        <Badge variant="outline">No</Badge>
                      )}
                    </TableCell>
                    <TableCell>{p.default_role ?? "user"}</TableCell>
                    <TableCell>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setDeleteTarget(p)}
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

      {/* ── Add Provider Dialog ──────────────────────────── */}
      <Dialog open={addOpen} onOpenChange={(v) => !v && closeAdd()}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>Add SSO provider</DialogTitle>
            <DialogDescription>
              Configure a new OAuth2 / OIDC identity provider.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 py-2">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <Label>Provider kind</Label>
                <Select
                  value={form.kind}
                  onValueChange={(v) => setForm({ ...form, kind: v as SsoKind })}
                >
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    {PROVIDER_KINDS.map((k) => (
                      <SelectItem key={k.value} value={k.value}>{k.label}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div>
                <Label>Display name</Label>
                <Input
                  value={form.display_name}
                  onChange={(e) => setForm({ ...form, display_name: e.target.value })}
                  placeholder="Acme Keycloak"
                />
                {errors.display_name && <p className="text-xs text-destructive mt-1">{errors.display_name}</p>}
              </div>
            </div>

            <div>
              <Label>Slug (URL path segment)</Label>
              <Input
                value={form.slug}
                onChange={(e) => setForm({ ...form, slug: e.target.value })}
                placeholder="acme-keycloak"
              />
              <p className="text-xs text-muted-foreground mt-1">
                Used in redirects: <code>/auth/sso/{form.slug || "<slug>"}/authorize</code>
              </p>
              {errors.slug && <p className="text-xs text-destructive mt-1">{errors.slug}</p>}
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <Label>Client ID</Label>
                <Input
                  value={form.client_id}
                  onChange={(e) => setForm({ ...form, client_id: e.target.value })}
                />
                {errors.client_id && <p className="text-xs text-destructive mt-1">{errors.client_id}</p>}
              </div>
              <div>
                <Label>Client secret</Label>
                <Input
                  type="password"
                  value={form.client_secret}
                  onChange={(e) => setForm({ ...form, client_secret: e.target.value })}
                />
                {errors.client_secret && <p className="text-xs text-destructive mt-1">{errors.client_secret}</p>}
              </div>
            </div>

            {kindMeta?.requiresIssuer && (
              <div>
                <Label>Issuer URL</Label>
                <Input
                  value={form.issuer_url ?? ""}
                  onChange={(e) => setForm({ ...form, issuer_url: e.target.value })}
                  placeholder="https://keycloak.example.com/realms/acme"
                />
                {errors.issuer_url && <p className="text-xs text-destructive mt-1">{errors.issuer_url}</p>}
              </div>
            )}

            <div className="grid grid-cols-2 gap-4">
              <div>
                <Label>Scopes (space-separated)</Label>
                <Input
                  value={form.scopes ?? ""}
                  onChange={(e) => setForm({ ...form, scopes: e.target.value })}
                  placeholder="openid profile email"
                />
              </div>
              <div>
                <Label>Default role</Label>
                <Select
                  value={form.default_role ?? "user"}
                  onValueChange={(v) => setForm({ ...form, default_role: v })}
                >
                  <SelectTrigger><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="user">User</SelectItem>
                    <SelectItem value="tenant_admin">Tenant Admin</SelectItem>
                    <SelectItem value="read_only">Read-only</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            <div className="flex items-center justify-between rounded-lg border p-3">
              <div>
                <Label>Auto-provision users</Label>
                <p className="text-xs text-muted-foreground">
                  Create a local user on first login if email is unknown
                </p>
              </div>
              <Switch
                checked={!!form.auto_provision}
                onCheckedChange={(v) => setForm({ ...form, auto_provision: v })}
              />
            </div>
          </div>

          <DialogFooter>
            <Button variant="ghost" onClick={closeAdd}>Cancel</Button>
            <Button onClick={handleSubmit} disabled={createMut.isPending}>
              {createMut.isPending ? "Creating..." : "Create Provider"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <ConfirmDialog
        open={deleteTarget !== null}
        onOpenChange={(v) => !v && setDeleteTarget(null)}
        title="Delete SSO provider?"
        description={`This will disable SSO login via "${deleteTarget?.display_name}". Existing linked users keep their accounts but lose this sign-in method.`}
        confirmLabel="Delete"
        variant="destructive"
        onConfirm={() => deleteTarget && deleteMut.mutate(deleteTarget.id)}
      />
    </div>
  )
}
