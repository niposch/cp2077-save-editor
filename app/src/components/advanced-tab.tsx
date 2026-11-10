import { useEffect, useMemo, useState } from "react"
import { ChevronRight, Search, TriangleAlert, CornerDownRight } from "lucide-react"
import { Input } from "@/components/ui/input"
import { HudHeading, Panel } from "@/components/hud"
import { useOpenSave } from "@/state/save-context"
import type { TreeValue } from "@/lib/core"
import { cn } from "@/lib/utils"

const PAGE = 100

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
        <Panel className="flex max-h-[75vh] flex-col gap-3">
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
            {filtered.length} of {chunks.length} objects
          </div>
          <ul className="-mr-2 flex-1 overflow-auto pr-2">
            {filtered.slice(0, 500).map((c) => (
              <li key={c.index}>
                <button
                  type="button"
                  onClick={() => setSelected(c.index)}
                  className={cn(
                    "flex w-full items-baseline gap-2 px-2 py-0.5 text-left text-sm hover:bg-accent",
                    selected === c.index && "bg-accent text-cyan",
                  )}
                >
                  <span className="w-12 shrink-0 font-mono text-xs text-muted-foreground">#{c.index}</span>
                  <span className="truncate">{c.type}</span>
                </button>
              </li>
            ))}
            {filtered.length > 500 && <li className="px-2 py-1 text-xs text-muted-foreground">…refine the filter to see more</li>}
          </ul>
        </Panel>
        <Panel className="min-w-0">
          {node && selected != null ? <ChunkView node={node} index={selected} onJump={setSelected} /> : <p className="text-muted-foreground">Nothing selected.</p>}
        </Panel>
      </div>
    </div>
  )
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
  if (!chunk) return <p className="text-muted-foreground">Couldn't read object #{index}.</p>
  return (
    <div className="min-w-0">
      <HudHeading right={<span className="font-mono text-xs text-muted-foreground">{node} · #{index}</span>}>{chunk.type}</HudHeading>
      <div className="overflow-x-auto font-mono text-[13px] leading-6">
        {chunk.value.k === "struct" && chunk.value.props.length === 0 && <p className="text-muted-foreground">No stored fields (all defaults).</p>}
        {chunk.value.k === "struct" &&
          chunk.value.props.map((p) => (
            <TreeRow key={p.name} name={p.name} ty={p.ty} v={p.v} path={`#${index}.${p.name}`} indexable node={node} onJump={onJump} depth={0} />
          ))}
      </div>
    </div>
  )
}

interface RowProps {
  name: string
  ty: string
  v: TreeValue
  /** Path addressing this value, or null when it can't be addressed (arrays of arrays). */
  path: string | null
  /** Whether `[i]` may be appended to `path` (only directly after a field name). */
  indexable: boolean
  node: string
  onJump: (i: number) => void
  depth: number
}

function TreeRow({ name, ty, v, path, indexable, node, onJump, depth }: RowProps) {
  const [open, setOpen] = useState(depth < 1 && !(v.k === "array" && v.items.length > 20))
  const [shown, setShown] = useState(PAGE)

  if (v.k === "scalar" || v.k === "buffer") {
    return (
      <div className="group flex items-center gap-2 hover:bg-accent/40" style={{ paddingLeft: depth * 16 + 20 }}>
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
          <ScalarEditor node={node} path={path} kind={v.kind} text={v.text} />
        )}
      </div>
    )
  }

  const count = v.k === "struct" ? v.props.length : v.items.length
  // a short label for array entries, e.g. "[5] · CoolSkill"
  const hint =
    v.k === "struct"
      ? v.props.find((p) => p.v.k === "scalar" && (p.v.kind === "enum" || p.v.kind === "cname" || p.v.kind === "str"))
      : undefined
  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-full items-center gap-1.5 text-left hover:bg-accent/40"
        style={{ paddingLeft: depth * 16 }}
      >
        <ChevronRight className={cn("size-3.5 shrink-0 text-muted-foreground transition-transform", open && "rotate-90")} />
        <span className="text-foreground/90">{name}</span>
        <span className="text-xs text-muted-foreground/70">{ty}</span>
        <span className="text-xs text-salmon/80">{v.k === "array" ? `[${count}]` : `{${count}}`}</span>
        {hint && hint.v.k === "scalar" && <span className="truncate text-xs text-gold/80">· {hint.v.text}</span>}
      </button>
      {open &&
        (v.k === "struct"
          ? v.props.map((p) => (
              <TreeRow key={p.name} name={p.name} ty={p.ty} v={p.v} path={path && `${path}.${p.name}`} indexable node={node} onJump={onJump} depth={depth + 1} />
            ))
          : (
            <>
              {v.items.slice(0, shown).map((item, i) => (
                <TreeRow
                  key={i}
                  name={`[${i}]`}
                  ty={v.ty}
                  v={item}
                  path={path && indexable ? `${path}[${i}]` : null}
                  indexable={false}
                  node={node}
                  onJump={onJump}
                  depth={depth + 1}
                />
              ))}
              {v.items.length > shown && (
                <button
                  type="button"
                  onClick={() => setShown(shown + PAGE)}
                  className="text-xs text-cyan-dim hover:text-cyan"
                  style={{ paddingLeft: (depth + 1) * 16 + 20 }}
                >
                  show {Math.min(PAGE, v.items.length - shown)} more of {v.items.length - shown} remaining
                </button>
              )}
            </>
          ))}
    </div>
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
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur()
        if (e.key === "Escape") setDraft(text)
      }}
      spellCheck={false}
      size={Math.max(4, Math.min(48, draft.length + 1))}
      className={cn("min-w-0 border border-transparent bg-transparent px-1 outline-none hover:border-border focus:border-cyan-dim", color, draft !== text && "border-gold/60")}
      title={path}
    />
  )
}
