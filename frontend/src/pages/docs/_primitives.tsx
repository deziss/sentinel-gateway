import { ReactNode } from "react"
import { cn } from "@/lib/utils"

export function H1({ children }: { children: ReactNode }) {
  return <h1 className="text-4xl font-bold tracking-tight mb-3">{children}</h1>
}

export function Lead({ children }: { children: ReactNode }) {
  return <p className="text-lg text-muted-foreground mb-10">{children}</p>
}

export function H2({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <h2
      id={id}
      className="text-2xl font-semibold tracking-tight mt-12 mb-4 border-b pb-2 scroll-mt-20"
    >
      {children}
    </h2>
  )
}

export function H3({ children, id }: { children: ReactNode; id?: string }) {
  return (
    <h3 id={id} className="text-xl font-semibold mt-8 mb-3 scroll-mt-20">
      {children}
    </h3>
  )
}

export function P({ children }: { children: ReactNode }) {
  return <p className="leading-7 my-4 text-foreground/90">{children}</p>
}

export function UL({ children }: { children: ReactNode }) {
  return <ul className="list-disc pl-6 my-4 space-y-2 text-foreground/90">{children}</ul>
}

export function OL({ children }: { children: ReactNode }) {
  return <ol className="list-decimal pl-6 my-4 space-y-2 text-foreground/90">{children}</ol>
}

export function Code({ children }: { children: ReactNode }) {
  return (
    <code className="bg-muted px-1.5 py-0.5 rounded text-sm font-mono">
      {children}
    </code>
  )
}

export function Pre({ children, lang }: { children: ReactNode; lang?: string }) {
  return (
    <div className="my-4 rounded-md border bg-muted/50 overflow-hidden">
      {lang && (
        <div className="px-4 py-1.5 text-xs font-mono text-muted-foreground border-b bg-muted/30">
          {lang}
        </div>
      )}
      <pre className="p-4 overflow-x-auto text-sm font-mono leading-6">
        <code>{children}</code>
      </pre>
    </div>
  )
}

export function Callout({
  kind = "info",
  children,
}: {
  kind?: "info" | "warn" | "tip"
  children: ReactNode
}) {
  const styles = {
    info: "border-blue-500/30 bg-blue-500/5 text-blue-900 dark:text-blue-100",
    warn: "border-amber-500/30 bg-amber-500/5 text-amber-900 dark:text-amber-100",
    tip: "border-emerald-500/30 bg-emerald-500/5 text-emerald-900 dark:text-emerald-100",
  }
  const labels = { info: "Note", warn: "Warning", tip: "Tip" }
  return (
    <div className={cn("my-5 border-l-4 rounded-r-md px-4 py-3", styles[kind])}>
      <div className="font-semibold text-sm mb-1">{labels[kind]}</div>
      <div className="text-sm leading-6">{children}</div>
    </div>
  )
}

export function Table({ children }: { children: ReactNode }) {
  return (
    <div className="my-5 overflow-x-auto rounded-md border">
      <table className="w-full text-sm">{children}</table>
    </div>
  )
}

export function THead({ children }: { children: ReactNode }) {
  return <thead className="bg-muted/50 border-b">{children}</thead>
}

export function TH({ children }: { children: ReactNode }) {
  return <th className="text-left font-semibold px-4 py-2">{children}</th>
}

export function TR({ children }: { children: ReactNode }) {
  return <tr className="border-b last:border-b-0">{children}</tr>
}

export function TD({ children }: { children: ReactNode }) {
  return <td className="px-4 py-2 align-top">{children}</td>
}

export function Method({ m }: { m: "GET" | "POST" | "PUT" | "DELETE" | "PATCH" }) {
  const colors = {
    GET: "bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/30",
    POST: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/30",
    PUT: "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/30",
    DELETE: "bg-red-500/10 text-red-600 dark:text-red-400 border-red-500/30",
    PATCH: "bg-violet-500/10 text-violet-600 dark:text-violet-400 border-violet-500/30",
  }
  return (
    <span
      className={cn(
        "inline-block px-2 py-0.5 rounded text-xs font-mono font-semibold border",
        colors[m]
      )}
    >
      {m}
    </span>
  )
}

export function Endpoint({
  method,
  path,
}: {
  method: "GET" | "POST" | "PUT" | "DELETE" | "PATCH"
  path: string
}) {
  return (
    <div className="flex items-center gap-3 my-3 p-3 rounded-md border bg-muted/30">
      <Method m={method} />
      <code className="text-sm font-mono">{path}</code>
    </div>
  )
}
