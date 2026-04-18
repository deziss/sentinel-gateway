import { useState } from "react"
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query"
import { pipe } from "fp-ts/function"
import * as E from "fp-ts/Either"
import {
  listPromptNames,
  listPromptVersions,
  listPromptDeployments,
  createPrompt,
  deployPrompt,
  deletePromptVersion,
  resolvePrompt,
  type Prompt,
  type PromptDeployment,
} from "@/lib/api"
import { validate, required } from "@/lib/fp-validate"
import { toast } from "@/hooks/use-toast"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
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
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Plus,
  FileText,
  Rocket,
  Trash2,
  Loader2,
  AlertTriangle,
  Play,
  Eye,
} from "lucide-react"
import { format } from "date-fns"

const DEFAULT_LABELS = ["prod", "staging", "canary", "dev"]

export function Prompts() {
  const queryClient = useQueryClient()
  const [createOpen, setCreateOpen] = useState(false)
  const [selectedName, setSelectedName] = useState<string | null>(null)
  const [deployOpen, setDeployOpen] = useState<Prompt | null>(null)
  const [viewOpen, setViewOpen] = useState<Prompt | null>(null)
  const [testOpen, setTestOpen] = useState<Prompt | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<Prompt | null>(null)

  // ── Queries ────────────────────────────────────────────────
  const { data: namesData, isLoading, isError, error } = useQuery({
    queryKey: ["prompt-names"],
    queryFn: listPromptNames,
  })
  const names = namesData?.prompts ?? []

  const { data: versionsData, isLoading: versionsLoading } = useQuery({
    queryKey: ["prompt-versions", selectedName],
    queryFn: () => listPromptVersions(selectedName!),
    enabled: !!selectedName,
  })
  const versions = versionsData?.versions ?? []

  const { data: deploymentsData } = useQuery({
    queryKey: ["prompt-deployments", selectedName],
    queryFn: () => listPromptDeployments(selectedName!),
    enabled: !!selectedName,
  })
  const deployments = deploymentsData?.deployments ?? []

  // ── Mutations ──────────────────────────────────────────────
  const createMut = useMutation({
    mutationFn: createPrompt,
    onSuccess: (p) => {
      queryClient.invalidateQueries({ queryKey: ["prompt-names"] })
      queryClient.invalidateQueries({ queryKey: ["prompt-versions", p.name] })
      toast({ title: "Prompt created", description: `${p.name} v${p.version}` })
      setCreateOpen(false)
      setSelectedName(p.name)
    },
    onError: (e: Error) =>
      toast({ title: "Create failed", description: e.message, variant: "destructive" }),
  })

  const deployMut = useMutation({
    mutationFn: ({ name, label, version }: { name: string; label: string; version: number }) =>
      deployPrompt(name, label, version),
    onSuccess: (d) => {
      queryClient.invalidateQueries({ queryKey: ["prompt-deployments", d.prompt_name] })
      toast({ title: `Deployed to ${d.label}`, description: `Version ${d.version}` })
      setDeployOpen(null)
    },
    onError: (e: Error) =>
      toast({ title: "Deploy failed", description: e.message, variant: "destructive" }),
  })

  const deleteMut = useMutation({
    mutationFn: ({ name, version }: { name: string; version: number }) =>
      deletePromptVersion(name, version),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["prompt-versions", selectedName] })
      queryClient.invalidateQueries({ queryKey: ["prompt-deployments", selectedName] })
      toast({ title: "Version deleted" })
      setDeleteTarget(null)
    },
    onError: (e: Error) =>
      toast({ title: "Delete failed", description: e.message, variant: "destructive" }),
  })

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load prompts</h2>
        <p className="text-sm text-muted-foreground mt-1">
          {error instanceof Error ? error.message : "Unknown error"}
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Prompts</h1>
          <p className="text-muted-foreground">
            Versioned prompt templates with label-based deployments
          </p>
        </div>
        <Button onClick={() => setCreateOpen(true)}>
          <Plus className="mr-2 h-4 w-4" /> New Prompt
        </Button>
      </div>

      <div className="grid gap-6 md:grid-cols-[250px_1fr]">
        {/* ── Sidebar: prompt names ─────────────────────────── */}
        <Card>
          <CardHeader className="pb-3">
            <CardTitle className="text-sm">Prompts ({names.length})</CardTitle>
          </CardHeader>
          <CardContent className="p-2">
            {isLoading ? (
              <div className="space-y-2">
                {[1, 2, 3].map((i) => (
                  <Skeleton key={i} className="h-8 w-full" />
                ))}
              </div>
            ) : names.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-8 text-center text-muted-foreground text-sm">
                <FileText className="h-8 w-8 mb-2 opacity-30" />
                No prompts
              </div>
            ) : (
              <div className="space-y-1">
                {names.map((n) => (
                  <button
                    key={n}
                    onClick={() => setSelectedName(n)}
                    className={`w-full text-left px-3 py-1.5 text-sm rounded-md transition-colors flex items-center gap-2 ${
                      selectedName === n
                        ? "bg-primary text-primary-foreground"
                        : "hover:bg-muted"
                    }`}
                  >
                    <FileText className="h-3.5 w-3.5 shrink-0" />
                    <span className="truncate">{n}</span>
                  </button>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        {/* ── Main: selected prompt details ─────────────────── */}
        <div>
          {!selectedName ? (
            <Card>
              <CardContent className="flex flex-col items-center justify-center py-20 text-center">
                <FileText className="h-10 w-10 text-muted-foreground mb-4" />
                <h3 className="text-lg font-semibold">Select a prompt</h3>
                <p className="text-sm text-muted-foreground mt-1">
                  Choose from the list, or create a new one.
                </p>
              </CardContent>
            </Card>
          ) : (
            <Tabs defaultValue="versions">
              <TabsList>
                <TabsTrigger value="versions">Versions ({versions.length})</TabsTrigger>
                <TabsTrigger value="deployments">
                  Deployments ({deployments.length})
                </TabsTrigger>
              </TabsList>

              <TabsContent value="versions" className="mt-4">
                <Card>
                  <CardHeader>
                    <CardTitle className="text-base">{selectedName}</CardTitle>
                    <CardDescription>
                      Version history. Newest at top.
                    </CardDescription>
                  </CardHeader>
                  <CardContent>
                    {versionsLoading ? (
                      <div className="space-y-3">
                        {[1, 2, 3].map((i) => (
                          <Skeleton key={i} className="h-12 w-full" />
                        ))}
                      </div>
                    ) : (
                      <Table>
                        <TableHeader>
                          <TableRow>
                            <TableHead>Version</TableHead>
                            <TableHead>Model</TableHead>
                            <TableHead>Preview</TableHead>
                            <TableHead>Created</TableHead>
                            <TableHead className="text-right">Actions</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {versions.map((v) => {
                            const deployedLabels = deployments
                              .filter((d) => d.version === v.version)
                              .map((d) => d.label)
                            return (
                              <TableRow key={v.id}>
                                <TableCell>
                                  <div className="flex items-center gap-2">
                                    <Badge variant="outline" className="font-mono">
                                      v{v.version}
                                    </Badge>
                                    {deployedLabels.map((l) => (
                                      <Badge key={l} variant="success" className="text-xs">
                                        {l}
                                      </Badge>
                                    ))}
                                  </div>
                                </TableCell>
                                <TableCell className="text-sm">
                                  {v.default_model ?? "\u2014"}
                                </TableCell>
                                <TableCell className="text-xs font-mono text-muted-foreground max-w-xs truncate">
                                  {v.content.slice(0, 60)}
                                  {v.content.length > 60 ? "\u2026" : ""}
                                </TableCell>
                                <TableCell className="text-xs text-muted-foreground whitespace-nowrap">
                                  {format(new Date(v.created_at), "MMM d HH:mm")}
                                </TableCell>
                                <TableCell className="text-right">
                                  <div className="flex justify-end gap-1">
                                    <Button
                                      variant="ghost"
                                      size="icon"
                                      onClick={() => setViewOpen(v)}
                                      title="View"
                                    >
                                      <Eye className="h-4 w-4" />
                                    </Button>
                                    <Button
                                      variant="ghost"
                                      size="icon"
                                      onClick={() => setTestOpen(v)}
                                      title="Test render"
                                    >
                                      <Play className="h-4 w-4" />
                                    </Button>
                                    <Button
                                      variant="ghost"
                                      size="icon"
                                      onClick={() => setDeployOpen(v)}
                                      title="Deploy"
                                    >
                                      <Rocket className="h-4 w-4 text-primary" />
                                    </Button>
                                    {deployedLabels.length === 0 && (
                                      <Button
                                        variant="ghost"
                                        size="icon"
                                        onClick={() => setDeleteTarget(v)}
                                        title="Delete"
                                      >
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
                    )}
                  </CardContent>
                </Card>
              </TabsContent>

              <TabsContent value="deployments" className="mt-4">
                <Card>
                  <CardHeader>
                    <CardTitle className="text-base">Active Deployments</CardTitle>
                    <CardDescription>
                      Route client requests with <code className="text-xs">prompt_ref.label</code>
                      &nbsp;to resolve the right version.
                    </CardDescription>
                  </CardHeader>
                  <CardContent>
                    {deployments.length === 0 ? (
                      <div className="flex flex-col items-center justify-center py-8 text-center text-muted-foreground">
                        <Rocket className="h-8 w-8 mb-3 opacity-30" />
                        <p className="text-sm">No deployments yet</p>
                      </div>
                    ) : (
                      <div className="grid gap-3 md:grid-cols-2">
                        {deployments.map((d) => (
                          <DeploymentCard key={d.id} deployment={d} />
                        ))}
                      </div>
                    )}
                  </CardContent>
                </Card>
              </TabsContent>
            </Tabs>
          )}
        </div>
      </div>

      {/* Create Dialog */}
      <CreatePromptDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        onCreate={createMut.mutate}
        pending={createMut.isPending}
      />

      {/* Deploy Dialog */}
      {deployOpen && (
        <DeployDialog
          prompt={deployOpen}
          onCancel={() => setDeployOpen(null)}
          onDeploy={(label) =>
            deployMut.mutate({ name: deployOpen.name, label, version: deployOpen.version })
          }
          pending={deployMut.isPending}
        />
      )}

      {/* View Dialog */}
      <Dialog open={!!viewOpen} onOpenChange={(o) => !o && setViewOpen(null)}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {viewOpen?.name} v{viewOpen?.version}
            </DialogTitle>
            <DialogDescription>
              {viewOpen?.default_model && (
                <span>Default model: <code>{viewOpen.default_model}</code></span>
              )}
            </DialogDescription>
          </DialogHeader>
          <pre className="bg-muted rounded-md p-4 text-xs font-mono overflow-auto max-h-96 whitespace-pre-wrap">
            {viewOpen?.content}
          </pre>
        </DialogContent>
      </Dialog>

      {/* Test render dialog */}
      {testOpen && <TestRenderDialog prompt={testOpen} onClose={() => setTestOpen(null)} />}

      {/* Delete confirmation */}
      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(o) => !o && setDeleteTarget(null)}
        title="Delete prompt version"
        description={`Are you sure you want to delete ${deleteTarget?.name} v${deleteTarget?.version}? This cannot be undone.`}
        confirmLabel="Delete"
        variant="destructive"
        loading={deleteMut.isPending}
        onConfirm={() =>
          deleteTarget &&
          deleteMut.mutate({ name: deleteTarget.name, version: deleteTarget.version })
        }
      />
    </div>
  )
}

// ── Sub-components ──────────────────────────────────────────

function DeploymentCard({ deployment }: { deployment: PromptDeployment }) {
  return (
    <div className="border rounded-md p-3 flex items-center justify-between">
      <div>
        <Badge variant="success" className="mb-1">
          {deployment.label}
        </Badge>
        <p className="text-sm">
          Version <span className="font-mono font-semibold">{deployment.version}</span>
        </p>
        <p className="text-xs text-muted-foreground mt-1">
          Deployed {format(new Date(deployment.deployed_at), "MMM d HH:mm")}
        </p>
      </div>
      <Rocket className="h-5 w-5 text-primary shrink-0" />
    </div>
  )
}

function CreatePromptDialog({
  open,
  onOpenChange,
  onCreate,
  pending,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  onCreate: (input: { name: string; content: string; default_model?: string }) => void
  pending: boolean
}) {
  const [name, setName] = useState("")
  const [content, setContent] = useState("")
  const [defaultModel, setDefaultModel] = useState("")
  const [errors, setErrors] = useState<Record<string, string>>({})

  function handleSubmit() {
    const fieldErrors: Record<string, string> = {}
    pipe(
      validate(name, required("Name")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["name"] = e.message))),
    )
    pipe(
      validate(content, required("Content")),
      E.mapLeft((errs) => errs.forEach((e) => (fieldErrors["content"] = e.message))),
    )
    if (Object.keys(fieldErrors).length > 0) {
      setErrors(fieldErrors)
      return
    }
    setErrors({})
    onCreate({
      name: name.trim(),
      content,
      default_model: defaultModel.trim() || undefined,
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Create Prompt</DialogTitle>
          <DialogDescription>
            A new version is created each time you save. Use <code>{"{{var_name}}"}</code> for
            variables.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-2">
          <div className="grid gap-2">
            <Label htmlFor="prompt-name">Name</Label>
            <Input
              id="prompt-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="customer_support"
            />
            {errors["name"] && (
              <p className="text-sm text-destructive">{errors["name"]}</p>
            )}
          </div>
          <div className="grid gap-2">
            <Label htmlFor="prompt-content">Content</Label>
            <Textarea
              id="prompt-content"
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="You are a helpful assistant for {{brand}}..."
              className="font-mono text-xs min-h-48"
            />
            {errors["content"] && (
              <p className="text-sm text-destructive">{errors["content"]}</p>
            )}
          </div>
          <div className="grid gap-2">
            <Label htmlFor="prompt-model">Default Model (optional)</Label>
            <Input
              id="prompt-model"
              value={defaultModel}
              onChange={(e) => setDefaultModel(e.target.value)}
              placeholder="gpt-4o"
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={pending}>
            {pending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Create
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function DeployDialog({
  prompt,
  onCancel,
  onDeploy,
  pending,
}: {
  prompt: Prompt
  onCancel: () => void
  onDeploy: (label: string) => void
  pending: boolean
}) {
  const [label, setLabel] = useState("prod")
  return (
    <Dialog open={true} onOpenChange={(o) => !o && onCancel()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>
            Deploy {prompt.name} v{prompt.version}
          </DialogTitle>
          <DialogDescription>
            Client requests with <code>prompt_ref.label = "{label}"</code> will resolve to this
            version.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-2 py-4">
          <Label>Label</Label>
          <Select value={label} onValueChange={setLabel}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {DEFAULT_LABELS.map((l) => (
                <SelectItem key={l} value={l}>
                  {l}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Input
            placeholder="Or type a custom label (e.g. experiment-a)"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
            className="mt-2"
          />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button onClick={() => onDeploy(label)} disabled={pending || !label}>
            {pending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Deploy
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function TestRenderDialog({ prompt, onClose }: { prompt: Prompt; onClose: () => void }) {
  const [varsJson, setVarsJson] = useState("{\n  \n}")
  const [result, setResult] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleRender() {
    setLoading(true)
    try {
      const vars = JSON.parse(varsJson || "{}") as Record<string, unknown>
      const res = await resolvePrompt(prompt.name, null, vars)
      setResult(res.content)
    } catch (e) {
      toast({
        title: "Render failed",
        description: e instanceof Error ? e.message : "Invalid JSON",
        variant: "destructive",
      })
    } finally {
      setLoading(false)
    }
  }

  return (
    <Dialog open={true} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>
            Test render: {prompt.name} v{prompt.version}
          </DialogTitle>
          <DialogDescription>
            Provide variables as JSON, then see the rendered output.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-2">
          <div>
            <Label>Variables (JSON)</Label>
            <Textarea
              value={varsJson}
              onChange={(e) => setVarsJson(e.target.value)}
              className="font-mono text-xs min-h-24"
            />
          </div>
          {result !== null && (
            <div>
              <Label>Rendered Output</Label>
              <pre className="bg-muted rounded-md p-3 text-xs font-mono overflow-auto max-h-64 whitespace-pre-wrap">
                {result}
              </pre>
            </div>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Close
          </Button>
          <Button onClick={handleRender} disabled={loading}>
            {loading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Render
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
