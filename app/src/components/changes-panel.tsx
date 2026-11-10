import { useMemo } from "react"
import { Undo2 } from "lucide-react"
import { HudHeading, Panel } from "@/components/hud"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useOpenSave } from "@/state/save-context"

/** Pending changes with per-change revert; shown beside every tab. */
export function ChangesPanel() {
  const { changes, revert } = useOpenSave()
  return (
    <Panel className="flex min-h-0 flex-col">
      <HudHeading className="mb-2" right={<span className="font-heading text-sm text-muted-foreground tabular-nums">{changes.length}</span>}>
        Pending changes
      </HudHeading>
      {changes.length === 0 ? (
        <p className="text-sm text-muted-foreground">No changes yet.</p>
      ) : (
        <ol className="-mr-2 min-h-0 space-y-1 overflow-auto pr-2 text-sm">
          {changes.map((c, i) => (
            <li key={c.target} className="group flex items-start gap-2">
              <span className="pt-0.5 font-heading text-xs text-cyan-dim tabular-nums">{String(i + 1).padStart(2, "0")}</span>
              <div className="min-w-0 flex-1">
                <div className="break-all text-foreground/90">{c.target}</div>
                <div className="font-mono text-xs break-all">
                  <span className="text-muted-foreground line-through decoration-muted-foreground/50">{c.old}</span>
                  <span className="text-muted-foreground"> → </span>
                  <span className="text-cyan">{c.new}</span>
                </div>
              </div>
              <Tooltip>
                <TooltipTrigger asChild>
                  <button
                    type="button"
                    onClick={() => revert(i)}
                    className="grid size-6 shrink-0 place-items-center text-muted-foreground opacity-70 transition hover:bg-accent hover:text-cyan hover:opacity-100 focus-visible:opacity-100 group-hover:opacity-100"
                    aria-label={`Revert ${c.target}`}
                  >
                    <Undo2 className="size-3.5" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="left">Revert this change</TooltipContent>
              </Tooltip>
            </li>
          ))}
        </ol>
      )}
    </Panel>
  )
}

export function SaveInfoPanel() {
  const { open } = useOpenSave()
  const info = useMemo(() => open.session.info(), [open])
  const m = open.meta
  const rows: [string, string | number | undefined][] = [
    ["Save", open.files.folder],
    ["Patch", m?.buildPatch],
    ["Difficulty", m?.difficulty],
    ["Save version", info.saveVersion],
    ["Game version", info.gameVersion],
    ["Sections", info.nodes],
  ]
  return (
    <Panel>
      <HudHeading className="mb-2">Save</HudHeading>
      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        {rows.map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-muted-foreground">{k}</dt>
            <dd className="truncate text-right font-heading font-medium">{v ?? "–"}</dd>
          </div>
        ))}
      </dl>
    </Panel>
  )
}
