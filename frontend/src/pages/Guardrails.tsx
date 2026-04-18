import { useState } from "react"
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import {
  listGuardrails,
  createGuardrail,
  updateGuardrail,
  deleteGuardrail,
  testGuardrails,
  type GuardrailRule,
  type GuardrailKind,
  type GuardrailStage,
  type GuardrailMode,
  type CreateGuardrailInput,
} from "@/lib/api"
import { toast } from "@/hooks/use-toast"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { Badge } from "@/components/ui/badge"
import { Switch } from "@/components/ui/switch"
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
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Plus,
  Shield,
  Trash2,
  Loader2,
  AlertTriangle,
  Play,
  CheckCircle2,
  XCircle,
} from "lucide-react"

const KIND_DESCRIPTIONS: Record<GuardrailKind, string> = {
  regex: "Match arbitrary patterns (jailbreak, keywords, secrets)",
  pii: "Built-in PII detection (email, phone, SSN, credit card, AWS keys)",
  length: "Enforce maximum content length",
  json_schema: "Validate JSON output against a schema (post-call only)",
}

const STAGE_LABELS: Record<GuardrailStage, string> = {
  pre_call: "Before LLM",
  post_call: "After LLM",
  logging_only: "Log Only",
}

const MODE_COLORS: Record<GuardrailMode, "destructive" | "warning" | "secondary"> = {
  block: "destructive",
  redact: "warning",
  flag: "secondary",
}

export function Guardrails() {
  const queryClient = useQueryClient()
  const [createOpen, setCreateOpen] = useState(false)
  const [testOpen, setTestOpen] = useState(false)
  const [deleteTarget, setDeleteTarget] = useState<GuardrailRule | null>(null)

  const { data, isLoading, isError, error } = useQuery({
    queryKey: ["guardrails"],
    queryFn: listGuardrails,
  })
  const rules = data?.rules ?? []

  const createMut = useMutation({
    mutationFn: createGuardrail,
    onSuccess: (r) => {
      queryClient.invalidateQueries({ queryKey: ["guardrails"] })
      toast({ title: "Rule created", description: r.name })
      setCreateOpen(false)
    },
    onError: (e: Error) =>
      toast({ title: "Create failed", description: e.message, variant: "destructive" }),
  })

  const toggleMut = useMutation({
    mutationFn: ({ id, is_active }: { id: string; is_active: boolean }) =>
      updateGuardrail(id, { is_active }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["guardrails"] })
    },
    onError: (e: Error) =>
      toast({ title: "Update failed", description: e.message, variant: "destructive" }),
  })

  const deleteMut = useMutation({
    mutationFn: deleteGuardrail,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["guardrails"] })
      toast({ title: "Rule deleted" })
      setDeleteTarget(null)
    },
    onError: (e: Error) =>
      toast({ title: "Delete failed", description: e.message, variant: "destructive" }),
  })

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-center">
        <AlertTriangle className="h-10 w-10 text-destructive mb-4" />
        <h2 className="text-lg font-semibold">Failed to load guardrails</h2>
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
          <h1 className="text-2xl font-bold tracking-tight">Guardrails</h1>
          <p className="text-muted-foreground">
            Policy checks applied before and after LLM requests
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setTestOpen(true)}>
            <Play className="mr-2 h-4 w-4" /> Test Pipeline
          </Button>
          <Button onClick={() => setCreateOpen(true)}>
            <Plus className="mr-2 h-4 w-4" /> New Rule
          </Button>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">Active Rules</CardTitle>
          <CardDescription>
            Lower priority = runs first. Redacting rules chain their output into later rules.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-3">
              {[1, 2, 3].map((i) => <Skeleton key={i} className="h-12 w-full" />)}
            </div>
          ) : rules.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center">
              <Shield className="h-10 w-10 text-muted-foreground mb-4" />
              <h3 className="text-lg font-semibold">No guardrails configured</h3>
              <p className="text-sm text-muted-foreground mt-1">
                Add rules to filter PII, block jailbreaks, enforce output schemas.
              </p>
              <Button className="mt-4" onClick={() => setCreateOpen(true)}>
                <Plus className="mr-2 h-4 w-4" /> Add Rule
              </Button>
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Kind</TableHead>
                  <TableHead>Stage</TableHead>
                  <TableHead>Mode</TableHead>
                  <TableHead>Category</TableHead>
                  <TableHead>Priority</TableHead>
                  <TableHead>Active</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rules.map((r) => (
                  <TableRow key={r.id}>
                    <TableCell className="font-medium">{r.name}</TableCell>
                    <TableCell>
                      <Badge variant="outline" className="capitalize">{r.kind}</Badge>
                    </TableCell>
                    <TableCell className="text-sm">{STAGE_LABELS[r.stage]}</TableCell>
                    <TableCell>
                      <Badge variant={MODE_COLORS[r.mode]} className="uppercase text-xs">
                        {r.mode}
                      </Badge>
                    </TableCell>
                    <TableCell>
                      <Badge variant="secondary">{r.category}</Badge>
                    </TableCell>
                    <TableCell className="font-mono text-xs">{r.priority}</TableCell>
                    <TableCell>
                      <Switch
                        checked={r.is_active}
                        onCheckedChange={(v) => toggleMut.mutate({ id: r.id, is_active: v })}
                      />
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => setDeleteTarget(r)}
                      >
                        <Trash2 className="h-4 w-4 text-destructive" />
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <CreateGuardrailDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        onCreate={createMut.mutate}
        pending={createMut.isPending}
      />

      {testOpen && <TestDialog onClose={() => setTestOpen(false)} />}

      <ConfirmDialog
        open={!!deleteTarget}
        onOpenChange={(o) => !o && setDeleteTarget(null)}
        title="Delete guardrail rule"
        description={`Are you sure you want to delete "${deleteTarget?.name}"? Active pipelines will stop enforcing this rule immediately.`}
        confirmLabel="Delete"
        variant="destructive"
        loading={deleteMut.isPending}
        onConfirm={() => deleteTarget && deleteMut.mutate(deleteTarget.id)}
      />
    </div>
  )
}

// ── Create dialog ───────────────────────────────────────────────────────────

function CreateGuardrailDialog({
  open,
  onOpenChange,
  onCreate,
  pending,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  onCreate: (input: CreateGuardrailInput) => void
  pending: boolean
}) {
  const [name, setName] = useState("")
  const [kind, setKind] = useState<GuardrailKind>("regex")
  const [stage, setStage] = useState<GuardrailStage>("pre_call")
  const [mode, setMode] = useState<GuardrailMode>("redact")
  const [category, setCategory] = useState("general")
  const [priority, setPriority] = useState("100")
  const [configJson, setConfigJson] = useState(configPlaceholder("regex"))

  function handleKindChange(k: GuardrailKind) {
    setKind(k)
    setConfigJson(configPlaceholder(k))
  }

  function handleSubmit() {
    let config: Record<string, unknown>
    try {
      config = JSON.parse(configJson || "{}") as Record<string, unknown>
    } catch (e) {
      toast({
        title: "Invalid config JSON",
        description: e instanceof Error ? e.message : "Parse error",
        variant: "destructive",
      })
      return
    }
    if (!name.trim()) {
      toast({ title: "Name is required", variant: "destructive" })
      return
    }
    onCreate({
      name: name.trim(),
      kind,
      stage,
      mode,
      category: category.trim() || "general",
      config,
      priority: parseInt(priority) || 100,
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Create Guardrail Rule</DialogTitle>
          <DialogDescription>{KIND_DESCRIPTIONS[kind]}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-2 md:grid-cols-2">
          <div>
            <Label>Name</Label>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="block-ssn"
            />
          </div>
          <div>
            <Label>Category</Label>
            <Input
              value={category}
              onChange={(e) => setCategory(e.target.value)}
              placeholder="pii, jailbreak, schema"
            />
          </div>
          <div>
            <Label>Kind</Label>
            <Select value={kind} onValueChange={(v) => handleKindChange(v as GuardrailKind)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="regex">regex</SelectItem>
                <SelectItem value="pii">pii</SelectItem>
                <SelectItem value="length">length</SelectItem>
                <SelectItem value="json_schema">json_schema</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div>
            <Label>Stage</Label>
            <Select value={stage} onValueChange={(v) => setStage(v as GuardrailStage)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="pre_call">pre_call (before LLM)</SelectItem>
                <SelectItem value="post_call">post_call (after LLM)</SelectItem>
                <SelectItem value="logging_only">logging_only</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div>
            <Label>Mode</Label>
            <Select value={mode} onValueChange={(v) => setMode(v as GuardrailMode)}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="block">block</SelectItem>
                <SelectItem value="redact">redact</SelectItem>
                <SelectItem value="flag">flag</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div>
            <Label>Priority (lower = runs first)</Label>
            <Input
              type="number"
              value={priority}
              onChange={(e) => setPriority(e.target.value)}
            />
          </div>
          <div className="md:col-span-2">
            <Label>Config (JSON)</Label>
            <Textarea
              value={configJson}
              onChange={(e) => setConfigJson(e.target.value)}
              className="font-mono text-xs min-h-32"
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

function configPlaceholder(kind: GuardrailKind): string {
  switch (kind) {
    case "regex":
      return JSON.stringify({ patterns: ["\\bsecret_\\w+\\b"] }, null, 2)
    case "pii":
      return JSON.stringify({ types: ["email", "phone", "ssn", "credit_card"] }, null, 2)
    case "length":
      return JSON.stringify({ max_chars: 100000 }, null, 2)
    case "json_schema":
      return JSON.stringify({
        schema: { type: "object", required: ["answer"] },
      }, null, 2)
  }
}

// ── Test Pipeline dialog ────────────────────────────────────────────────────

function TestDialog({ onClose }: { onClose: () => void }) {
  const [content, setContent] = useState(
    "Hi, my email is alice@example.com and my SSN is 123-45-6789.",
  )
  const testMut = useMutation({
    mutationFn: testGuardrails,
  })

  return (
    <Dialog open={true} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Test Guardrail Pipeline</DialogTitle>
          <DialogDescription>
            Evaluate arbitrary content against all active pre-call rules.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-2">
          <div>
            <Label>Input</Label>
            <Textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              className="min-h-24"
            />
          </div>
          <Button
            onClick={() => testMut.mutate(content)}
            disabled={testMut.isPending || !content.trim()}
          >
            {testMut.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Run Pipeline
          </Button>

          {testMut.data && (
            <div className="space-y-3 border-t pt-3">
              <div className="flex items-center gap-2">
                {testMut.data.blocked ? (
                  <>
                    <XCircle className="h-5 w-5 text-destructive" />
                    <span className="font-semibold text-destructive">Blocked</span>
                  </>
                ) : (
                  <>
                    <CheckCircle2 className="h-5 w-5 text-green-500" />
                    <span className="font-semibold text-green-500">Passed</span>
                  </>
                )}
              </div>
              {testMut.data.final_content !== testMut.data.input && (
                <div>
                  <Label>Modified Output</Label>
                  <pre className="bg-muted rounded p-2 text-xs font-mono whitespace-pre-wrap">
                    {testMut.data.final_content}
                  </pre>
                </div>
              )}
              <div>
                <Label>Evaluated Rules</Label>
                <div className="space-y-1 mt-1">
                  {testMut.data.results.map((r, i) => (
                    <div
                      key={i}
                      className="flex items-center justify-between text-xs bg-muted/50 rounded px-2 py-1"
                    >
                      <span className="font-medium">{r.name}</span>
                      <span className="text-muted-foreground">{r.outcome}</span>
                      <span className="text-muted-foreground font-mono">{r.duration_ms}ms</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>Close</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
