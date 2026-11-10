import { RotateCcw, TriangleAlert, CircleCheck } from "lucide-react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { HudHeading, Panel, Stepper } from "@/components/hud"
import { useOpenSave } from "@/state/save-context"
import type { AttributeKey, LifePath, PlayerData, PointKey, ProgressionKey } from "@/lib/core"
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

const LIFE_PATHS: { value: LifePath; label: string }[] = [
  { value: "StreetKid", label: "Street Kid" },
  { value: "Nomad", label: "Nomad" },
  { value: "Corporate", label: "Corpo" },
]

function ProgressionPanel({ player }: { player: PlayerData }) {
  const { mutate } = useOpenSave()
  const setProg = (key: ProgressionKey, label: string, v: number) => {
    const warn = mutate(`Couldn't set ${label}`, (s) => s.setProgression(key, v))
    if (warn) toast.warning(warn)
  }
  const row = "flex flex-wrap items-center justify-between gap-x-2 gap-y-1"
  return (
    <Panel>
      <HudHeading>Progression</HudHeading>
      <div className="space-y-4">
        <div className={row}>
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="hud-label text-sm text-cyan">Level</span>
            </TooltipTrigger>
            <TooltipContent className="max-w-64">
              1–60. XP towards the next level resets to 0. Changing the level here doesn't grant the attribute and perk points a
              level-up would; adjust those below.
            </TooltipContent>
          </Tooltip>
          <Stepper label="Level" value={player.level ?? 1} min={1} max={60} onCommit={(v) => setProg("level", "level", v)} valueClassName="text-2xl text-cyan" />
        </div>
        <div className={row}>
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="hud-label text-sm text-emerald-300">Street cred</span>
            </TooltipTrigger>
            <TooltipContent className="max-w-64">1–50. Street cred XP towards the next level resets to 0.</TooltipContent>
          </Tooltip>
          <Stepper
            label="Street cred"
            value={player.streetCred ?? 1}
            min={1}
            max={50}
            onCommit={(v) => setProg("streetcred", "street cred", v)}
            valueClassName="text-2xl text-emerald-300"
          />
        </div>
        <div className={row}>
          <span className="hud-label text-sm text-gold">Money €$</span>
          {player.money == null ? (
            <span className="text-xs text-muted-foreground">No money item in V's inventory</span>
          ) : (
            <Stepper
              label="Money"
              value={player.money}
              min={0}
              max={player.moneyMax}
              onCommit={(v) => mutate("Couldn't set money", (s) => s.setMoney(v))}
              className="ml-auto"
              valueClassName="text-2xl text-gold"
            />
          )}
        </div>
      </div>
    </Panel>
  )
}

function LifePathPanel({ value }: { value: LifePath | null }) {
  const { mutate } = useOpenSave()
  return (
    <Panel>
      <HudHeading className="mb-2">Life path</HudHeading>
      <div className="grid grid-cols-3 gap-1.5" role="radiogroup" aria-label="Life path">
        {LIFE_PATHS.map((lp) => (
          <button
            key={lp.value}
            type="button"
            role="radio"
            aria-checked={value === lp.value}
            onClick={() => value !== lp.value && mutate("Couldn't change the life path", (s) => s.setLifePath(lp.value))}
            className={cn(
              "hud-label cut cut-sm border px-1 py-1.5 text-xs whitespace-nowrap transition-colors",
              value === lp.value ? "border-cyan-dim bg-cyan/10 text-cyan" : "border-border text-muted-foreground hover:text-foreground",
            )}
          >
            {lp.label}
          </button>
        ))}
      </div>
      <p className="mt-2 text-xs text-muted-foreground">
        Changes background-specific dialogue options from here on. The prologue isn't replayed, and quests already finished keep their
        outcome.
      </p>
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

export function CharacterTab() {
  const { player, mutate } = useOpenSave()

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
    <div className="grid gap-6 xl:grid-cols-[minmax(240px,280px)_1fr]">
      <div className="grid content-start gap-6 md:grid-cols-2 xl:grid-cols-1">
        <ProgressionPanel player={player} />
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
      </div>

      <div className="mx-auto w-full max-w-[600px] space-y-6 self-start">
      <div className="grid min-w-[420px] grid-cols-3 gap-3">
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
      <div className="grid gap-6 sm:grid-cols-2">
        <ResetPanel used={player.resetUsed} onRestore={restore} />
        <LifePathPanel value={player.lifePath} />
      </div>
      </div>
    </div>
  )
}
