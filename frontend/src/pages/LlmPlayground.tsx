import { useState, useRef, useEffect } from "react"
import { useQuery } from "@tanstack/react-query"
import {
  listBackends,
  chatCompletion,
  streamChatCompletion,
  createEmbedding,
  type ChatMessage,
} from "@/lib/api"
import { toast } from "@/hooks/use-toast"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"
import { Switch } from "@/components/ui/switch"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  CardDescription,
} from "@/components/ui/card"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { ConfirmDialog } from "@/components/ConfirmDialog"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Send,
  Bot,
  User,
  MessageSquare,
  Loader2,
  Trash2,
  Cpu,
  Sparkles,
} from "lucide-react"

interface PlaygroundMessage {
  role: "system" | "user" | "assistant"
  content: string
}

export function LlmPlayground() {
  // Chat state
  const [messages, setMessages] = useState<PlaygroundMessage[]>([])
  const [input, setInput] = useState("")
  const [systemPrompt, setSystemPrompt] = useState("")
  const [selectedModel, setSelectedModel] = useState("")
  const [temperature, setTemperature] = useState(0.7)
  const [maxTokens, setMaxTokens] = useState(4096)
  const [topP, setTopP] = useState(1.0)
  const [useStreaming, setUseStreaming] = useState(false)
  const [loading, setLoading] = useState(false)
  const [stats, setStats] = useState({ latency: 0, promptTokens: 0, completionTokens: 0, totalTokens: 0 })
  const [clearOpen, setClearOpen] = useState(false)
  const scrollRef = useRef<HTMLDivElement>(null)

  // Embedding state
  const [embeddingInput, setEmbeddingInput] = useState("")
  const [embeddingModel, setEmbeddingModel] = useState("")
  const [embeddingResult, setEmbeddingResult] = useState<{ dimensions: number; tokens: number; preview: number[] } | null>(null)
  const [embeddingLoading, setEmbeddingLoading] = useState(false)

  const { data: backends, isLoading: backendsLoading } = useQuery({
    queryKey: ["backends"],
    queryFn: listBackends,
  })

  const llmBackends = (backends ?? []).filter((b) =>
    ["open_ai", "anthropic", "google_vertex", "aws_bedrock", "ollama", "vllm", "open_ai_compatible"].includes(b.provider_type)
  )

  // Auto-select first model
  useEffect(() => {
    if (llmBackends.length > 0 && !selectedModel) {
      setSelectedModel(llmBackends[0].name)
    }
  }, [llmBackends, selectedModel])

  // Auto-scroll
  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight, behavior: "smooth" })
  }, [messages])

  async function handleSend() {
    if (!input.trim() || loading || !selectedModel) return

    const userMsg: PlaygroundMessage = { role: "user", content: input }
    const allMessages: ChatMessage[] = [
      ...(systemPrompt ? [{ role: "system" as const, content: systemPrompt }] : []),
      ...messages.map((m) => ({ role: m.role, content: m.content })),
      { role: "user" as const, content: input },
    ]

    setMessages((prev) => [...prev, userMsg])
    setInput("")
    setLoading(true)
    const startTime = Date.now()

    if (useStreaming) {
      const assistantMsg: PlaygroundMessage = { role: "assistant", content: "" }
      setMessages((prev) => [...prev, assistantMsg])

      const { reader, abort } = streamChatCompletion(selectedModel, allMessages, {
        temperature,
        max_tokens: maxTokens,
        top_p: topP,
      })

      const decoder = new TextDecoder()
      let fullContent = ""

      try {
        while (true) {
          const { done, value } = await reader.read()
          if (done) break

          const chunk = decoder.decode(value, { stream: true })
          const lines = chunk.split("\n").filter((l) => l.startsWith("data: "))

          for (const line of lines) {
            const data = line.slice(6).trim()
            if (data === "[DONE]") break
            try {
              const parsed = JSON.parse(data)
              const delta = parsed.choices?.[0]?.delta?.content
              if (delta) {
                fullContent += delta
                setMessages((prev) => {
                  const updated = [...prev]
                  updated[updated.length - 1] = { role: "assistant", content: fullContent }
                  return updated
                })
              }
            } catch {
              // skip unparseable chunks
            }
          }
        }
      } catch (err) {
        if ((err as Error).name !== "AbortError") {
          setMessages((prev) => {
            const updated = [...prev]
            updated[updated.length - 1] = {
              role: "assistant",
              content: `Error: ${(err as Error).message}`,
            }
            return updated
          })
        }
      }

      void abort // reference to keep linter happy
      setStats({ latency: Date.now() - startTime, promptTokens: 0, completionTokens: 0, totalTokens: 0 })
    } else {
      try {
        const resp = await chatCompletion(selectedModel, allMessages, {
          temperature,
          max_tokens: maxTokens,
          top_p: topP,
        })
        const content = resp.choices[0]?.message?.content ?? ""
        setMessages((prev) => [...prev, { role: "assistant", content }])
        setStats({
          latency: Date.now() - startTime,
          promptTokens: resp.usage?.prompt_tokens ?? 0,
          completionTokens: resp.usage?.completion_tokens ?? 0,
          totalTokens: resp.usage?.total_tokens ?? 0,
        })
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : "Unknown error"
        setMessages((prev) => [
          ...prev,
          { role: "assistant", content: `Error: ${msg}. Ensure the gateway is running.` },
        ])
      }
    }

    setLoading(false)
  }

  async function handleEmbedding() {
    if (!embeddingInput.trim() || embeddingLoading) return
    const model = embeddingModel || selectedModel
    if (!model) {
      toast({ title: "Select a model first", variant: "destructive" })
      return
    }

    setEmbeddingLoading(true)
    try {
      const resp = await createEmbedding(model, embeddingInput)
      const emb = resp.data[0]?.embedding ?? []
      setEmbeddingResult({
        dimensions: emb.length,
        tokens: resp.usage?.total_tokens ?? 0,
        preview: emb.slice(0, 10),
      })
    } catch (err: unknown) {
      toast({
        title: "Embedding failed",
        description: err instanceof Error ? err.message : "Unknown error",
        variant: "destructive",
      })
    } finally {
      setEmbeddingLoading(false)
    }
  }

  function handleClear() {
    setMessages([])
    setStats({ latency: 0, promptTokens: 0, completionTokens: 0, totalTokens: 0 })
    setClearOpen(false)
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">LLM Playground</h1>
        <p className="text-muted-foreground">Test models through the Sentinel Gateway proxy</p>
      </div>

      <Tabs defaultValue="chat">
        <TabsList>
          <TabsTrigger value="chat">Chat Completions</TabsTrigger>
          <TabsTrigger value="embeddings">Embeddings</TabsTrigger>
        </TabsList>

        {/* ── Chat Tab ──────────────────────────────────────────── */}
        <TabsContent value="chat" className="mt-4">
          <div className="flex gap-6 h-[calc(100vh-16rem)]">
            {/* Sidebar */}
            <div className="w-72 flex-shrink-0 space-y-4">
              <Card className="h-full overflow-auto">
                <CardHeader className="pb-3">
                  <CardTitle className="text-sm">Configuration</CardTitle>
                </CardHeader>
                <CardContent className="space-y-5">
                  {/* Model Select */}
                  <div className="space-y-2">
                    <Label className="text-xs uppercase text-muted-foreground font-semibold">Model</Label>
                    {backendsLoading ? (
                      <Skeleton className="h-9 w-full" />
                    ) : (
                      <Select value={selectedModel} onValueChange={setSelectedModel}>
                        <SelectTrigger className="h-9">
                          <SelectValue placeholder="Select model" />
                        </SelectTrigger>
                        <SelectContent>
                          {llmBackends.map((b) => (
                            <SelectItem key={b.id} value={b.name}>
                              <div className="flex items-center gap-2">
                                <Cpu className="h-3 w-3" />
                                {b.name}
                              </div>
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                    )}
                  </div>

                  {/* System Prompt */}
                  <div className="space-y-2">
                    <Label className="text-xs uppercase text-muted-foreground font-semibold">System Prompt</Label>
                    <Textarea
                      value={systemPrompt}
                      onChange={(e) => setSystemPrompt(e.target.value)}
                      placeholder="You are a helpful assistant..."
                      className="min-h-[80px] text-xs"
                    />
                  </div>

                  {/* Temperature */}
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <Label className="text-xs uppercase text-muted-foreground font-semibold">Temperature</Label>
                      <span className="text-xs font-mono">{temperature}</span>
                    </div>
                    <Input
                      type="range"
                      min={0}
                      max={2}
                      step={0.1}
                      value={temperature}
                      onChange={(e) => setTemperature(parseFloat(e.target.value))}
                      className="h-2 cursor-pointer"
                    />
                  </div>

                  {/* Top P */}
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <Label className="text-xs uppercase text-muted-foreground font-semibold">Top P</Label>
                      <span className="text-xs font-mono">{topP}</span>
                    </div>
                    <Input
                      type="range"
                      min={0}
                      max={1}
                      step={0.05}
                      value={topP}
                      onChange={(e) => setTopP(parseFloat(e.target.value))}
                      className="h-2 cursor-pointer"
                    />
                  </div>

                  {/* Max Tokens */}
                  <div className="space-y-2">
                    <Label className="text-xs uppercase text-muted-foreground font-semibold">Max Tokens</Label>
                    <Input
                      type="number"
                      min={1}
                      max={128000}
                      value={maxTokens}
                      onChange={(e) => setMaxTokens(parseInt(e.target.value) || 4096)}
                      className="h-9"
                    />
                  </div>

                  {/* Streaming Toggle */}
                  <div className="flex items-center justify-between">
                    <Label className="text-xs uppercase text-muted-foreground font-semibold">Streaming</Label>
                    <Switch checked={useStreaming} onCheckedChange={setUseStreaming} />
                  </div>

                  {/* Stats */}
                  {stats.latency > 0 && (
                    <div className="pt-4 border-t space-y-1.5">
                      <div className="flex justify-between text-xs">
                        <span className="text-muted-foreground">Latency</span>
                        <span className="font-mono text-emerald-500">{stats.latency}ms</span>
                      </div>
                      {stats.totalTokens > 0 && (
                        <>
                          <div className="flex justify-between text-xs">
                            <span className="text-muted-foreground">Prompt</span>
                            <span className="font-mono">{stats.promptTokens}</span>
                          </div>
                          <div className="flex justify-between text-xs">
                            <span className="text-muted-foreground">Completion</span>
                            <span className="font-mono">{stats.completionTokens}</span>
                          </div>
                          <div className="flex justify-between text-xs font-medium">
                            <span>Total Tokens</span>
                            <span className="font-mono">{stats.totalTokens}</span>
                          </div>
                        </>
                      )}
                    </div>
                  )}
                </CardContent>
              </Card>
            </div>

            {/* Chat Area */}
            <Card className="flex-1 flex flex-col min-w-0">
              <CardHeader className="border-b py-3">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <MessageSquare className="h-5 w-5 text-primary" />
                    <div>
                      <CardTitle className="text-base">Chat</CardTitle>
                      <CardDescription className="text-xs">
                        {selectedModel ? `Model: ${selectedModel}` : "Select a model to begin"}
                      </CardDescription>
                    </div>
                  </div>
                  {messages.length > 0 && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 text-muted-foreground hover:text-destructive"
                      onClick={() => setClearOpen(true)}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  )}
                </div>
              </CardHeader>

              <CardContent ref={scrollRef} className="flex-1 overflow-y-auto p-6 space-y-4">
                {messages.length === 0 && (
                  <div className="flex flex-col items-center justify-center h-full text-center text-muted-foreground">
                    <Sparkles className="h-10 w-10 mb-4 opacity-30" />
                    <p className="text-sm">Send a message to start testing</p>
                  </div>
                )}
                {messages.map((msg, i) => (
                  <div key={i} className={`flex gap-3 ${msg.role === "user" ? "flex-row-reverse" : ""}`}>
                    <div
                      className={`mt-0.5 h-8 w-8 rounded-full flex items-center justify-center shrink-0 text-xs font-bold ${
                        msg.role === "user"
                          ? "bg-primary text-primary-foreground"
                          : "bg-muted text-muted-foreground"
                      }`}
                    >
                      {msg.role === "user" ? <User className="h-4 w-4" /> : <Bot className="h-4 w-4" />}
                    </div>
                    <div
                      className={`max-w-[80%] rounded-lg px-4 py-2.5 text-sm whitespace-pre-wrap ${
                        msg.role === "user"
                          ? "bg-primary text-primary-foreground"
                          : "bg-muted"
                      }`}
                    >
                      {msg.content}
                    </div>
                  </div>
                ))}
                {loading && !useStreaming && (
                  <div className="flex gap-3">
                    <div className="h-8 w-8 rounded-full bg-muted flex items-center justify-center">
                      <Loader2 className="h-4 w-4 animate-spin" />
                    </div>
                    <div className="h-8 w-20 bg-muted rounded-lg animate-pulse" />
                  </div>
                )}
              </CardContent>

              <div className="p-4 border-t">
                <div className="flex gap-2">
                  <Input
                    placeholder="Type a message..."
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && !e.shiftKey && handleSend()}
                    disabled={loading || !selectedModel}
                  />
                  <Button onClick={handleSend} disabled={loading || !input.trim() || !selectedModel}>
                    {loading ? <Loader2 className="h-4 w-4 animate-spin" /> : <Send className="h-4 w-4" />}
                  </Button>
                </div>
              </div>
            </Card>
          </div>
        </TabsContent>

        {/* ── Embeddings Tab ───────────────────────────────────── */}
        <TabsContent value="embeddings" className="mt-4">
          <div className="grid gap-6 md:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Generate Embedding</CardTitle>
                <CardDescription>Convert text to a vector representation</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <Label>Model</Label>
                  {backendsLoading ? (
                    <Skeleton className="h-9 w-full" />
                  ) : (
                    <Select value={embeddingModel || selectedModel} onValueChange={setEmbeddingModel}>
                      <SelectTrigger>
                        <SelectValue placeholder="Select embedding model" />
                      </SelectTrigger>
                      <SelectContent>
                        {llmBackends.map((b) => (
                          <SelectItem key={b.id} value={b.name}>{b.name}</SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  )}
                </div>
                <div className="space-y-2">
                  <Label>Input Text</Label>
                  <Textarea
                    value={embeddingInput}
                    onChange={(e) => setEmbeddingInput(e.target.value)}
                    placeholder="Enter text to generate an embedding vector..."
                    className="min-h-[150px]"
                  />
                </div>
                <Button
                  onClick={handleEmbedding}
                  disabled={embeddingLoading || !embeddingInput.trim()}
                  className="w-full"
                >
                  {embeddingLoading && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
                  Generate Embedding
                </Button>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Result</CardTitle>
                <CardDescription>Embedding vector details</CardDescription>
              </CardHeader>
              <CardContent>
                {embeddingResult ? (
                  <div className="space-y-4">
                    <div className="grid grid-cols-2 gap-4">
                      <div className="space-y-1">
                        <span className="text-xs text-muted-foreground uppercase font-semibold">Dimensions</span>
                        <p className="text-2xl font-bold">{embeddingResult.dimensions}</p>
                      </div>
                      <div className="space-y-1">
                        <span className="text-xs text-muted-foreground uppercase font-semibold">Tokens Used</span>
                        <p className="text-2xl font-bold">{embeddingResult.tokens}</p>
                      </div>
                    </div>
                    <div className="space-y-2">
                      <span className="text-xs text-muted-foreground uppercase font-semibold">
                        Vector Preview (first 10 dims)
                      </span>
                      <pre className="bg-muted rounded-lg p-3 text-xs font-mono overflow-auto max-h-[200px]">
                        [{embeddingResult.preview.map((v) => v.toFixed(6)).join(",\n ")}
                        {embeddingResult.dimensions > 10 ? ",\n ..." : ""}]
                      </pre>
                    </div>
                  </div>
                ) : (
                  <div className="flex flex-col items-center justify-center py-12 text-center text-muted-foreground">
                    <Cpu className="h-10 w-10 mb-4 opacity-30" />
                    <p className="text-sm">Generate an embedding to see results</p>
                  </div>
                )}
              </CardContent>
            </Card>
          </div>
        </TabsContent>
      </Tabs>

      {/* Clear Chat Confirmation */}
      <ConfirmDialog
        open={clearOpen}
        onOpenChange={setClearOpen}
        title="Clear Conversation"
        description="Are you sure you want to clear the entire conversation history? This action cannot be undone."
        confirmLabel="Clear"
        variant="destructive"
        onConfirm={handleClear}
      />
    </div>
  )
}
