import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  listUsers,
  inviteUser,
  updateUser,
  deactivateUser,
  type User,
  type InviteUserInput,
} from "@/lib/api"
import { validate, required, email as emailValidator, minLength } from "@/lib/fp-validate"
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
  DialogDescription,
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
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  UserPlus,
  Shield,
  Mail,
  Loader2,
  Trash2,
  Pencil,
  Users as UsersIcon,
  AlertTriangle,
} from "lucide-react"
import { format } from "date-fns"

const ROLES = [
  { value: "tenant_admin", label: "Admin" },
  { value: "user", label: "User" },
  { value: "read_only", label: "Read Only" },
]

const STATUS_MAP: Record<string, { label: string; variant: "success" | "warning" | "destructive" | "secondary" }> = {
  active: { label: "Active", variant: "success" },
  inactive: { label: "Inactive", variant: "secondary" },
  locked: { label: "Locked", variant: "destructive" },
  pending: { label: "Pending", variant: "warning" },
}

export function Users() {
  const queryClient = useQueryClient()
  const [inviteOpen, setInviteOpen] = useState(false)
  const [inviteForm, setInviteForm] = useState<InviteUserInput>({ email: "", password: "", role: "user" })
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [deactivateTarget, setDeactivateTarget] = useState<User | null>(null)
  const [editTarget, setEditTarget] = useState<User | null>(null)
  const [editRole, setEditRole] = useState("")

  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["users"],
    queryFn: listUsers,
  })

  const users = data?.users ?? []

  const inviteMut = useMutation({
    mutationFn: (input: InviteUserInput) => inviteUser(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] })
      toast({ title: "User invited", description: `${inviteForm.email} has been added.` })
      closeInvite()
    },
    onError: (err: Error) => {
      toast({ title: "Failed to invite user", description: err.message, variant: "destructive" })
    },
  })

  const updateMut = useMutation({
    mutationFn: ({ id, role }: { id: string; role: string }) => updateUser(id, { role }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] })
      toast({ title: "Role updated" })
      setEditTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to update user", description: err.message, variant: "destructive" })
    },
  })

  const deactivateMut = useMutation({
    mutationFn: (id: string) => deactivateUser(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] })
      toast({ title: "User deactivated", description: `${deactivateTarget?.email} has been deactivated.` })
      setDeactivateTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to deactivate user", description: err.message, variant: "destructive" })
      setDeactivateTarget(null)
    },
  })

  function closeInvite() {
    setInviteOpen(false)
    setInviteForm({ email: "", password: "", role: "user" })
    setErrors({})
  }

  function handleInvite() {
    const fieldErrors: Record<string, string> = {}
    pipe(
      validate(inviteForm.email, required("Email"), emailValidator("Email")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["email"] = e.message)))
    )
    pipe(
      validate(inviteForm.password, required("Password"), minLength("Password", 8)),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["password"] = e.message)))
    )
    if (Object.keys(fieldErrors).length > 0) {
      setErrors(fieldErrors)
      return
    }
    setErrors({})
    inviteMut.mutate(inviteForm)
  }

  function openEditRole(user: User) {
    setEditTarget(user)
    setEditRole(user.role)
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load users</h2>
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
          <h1 className="text-2xl font-bold tracking-tight">Users</h1>
          <p className="text-muted-foreground">Manage team members and access roles</p>
        </div>
        <Button onClick={() => setInviteOpen(true)}>
          <UserPlus className="mr-2 h-4 w-4" /> Invite User
        </Button>
      </div>

      {isLoading ? (
        <Card>
          <CardContent className="p-6 space-y-3">
            {[1, 2, 3].map((i) => <Skeleton key={i} className="h-14 w-full" />)}
          </CardContent>
        </Card>
      ) : users.length === 0 ? (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12 text-center">
            <UsersIcon className="h-10 w-10 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold">No users yet</h3>
            <p className="text-sm text-muted-foreground mt-1">
              Invite your first team member to get started.
            </p>
            <Button className="mt-4" onClick={() => setInviteOpen(true)}>
              <UserPlus className="mr-2 h-4 w-4" /> Invite User
            </Button>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader>
            <CardTitle>Team Members ({users.length})</CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>User</TableHead>
                  <TableHead>Role</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Last Login</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {users.map((user) => {
                  const statusInfo = STATUS_MAP[user.status] ?? { label: user.status, variant: "secondary" as const }
                  return (
                    <TableRow key={user.id}>
                      <TableCell>
                        <div className="flex items-center gap-3">
                          <div className="h-8 w-8 rounded-full bg-primary/10 flex items-center justify-center text-primary font-semibold text-xs">
                            {user.email[0]?.toUpperCase() ?? "U"}
                          </div>
                          <div className="flex flex-col">
                            <span className="font-medium text-sm">{user.email.split("@")[0]}</span>
                            <span className="text-xs text-muted-foreground flex items-center gap-1">
                              <Mail className="h-3 w-3" /> {user.email}
                            </span>
                          </div>
                        </div>
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-1.5">
                          <Shield className="h-3.5 w-3.5 text-muted-foreground" />
                          <span className="text-sm capitalize">{user.role.replace(/_/g, " ")}</span>
                        </div>
                      </TableCell>
                      <TableCell>
                        <Badge variant={statusInfo.variant}>{statusInfo.label}</Badge>
                      </TableCell>
                      <TableCell className="text-sm text-muted-foreground">
                        {user.last_login_at
                          ? format(new Date(user.last_login_at), "MMM d, yyyy HH:mm")
                          : "Never"}
                      </TableCell>
                      <TableCell className="text-right">
                        <div className="flex justify-end gap-1">
                          <Button variant="ghost" size="icon" onClick={() => openEditRole(user)}>
                            <Pencil className="h-4 w-4" />
                          </Button>
                          {user.status === "active" && (
                            <Button variant="ghost" size="icon" onClick={() => setDeactivateTarget(user)}>
                              <Trash2 className="h-4 w-4 text-destructive" />
                            </Button>
                          )}
                        </div>
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      {/* Invite User Dialog */}
      <Dialog open={inviteOpen} onOpenChange={(open) => { if (!open) closeInvite() }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Invite User</DialogTitle>
            <DialogDescription>Add a new team member to this tenant.</DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="invEmail">Email</Label>
              <Input
                id="invEmail"
                type="email"
                value={inviteForm.email}
                onChange={(e) => setInviteForm((f) => ({ ...f, email: e.target.value }))}
                placeholder="name@company.com"
              />
              {errors["email"] && <p className="text-sm text-destructive">{errors["email"]}</p>}
            </div>
            <div className="grid gap-2">
              <Label htmlFor="invPassword">Initial Password</Label>
              <Input
                id="invPassword"
                type="password"
                value={inviteForm.password}
                onChange={(e) => setInviteForm((f) => ({ ...f, password: e.target.value }))}
                placeholder="Min 8 characters"
              />
              {errors["password"] && <p className="text-sm text-destructive">{errors["password"]}</p>}
            </div>
            <div className="grid gap-2">
              <Label>Role</Label>
              <Select
                value={inviteForm.role ?? "user"}
                onValueChange={(v) => setInviteForm((f) => ({ ...f, role: v }))}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {ROLES.map((r) => (
                    <SelectItem key={r.value} value={r.value}>{r.label}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={closeInvite}>Cancel</Button>
            <Button onClick={handleInvite} disabled={inviteMut.isPending}>
              {inviteMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Invite
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Edit Role Dialog */}
      <Dialog open={!!editTarget} onOpenChange={(open) => { if (!open) setEditTarget(null) }}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Change Role</DialogTitle>
            <DialogDescription>Update the role for {editTarget?.email}</DialogDescription>
          </DialogHeader>
          <div className="py-4">
            <Select value={editRole} onValueChange={setEditRole}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {ROLES.map((r) => (
                  <SelectItem key={r.value} value={r.value}>{r.label}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setEditTarget(null)}>Cancel</Button>
            <Button
              onClick={() => editTarget && updateMut.mutate({ id: editTarget.id, role: editRole })}
              disabled={updateMut.isPending || editRole === editTarget?.role}
            >
              {updateMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Deactivate Confirmation */}
      <ConfirmDialog
        open={!!deactivateTarget}
        onOpenChange={(open) => { if (!open) setDeactivateTarget(null) }}
        title="Deactivate User"
        description={`Are you sure you want to deactivate "${deactivateTarget?.email}"? They will immediately lose access to the gateway. This action can be reversed by an admin.`}
        confirmLabel="Deactivate"
        variant="destructive"
        loading={deactivateMut.isPending}
        onConfirm={() => deactivateTarget && deactivateMut.mutate(deactivateTarget.id)}
      />
    </div>
  )
}
