import { useEffect, useState, type ReactNode } from "react"
import { Minus, Plus } from "lucide-react"
import { cn } from "@/lib/utils"

/** Section heading in the game's condensed uppercase style. */
export function HudHeading({ children, className, right }: { children: ReactNode; className?: string; right?: ReactNode }) {
  return (
    <div className={cn("mb-3 flex items-center gap-3", className)}>
      <h2 className="hud-label text-salmon text-base">{children}</h2>
      <div className="h-px flex-1 bg-gradient-to-r from-salmon-dim/80 to-transparent" />
      {right}
    </div>
  )
}

export function Panel({ children, className, active }: { children: ReactNode; className?: string; active?: boolean }) {
  return <div className={cn("hud-panel p-4", active && "is-active", className)}>{children}</div>
}

/**
 * Number with −/+ buttons; click the number to type a value.
 * Calls onCommit only with values inside [min, max].
 */
export function Stepper({
  value,
  min,
  max,
  onCommit,
  className,
  valueClassName,
  label,
}: {
  value: number
  min: number
  max: number
  onCommit: (v: number) => void
  className?: string
  valueClassName?: string
  label: string
}) {
  const [editing, setEditing] = useState(false)
  const [text, setText] = useState(String(value))
  useEffect(() => setText(String(value)), [value])

  const commit = (v: number) => {
    if (Number.isFinite(v) && v >= min && v <= max && v !== value) onCommit(v)
  }
  const btn =
    "grid size-6 shrink-0 place-items-center text-muted-foreground transition-colors hover:text-cyan disabled:pointer-events-none disabled:opacity-30"

  return (
    <div className={cn("flex items-center gap-0.5", className)}>
      <button type="button" className={btn} disabled={value <= min} onClick={() => commit(value - 1)} aria-label={`Decrease ${label}`}>
        <Minus className="size-4" />
      </button>
      {editing ? (
        <input
          autoFocus
          inputMode="numeric"
          className={cn("w-14 bg-transparent text-center font-heading font-semibold tabular-nums outline-none ring-1 ring-cyan-dim", valueClassName)}
          value={text}
          onChange={(e) => setText(e.target.value.replace(/[^\d-]/g, ""))}
          onBlur={() => {
            setEditing(false)
            commit(Number(text))
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") (e.target as HTMLInputElement).blur()
            if (e.key === "Escape") {
              setText(String(value))
              setEditing(false)
            }
          }}
          aria-label={label}
        />
      ) : (
        <button
          type="button"
          className={cn("min-w-12 font-heading font-semibold tabular-nums hover:underline decoration-dotted underline-offset-4", valueClassName)}
          onClick={() => setEditing(true)}
          title={`Click to type a value (${min}–${max})`}
          aria-label={`${label}: ${value}`}
        >
          {value}
        </button>
      )}
      <button type="button" className={btn} disabled={value >= max} onClick={() => commit(value + 1)} aria-label={`Increase ${label}`}>
        <Plus className="size-4" />
      </button>
    </div>
  )
}

export function formatPlayTime(seconds?: number) {
  if (!seconds) return "–"
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  return h > 0 ? `${h}h ${m}m` : `${m}m`
}

/** "21:01:21, 10.11.2026" → Date */
export function parseTimestamp(s?: string): Date | null {
  const m = s && /(\d+):(\d+):(\d+),\s*(\d+)\.(\d+)\.(\d+)/.exec(s)
  if (!m) return null
  const [, h, mi, se, d, mo, y] = m.map(Number)
  return new Date(y, mo - 1, d, h, mi, se)
}
