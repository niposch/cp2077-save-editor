import { useMemo, useState } from "react"
import { Search, Trash2, Save } from "lucide-react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { HudHeading, Panel } from "@/components/hud"
import { VirtualList } from "@/components/virtual-list"
import { useOpenSave } from "@/state/save-context"
import type { FactInfo } from "@/lib/core"
import { cn } from "@/lib/utils"

export function FactsTab() {
  const { open, rev, mutate } = useOpenSave()
  const [query, setQuery] = useState("")
  const [newValue, setNewValue] = useState("")
  const key = query.trim()

  const fact = useMemo<FactInfo | null>(() => {
    if (!key) return null
    try {
      return open.session.getFact(key)
    } catch {
      return null
    }
  }, [key, open, rev])

  const total = useMemo(() => open.session.factCount(), [open, rev])

  // Facts the game itself lists in metadata.9.json, with the value in the (possibly edited) save.
  const listed = useMemo(() => {
    return (open.meta?.facts ?? []).map((f) => {
      const [name, metaValue] = f.split("=")
      let current: number | null = null
      try {
        current = open.session.getFact(name).value
      } catch {
        /* ignore */
      }
      return { name, metaValue, current }
    })
  }, [open, rev])

  const set = () => {
    const v = Number(newValue)
    if (!Number.isInteger(v) || v < 0) return toast.error("Fact values are whole numbers ≥ 0")
    if (mutate("Couldn't set fact", (s) => (s.setFact(key, v), true))) toast.success(`${key} = ${v}`)
    setNewValue("")
  }
  const remove = () => {
    if (mutate("Couldn't remove fact", (s) => s.removeFact(key))) toast.success(`${key} removed`)
  }

  return (
    <div className="grid gap-6 lg:grid-cols-[1fr_minmax(280px,380px)]">
      <Panel className="space-y-4">
        <HudHeading right={<span className="font-heading text-sm text-muted-foreground">{total.toLocaleString()} facts in save</span>}>
          Quest facts
        </HudHeading>
        <p className="text-sm text-muted-foreground">
          Facts are the game's quest flags. They're stored only as hashes of their names, so you need the exact name (or a{" "}
          <span className="font-mono text-xs">0x…</span> hash). An absent fact counts as 0.
        </p>
        <div className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="e.g. ResetAttributeDisabled or 0x2b0475c8"
            className="pl-9 font-mono"
            spellCheck={false}
          />
        </div>
        {fact && (
          <div className="hud-panel is-active space-y-4 p-4">
            <div className="flex flex-wrap items-baseline justify-between gap-2">
              <span className="font-mono text-sm break-all">{key}</span>
              <span className="font-mono text-xs text-muted-foreground">{fact.hash}</span>
            </div>
            <div className="flex items-baseline gap-3">
              <span className={cn("font-heading text-4xl font-semibold", fact.value == null ? "text-muted-foreground" : "text-cyan")}>
                {fact.value ?? 0}
              </span>
              <span className="text-sm text-muted-foreground">{fact.value == null ? "not set" : `table ${fact.table}`}</span>
            </div>
            <div className="flex flex-wrap gap-2">
              <Input
                value={newValue}
                onChange={(e) => setNewValue(e.target.value.replace(/\D/g, ""))}
                onKeyDown={(e) => e.key === "Enter" && newValue && set()}
                placeholder="New value"
                inputMode="numeric"
                className="w-32 font-mono"
              />
              <Button onClick={set} disabled={!newValue}>
                <Save /> Set
              </Button>
              <Button variant="outline" onClick={remove} disabled={fact.value == null}>
                <Trash2 /> Remove
              </Button>
            </div>
          </div>
        )}
      </Panel>

      <Panel>
        <HudHeading className="mb-2">Listed by this save</HudHeading>
        <p className="mb-3 text-xs text-muted-foreground">Facts the game records in the save's metadata. Click one to inspect it.</p>
        {listed.length === 0 ? (
          <p className="text-sm text-muted-foreground">None.</p>
        ) : (
          <VirtualList
            className="pr-1"
            style={{ height: Math.min(listed.length * 26, 520) }}
            items={listed}
            rowHeight={26}
            getKey={(f) => f.name}
            render={(f) => (
              <button
                type="button"
                onClick={() => setQuery(f.name)}
                className={cn(
                  "flex h-full w-full items-center justify-between gap-2 px-2 text-left font-mono text-xs hover:bg-accent",
                  key === f.name && "bg-accent text-cyan",
                )}
              >
                <span className="truncate">{f.name}</span>
                <span className="tabular-nums text-muted-foreground">{f.current ?? 0}</span>
              </button>
            )}
          />
        )}
      </Panel>
    </div>
  )
}
