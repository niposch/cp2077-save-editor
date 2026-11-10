import { useMemo } from "react"
import { RotateCcw, TriangleAlert, CircleCheck } from "lucide-react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { HudHeading, Panel, Stepper } from "@/components/hud"
import { useOpenSave } from "@/state/save-context"
import type { AttributeKey, PlayerData, PointKey } from "@/lib/core"
import { cn } from "@/lib/utils"

const ATTR_LABEL: Record<AttributeKey, string> = {
  body: "Body",
  reflexes: "Reflexes",
  tech: "Technical Ability",
  intelligence: "Intelligence",
  cool: "Cool",
}

/** Grid placement mirroring the in-game attribute layout. */
const ATTR_AREA: Record<AttributeKey, string> = {
  reflexes: "col-start-2 row-start-1",
  body: "col-start-1 row-start-2",
  tech: "col-start-3 row-start-2",
  intelligence: "col-start-1 row-start-3",
  cool: "col-start-3 row-start-3",
}

const POINT_STYLE: Record<PointKey, { label: string; color: string; hint: string }> = {
  attribute: { label: "Attribute points available", color: "text-cyan", hint: "Unspent attribute points" },
  perk: { label: "Perk points available", color: "text-gold", hint: "Unspent perk points" },
  relic: { label: "Relic points available", color: "text-sky-400", hint: "Unspent Relic (Phantom Liberty) points" },
  secondary: { label: "Legacy points", color: "text-muted-foreground", hint: "Pre-2.0 secondary perk points; unused by the current skill system" },
}

function AttributeTile({ a, onSet }: { a: PlayerData["attributes"][number]; onSet: (v: number) => void }) {
  const value = a.value ?? 3
  const mismatch = a.modifier != null && Math.abs(a.modifier - value) > 0.01
  return (
    <Panel className={cn("flex flex-col items-center gap-1 px-2 py-5", ATTR_AREA[a.key])}>
      <div className="hud-label flex min-h-10 items-center gap-1.5 text-center text-sm leading-tight text-salmon">
        {ATTR_LABEL[a.key]}
        {mismatch && (
          <Tooltip>
            <TooltipTrigger asChild>
              <TriangleAlert className="size-3.5 text-gold" />
            </TooltipTrigger>
            <TooltipContent>
              Save has {value} in player data but {a.modifier} in the stats system. Setting a value fixes both.
            </TooltipContent>
          </Tooltip>
        )}
      </div>
      <Stepper label={ATTR_LABEL[a.key]} value={value} min={3} max={20} onCommit={onSet} valueClassName="text-4xl text-cyan" />
    </Panel>
  )
}

function ResetPanel({ used, onRestore }: { used: boolean; onRestore: () => void }) {
  return (
    <Panel className="flex flex-col gap-3">
      <HudHeading className="mb-0">Reset attributes</HudHeading>
      <p className="text-sm text-muted-foreground">
        The game allows one attribute reset per playthrough.{" "}
        {used ? "This save has used it." : "This save can still use it."}
      </p>
      <div className="flex items-center justify-between gap-3">
        <span className={cn("hud-label flex items-center gap-1.5 text-sm", used ? "text-salmon" : "text-emerald-300")}>
          {used ? <RotateCcw className="size-4" /> : <CircleCheck className="size-4" />}
          {used ? "Used" : "Available"}
        </span>
        <Button variant="outline" disabled={!used} onClick={onRestore} className="hud-label cut cut-sm border-cyan-dim text-cyan hover:bg-cyan/10 hover:text-cyan">
          <RotateCcw /> Restore reset
        </Button>
      </div>
    </Panel>
  )
}

export function ChangesPanel() {
  const { changes } = useOpenSave()
  return (
    <Panel>
      <HudHeading className="mb-2" right={<span className="font-heading text-sm text-muted-foreground">{changes.length}</span>}>
        Pending changes
      </HudHeading>
      {changes.length === 0 ? (
        <p className="text-sm text-muted-foreground">No changes yet.</p>
      ) : (
        <ol className="max-h-72 space-y-1 overflow-auto pr-1 text-sm">
          {changes.map((c, i) => (
            <li key={i} className="flex gap-2">
              <span className="font-heading text-cyan-dim tabular-nums">{String(i + 1).padStart(2, "0")}</span>
              <span className="break-all">{c}</span>
            </li>
          ))}
        </ol>
      )}
    </Panel>
  )
}

export function CharacterTab() {
  const { open, rev, mutate } = useOpenSave()
  const player = useMemo<PlayerData | null>(() => {
    try {
      return open.session.player()
    } catch {
      return null
    }
  }, [open, rev])

  if (!player) {
    return <p className="text-muted-foreground">Couldn't read player data from this save.</p>
  }

  const setAttr = (key: AttributeKey, v: number) => {
    const warn = mutate(`Couldn't set ${ATTR_LABEL[key]}`, (s) => s.setAttribute(key, v))
    if (warn) toast.warning(warn)
  }
  const setPoints = (key: PointKey, v: number) => mutate("Couldn't set points", (s) => s.setPoints(key, v))
  const restore = () => {
    if (mutate("Couldn't restore the reset", (s) => s.unlockReset())) toast.success("Attribute reset is available again")
  }
  const points = player.points.filter((p) => p.key !== "secondary")

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(240px,280px)_1fr] xl:grid-cols-[minmax(240px,280px)_1fr_minmax(260px,300px)]">
      <div className="space-y-6">
        <Panel>
          <HudHeading>Points</HudHeading>
          <div className="space-y-4">
            {points.map((p) => (
              <div key={p.key} className="flex items-center justify-between gap-2">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <div className={cn("hud-label text-sm leading-tight", POINT_STYLE[p.key].color)}>{POINT_STYLE[p.key].label}</div>
                  </TooltipTrigger>
                  <TooltipContent>
                    {POINT_STYLE[p.key].hint}
                    {p.spent != null && ` · ${p.spent} spent`}
                  </TooltipContent>
                </Tooltip>
                <Stepper
                  label={POINT_STYLE[p.key].label}
                  value={p.unspent ?? 0}
                  min={0}
                  max={999}
                  onCommit={(v) => setPoints(p.key, v)}
                  valueClassName={cn("text-2xl", POINT_STYLE[p.key].color)}
                />
              </div>
            ))}
          </div>
        </Panel>
        <ResetPanel used={player.resetUsed} onRestore={restore} />
      </div>

      <div className="mx-auto grid w-full max-w-[560px] min-w-[420px] grid-cols-3 gap-3 self-start">
        {player.attributes.map((a) => (
          <AttributeTile key={a.key} a={a} onSet={(v) => setAttr(a.key, v)} />
        ))}
        <div className="col-start-2 row-start-2 flex flex-col items-center justify-center">
          <div className="grid size-24 rotate-45 place-items-center border border-cyan-dim/60 bg-cyan/5">
            <div className="-rotate-45 text-center">
              <div className="font-heading text-4xl font-semibold text-cyan">{player.level ?? "–"}</div>
              <div className="hud-label text-xs text-cyan-dim">Level</div>
            </div>
          </div>
        </div>
        <div className="col-span-3 col-start-1 row-start-4 pt-1 text-center text-xs text-muted-foreground">
          Attributes range from 3 to 20. Changing one doesn't refund or consume points; adjust points separately.
        </div>
      </div>

      <div className="grid gap-6 sm:grid-cols-2 lg:col-span-2 xl:col-span-1 xl:grid-cols-1 xl:content-start">
        <ChangesPanel />
        <SaveInfoPanel />
      </div>
    </div>
  )
}

function SaveInfoPanel() {
  const { open } = useOpenSave()
  const info = useMemo(() => open.session.info(), [open])
  const m = open.meta
  const rows: [string, string | number | undefined][] = [
    ["Patch", m?.buildPatch],
    ["Life path", m?.lifePath],
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
            <dd className="text-right font-heading font-medium">{v ?? "–"}</dd>
          </div>
        ))}
      </dl>
    </Panel>
  )
}
