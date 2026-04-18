import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  getSettings,
  updateSettings,
  listWebhooks,
  createWebhook,
  deleteWebhook,
  testWebhook,
  type Webhook,
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
  CardFooter,
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  Building2,
  Webhook as WebhookIcon,
  Plus,
  Trash2,
  Send,
  Loader2,
  Copy,
  Check,
} from "lucide-react"

export function Settings() {
  const queryClient = useQueryClient()
  const [orgName, setOrgName] = useState("")
  const [orgLoaded, setOrgLoaded] = useState(false)
  const [webhookOpen, setWebhookOpen] = useState(false)
  const [webhookUrl, setWebhookUrl] = useState("")
  const [webhookEvents, setWebhookEvents] = useState("*")
  const [webhookErrors, setWebhookErrors] = useState<Record<string, string>>({})
  const [deleteTarget, setDeleteTarget] = useState<Webhook | null>(null)
  const [copiedSecret, setCopiedSecret] = useState(false)
  const [newWebhookSecret, setNewWebhookSecret] = useState<string | null>(null)

  const { data: settings, isLoading: settingsLoading } = useQuery({
    queryKey: ["settings"],
    queryFn: getSettings,
  })

  const { data: webhooksData, isLoading: webhooksLoading } = useQuery({
    queryKey: ["webhooks"],
    queryFn: listWebhooks,
  })

  const webhooks = webhooksData?.webhooks ?? []

  // Initialize orgName from settings once loaded
  if (settings && !orgLoaded) {
    setOrgName(settings["org_name"] ?? "")
    setOrgLoaded(true)
  }

  const updateSettingsMut = useMutation({
    mutationFn: (s: Record<string, string>) => updateSettings(s),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["settings"] })
      toast({ title: "Settings saved" })
    },
    onError: (err: Error) => {
      toast({ title: "Failed to save settings", description: err.message, variant: "destructive" })
    },
  })

  const createWebhookMut = useMutation({
    mutationFn: ({ url, events }: { url: string; events: string[] }) => createWebhook(url, events),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["webhooks"] })
      if (data.secret) {
        setNewWebhookSecret(data.secret)
      } else {
        toast({ title: "Webhook created" })
        closeWebhookForm()
      }
    },
    onError: (err: Error) => {
      toast({ title: "Failed to create webhook", description: err.message, variant: "destructive" })
    },
  })

  const deleteWebhookMut = useMutation({
    mutationFn: (id: string) => deleteWebhook(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["webhooks"] })
      toast({ title: "Webhook deleted" })
      setDeleteTarget(null)
    },
    onError: (err: Error) => {
      toast({ title: "Failed to delete webhook", description: err.message, variant: "destructive" })
      setDeleteTarget(null)
    },
  })

  const testWebhookMut = useMutation({
    mutationFn: (id: string) => testWebhook(id),
    onSuccess: () => {
      toast({ title: "Test event sent" })
    },
    onError: (err: Error) => {
      toast({ title: "Test failed", description: err.message, variant: "destructive" })
    },
  })

  function closeWebhookForm() {
    setWebhookOpen(false)
    setWebhookUrl("")
    setWebhookEvents("*")
    setWebhookErrors({})
    setNewWebhookSecret(null)
    setCopiedSecret(false)
  }

  function handleCreateWebhook() {
    const fieldErrors: Record<string, string> = {}
    pipe(
      validate(webhookUrl, required("URL"), urlValidator("URL")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["url"] = e.message)))
    )
    if (Object.keys(fieldErrors).length > 0) {
      setWebhookErrors(fieldErrors)
      return
    }
    setWebhookErrors({})
    const events = webhookEvents.split(",").map((e) => e.trim()).filter(Boolean)
    createWebhookMut.mutate({ url: webhookUrl, events })
  }

  async function copySecret() {
    if (!newWebhookSecret) return
    await navigator.clipboard.writeText(newWebhookSecret)
    setCopiedSecret(true)
    setTimeout(() => setCopiedSecret(false), 2000)
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
        <p className="text-muted-foreground">Manage tenant configuration and integrations</p>
      </div>

      <Tabs defaultValue="general">
        <TabsList>
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="webhooks">Webhooks</TabsTrigger>
        </TabsList>

        {/* General Settings */}
        <TabsContent value="general" className="space-y-6">
          {settingsLoading ? (
            <Card>
              <CardContent className="p-6 space-y-3">
                <Skeleton className="h-10 w-full" />
                <Skeleton className="h-10 w-full" />
              </CardContent>
            </Card>
          ) : (
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Building2 className="h-5 w-5" /> Tenant Identity
                </CardTitle>
                <CardDescription>Basic information about your gateway instance.</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid gap-2">
                  <Label htmlFor="orgName">Organization Name</Label>
                  <Input
                    id="orgName"
                    value={orgName}
                    onChange={(e) => setOrgName(e.target.value)}
                    placeholder="My Organization"
                  />
                </div>
                {settings && (
                  <div className="grid gap-4 pt-2">
                    {Object.entries(settings)
                      .filter(([key]) => key !== "org_name")
                      .map(([key, value]) => (
                        <div key={key} className="flex items-center justify-between text-sm">
                          <span className="text-muted-foreground font-mono">{key}</span>
                          <span className="font-medium">{value}</span>
                        </div>
                      ))}
                  </div>
                )}
              </CardContent>
              <CardFooter className="border-t pt-4">
                <Button
                  onClick={() => updateSettingsMut.mutate({ org_name: orgName })}
                  disabled={updateSettingsMut.isPending}
                >
                  {updateSettingsMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                  Save Changes
                </Button>
              </CardFooter>
            </Card>
          )}
        </TabsContent>

        {/* Webhooks */}
        <TabsContent value="webhooks" className="space-y-6">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-lg font-semibold">Webhook Endpoints</h2>
              <p className="text-sm text-muted-foreground">Receive audit events via HTTP POST</p>
            </div>
            <Button onClick={() => setWebhookOpen(true)}>
              <Plus className="mr-2 h-4 w-4" /> Add Webhook
            </Button>
          </div>

          {webhooksLoading ? (
            <Card>
              <CardContent className="p-6 space-y-3">
                {[1, 2].map((i) => <Skeleton key={i} className="h-12 w-full" />)}
              </CardContent>
            </Card>
          ) : webhooks.length === 0 ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-12 text-center">
                <WebhookIcon className="h-10 w-10 text-muted-foreground mb-4" />
                <h3 className="text-lg font-semibold">No webhooks configured</h3>
                <p className="text-sm text-muted-foreground mt-1">
                  Add a webhook to receive audit events in real time.
                </p>
              </CardContent>
            </Card>
          ) : (
            <Card>
              <CardContent className="pt-6">
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>URL</TableHead>
                      <TableHead>Events</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {webhooks.map((wh) => (
                      <TableRow key={wh.id}>
                        <TableCell className="font-mono text-xs max-w-[250px] truncate">
                          {wh.url}
                        </TableCell>
                        <TableCell>
                          <div className="flex gap-1 flex-wrap">
                            {wh.events.map((ev) => (
                              <Badge key={ev} variant="secondary" className="text-xs">{ev}</Badge>
                            ))}
                          </div>
                        </TableCell>
                        <TableCell>
                          <Badge variant={wh.is_active ? "success" : "secondary"}>
                            {wh.is_active ? "Active" : "Inactive"}
                          </Badge>
                        </TableCell>
                        <TableCell className="text-right">
                          <div className="flex justify-end gap-1">
                            <Button
                              variant="ghost"
                              size="icon"
                              onClick={() => testWebhookMut.mutate(wh.id)}
                              disabled={testWebhookMut.isPending}
                            >
                              <Send className="h-4 w-4" />
                            </Button>
                            <Button variant="ghost" size="icon" onClick={() => setDeleteTarget(wh)}>
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
        </TabsContent>
      </Tabs>

      {/* Create Webhook Dialog */}
      <Dialog open={webhookOpen} onOpenChange={(open) => { if (!open) closeWebhookForm() }}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{newWebhookSecret ? "Webhook Created" : "Add Webhook"}</DialogTitle>
          </DialogHeader>

          {newWebhookSecret ? (
            <div className="space-y-4 py-4">
              <p className="text-sm text-muted-foreground">
                Copy the signing secret now. It will not be shown again.
              </p>
              <div className="flex items-center gap-2">
                <Input value={newWebhookSecret} readOnly className="font-mono text-xs" />
                <Button variant="outline" size="icon" onClick={copySecret}>
                  {copiedSecret ? <Check className="h-4 w-4 text-green-500" /> : <Copy className="h-4 w-4" />}
                </Button>
              </div>
              <Button className="w-full" onClick={closeWebhookForm}>Done</Button>
            </div>
          ) : (
            <>
              <div className="grid gap-4 py-4">
                <div className="grid gap-2">
                  <Label htmlFor="whUrl">Endpoint URL</Label>
                  <Input
                    id="whUrl"
                    value={webhookUrl}
                    onChange={(e) => setWebhookUrl(e.target.value)}
                    placeholder="https://example.com/webhook"
                  />
                  {webhookErrors["url"] && <p className="text-sm text-destructive">{webhookErrors["url"]}</p>}
                </div>
                <div className="grid gap-2">
                  <Label htmlFor="whEvents">Events (comma-separated, * for all)</Label>
                  <Input
                    id="whEvents"
                    value={webhookEvents}
                    onChange={(e) => setWebhookEvents(e.target.value)}
                    placeholder="*"
                  />
                </div>
              </div>
              <DialogFooter>
                <Button variant="outline" onClick={closeWebhookForm}>Cancel</Button>
                <Button onClick={handleCreateWebhook} disabled={createWebhookMut.isPending}>
                  {createWebhookMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                  Create
                </Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>

      {/* Delete Webhook Confirmation */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(open) => { if (!open) setDeleteTarget(null) }}
        title="Delete Webhook"
        description={`Are you sure you want to delete the webhook for "${deleteTarget?.url}"? You will stop receiving events at this endpoint. This action cannot be undone.`}
        confirmLabel="Delete"
        variant="destructive"
        loading={deleteWebhookMut.isPending}
        onConfirm={() => deleteTarget && deleteWebhookMut.mutate(deleteTarget.id)}
      />
    </div>
  )
}
