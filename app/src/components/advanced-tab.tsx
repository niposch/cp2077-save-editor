import { useEffect, useMemo, useState } from "react"
import { ChevronRight, Search, TriangleAlert, CornerDownRight } from "lucide-react"
import { Input } from "@/components/ui/input"
import { HudHeading, Panel } from "@/components/hud"
import { VirtualList } from "@/components/virtual-list"
import { useOpenSave } from "@/state/save-context"
import type { TreeValue } from "@/lib/core"
import { cn } from "@/lib/utils"

const ROW = 24

export function AdvancedTab() {
  const { open, rev } = useOpenSave()
  const nodes = useMemo(() => open.session.packageNodes(), [open])
  const [node, setNode] = useState(() => (nodes.includes("ScriptableSystemsContainer") ? "ScriptableSystemsContainer" : nodes[0]))
  const [filter, setFilter] = useState("")
  const [selected, setSelected] = useState<number | null>(null)

  const chunks = useMemo(() => (node ? open.session.packageChunks(node) : []), [open, node, rev])
  const filtered = useMemo(() => {
    const f = filter.trim().toLowerCase()
    return f ? chunks.filter((c) => c.type.toLowerCase().includes(f) || `#${c.index}` === f) : chunks
  }, [chunks, filter])

  useEffect(() => {
    // default selection: the player's development data if present
    const pdd = chunks.find((c) => c.type === "PlayerDevelopmentData")
    setSelected(pdd?.index ?? chunks[0]?.index ?? null)
  }, [node]) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="space-y-4">
      <div className="flex items-start gap-2 border border-gold/30 bg-gold/5 px-3 py-2 text-sm text-gold/90">
        <TriangleAlert className="mt-0.5 size-4 shrink-0" />
        <span>
          Advanced edits change exactly one stored value. The game sometimes keeps copies of a value elsewhere (attributes are stored
          twice, for example), so prefer the Character tab where it applies, and keep a backup of your save.
        </span>
      </div>
      <div className="grid gap-6 lg:grid-cols-[minmax(260px,320px)_1fr]">
        <Panel className="flex h-[75vh] flex-col gap-3">
          <HudHeading className="mb-0">Systems</HudHeading>
          <div className="flex flex-wrap gap-1.5">
            {nodes.map((n) => (
              <button
                key={n}
                type="button"
                onClick={() => setNode(n)}
                className={cn(
                  "border px-2 py-0.5 font-heading text-xs font-medium tracking-wide transition-colors",
                  n === node ? "border-cyan-dim bg-cyan/10 text-cyan" : "border-border text-muted-foreground hover:text-foreground",
                )}
              >
                {n}
              </button>
            ))}
          </div>
          <div className="relative">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Filter by type or #index" className="pl-9" />
          </div>
          <div className="text-xs text-muted-foreground">
            {filtered.length.toLocaleString()} of {chunks.length.toLocaleString()} objects
          </div>
          <VirtualList
            className="-mr-2 min-h-0 flex-1 pr-2"
            items={filtered}
            rowHeight={26}
            getKey={(c) => c.index}
            render={(c) => (
              <button
                type="button"
                onClick={() => setSelected(c.index)}
                className={cn(
                  "flex h-full w-full items-center gap-2 px-2 text-left text-sm hover:bg-accent",
                  selected === c.index && "bg-accent text-cyan",
                )}
              >
                <span className="w-12 shrink-0 font-mono text-xs text-muted-foreground">#{c.index}</span>
                <span className="truncate">{c.type}</span>
              </button>
            )}
          />
        </Panel>
        <Panel className="flex h-[75vh] min-w-0 flex-col">
          {node && selected != null ? <ChunkView node={node} index={selected} onJump={setSelected} /> : <p className="text-muted-foreground">Nothing selected.</p>}
        </Panel>
      </div>
    </div>
  )
}

/** One visible line of the value tree. */
interface FlatRow {
  /** Structural key (stable across edits), used for expansion state. */
  key: string
  depth: number
  name: string
  ty: string
  v: TreeValue
  /** Edit path for this value, or null when it can't be addressed (arrays of arrays). */
  path: string | null
  open: boolean
}

/** Flatten the expanded parts of a value tree into rows. */
function flatten(props: { name: string; ty: string; v: TreeValue }[], path: string | null, key: string, depth: number, expanded: Set<string>, out: FlatRow[]) {
  for (const p of props) push(p.name, p.ty, p.v, path && `${path}.${p.name}`, true, `${key}.${p.name}`, depth, expanded, out)
}

function push(name: string, ty: string, v: TreeValue, path: string | null, indexable: boolean, key: string, depth: number, expanded: Set<string>, out: FlatRow[]) {
  const open = expanded.has(key)
  out.push({ key, depth, name, ty, v, path, open })
  if (!open) return
  if (v.k === "struct") flatten(v.props, path, key, depth + 1, expanded, out)
  else if (v.k === "array")
    v.items.forEach((item, i) =>
      // `[i]` can only follow a field name, so elements of nested arrays aren't addressable
      push(`[${i}]`, v.ty, item, path && indexable ? `${path}[${i}]` : null, false, `${key}[${i}]`, depth + 1, expanded, out),
    )
}

/** Top-level structs and short arrays start expanded. */
function initialExpanded(v: TreeValue): Set<string> {
  const s = new Set<string>()
  if (v.k === "struct")
    for (const p of v.props) {
      if (p.v.k === "struct" || (p.v.k === "array" && p.v.items.length <= 20)) s.add(`.${p.name}`)
    }
  return s
}

function ChunkView({ node, index, onJump }: { node: string; index: number; onJump: (i: number) => void }) {
  const { open, rev } = useOpenSave()
  const chunk = useMemo(() => {
    try {
      return open.session.chunk(node, index)
    } catch {
      return null
    }
  }, [open, node, index, rev])

  // expansion survives edits, resets when another object is shown
  const [expanded, setExpanded] = useState<Set<string>>(new Set())
  useEffect(() => {
    if (chunk) setExpanded(initialExpanded(chunk.value))
  }, [node, index, open]) // eslint-disable-line react-hooks/exhaustive-deps

  const rows = useMemo(() => {
    const out: FlatRow[] = []
    if (chunk?.value.k === "struct") flatten(chunk.value.props, `#${index}`, "", 0, expanded, out)
    return out
  }, [chunk, expanded, index])

  if (!chunk) return <p className="text-muted-foreground">Couldn't read object #{index}.</p>
  const toggle = (key: string) =>
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })

  return (
    <>
      <HudHeading right={<span className="font-mono text-xs text-muted-foreground">{node} · #{index}</span>}>{chunk.type}</HudHeading>
      {rows.length === 0 ? (
        <p className="text-muted-foreground">No stored fields (all defaults).</p>
      ) : (
        <VirtualList
          className="min-h-0 flex-1 font-mono text-[13px]"
          items={rows}
          rowHeight={ROW}
          getKey={(r) => r.key}
          render={(r) => <TreeRow row={r} node={node} onJump={onJump} onToggle={toggle} />}
        />
      )}
    </>
  )
}

function TreeRow({ row, node, onJump, onToggle }: { row: FlatRow; node: string; onJump: (i: number) => void; onToggle: (key: string) => void }) {
  const { v, depth, name, ty } = row
  if (v.k === "scalar" || v.k === "buffer") {
    return (
      <div className="flex h-full items-center gap-2 whitespace-nowrap hover:bg-accent/40" style={{ paddingLeft: depth * 16 + 20 }}>
        <span className="shrink-0 text-foreground/90">{name}</span>
        <span className="shrink-0 text-xs text-muted-foreground/70">{ty}</span>
        <span className="text-muted-foreground">=</span>
        {v.k === "buffer" ? (
          <span className="text-muted-foreground">&lt;{v.len.toLocaleString()} bytes, not editable&gt;</span>
        ) : v.kind === "handle" ? (
          <button type="button" className="flex items-center gap-1 text-sky-400 hover:underline" onClick={() => Number(v.text) >= 0 && onJump(Number(v.text))}>
            <CornerDownRight className="size-3" /> #{v.text}
          </button>
        ) : (
          <ScalarEditor node={node} path={row.path} kind={v.kind} text={v.text} />
        )}
      </div>
    )
  }
  const count = v.k === "struct" ? v.props.length : v.items.length
  // a short label for array entries, e.g. "[5] · CoolSkill"
  const hint = v.k === "struct" ? v.props.find((p) => p.v.k === "scalar" && (p.v.kind === "enum" || p.v.kind === "cname" || p.v.kind === "str")) : undefined
  return (
    <button
      type="button"
      onClick={() => onToggle(row.key)}
      className="flex h-full w-full items-center gap-1.5 whitespace-nowrap text-left hover:bg-accent/40"
      style={{ paddingLeft: depth * 16 }}
      aria-expanded={row.open}
    >
      <ChevronRight className={cn("size-3.5 shrink-0 text-muted-foreground transition-transform", row.open && "rotate-90")} />
      <span className="text-foreground/90">{name}</span>
      <span className="text-xs text-muted-foreground/70">{ty}</span>
      <span className="text-xs text-salmon/80">{v.k === "array" ? `[${count.toLocaleString()}]` : `{${count}}`}</span>
      {hint && hint.v.k === "scalar" && <span className="truncate text-xs text-gold/80">· {hint.v.text}</span>}
    </button>
  )
}

function ScalarEditor({ node, path, kind, text }: { node: string; path: string | null; kind: string; text: string }) {
  const { mutate } = useOpenSave()
  const [draft, setDraft] = useState(text)
  useEffect(() => setDraft(text), [text])
  const color = kind === "enum" || kind === "cname" ? "text-gold" : kind === "str" ? "text-emerald-300" : "text-cyan"

  if (!path) return <span className={color}>{text}</span>

  if (kind === "bool") {
    return (
      <button
        type="button"
        className={cn("px-1 hover:bg-accent", color)}
        onClick={() => mutate("Couldn't set value", (s) => s.setValue(node, path, text === "true" ? "false" : "true"))}
        title="Click to toggle"
      >
        {text}
      </button>
    )
  }

  const commit = () => {
    if (draft === text) return
    if (mutate("Couldn't set value", (s) => s.setValue(node, path, draft)) === undefined) setDraft(text)
  }
  return (
    <input
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onFocus={(e) => e.currentTarget.select()}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur()
        if (e.key === "Escape") setDraft(text)
      }}
      spellCheck={false}
      size={Math.max(4, Math.min(48, draft.length + 1))}
      className={cn(
        "h-5 min-w-0 bg-transparent px-1 outline-none hover:shadow-[0_0_0_1px_var(--color-border)] focus:shadow-[0_0_0_1px_var(--color-cyan-dim)]",
        color,
        draft !== text && "shadow-[0_0_0_1px_color-mix(in_oklab,var(--color-gold)_60%,transparent)]",
      )}
      title={path}
    />
  )
}
