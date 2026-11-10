import { useEffect, useRef, useState, type ReactNode } from "react"
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

const fmt = (n: number) => n.toLocaleString("en-US")

/**
 * Numeric field with −/+ buttons.
 *
 * The value is always a real <input> with a fixed width, so focusing it never shifts the
 * layout, Tab moves straight from field to field (the −/+ buttons are skipped), and typing
 * replaces the value because it's selected on focus. ↑/↓ step, Enter commits, Esc reverts.
 * Values are clamped to [min, max]; onCommit fires only for actual changes.
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
  const ref = useRef<HTMLInputElement>(null)
  const [focused, setFocused] = useState(false)
  const [text, setText] = useState(fmt(value))
  // Don't clobber what the user is typing; resync once focus leaves.
  useEffect(() => {
    if (!focused) setText(fmt(value))
  }, [value, focused])
  const parse = (t: string) => (t.replace(/,/g, "") === "" ? NaN : Number(t.replace(/,/g, "")))
  const justFocused = useRef(false)
  const skipCommit = useRef(false)

  const commit = (raw: number) => {
    if (!Number.isFinite(raw)) return
    const v = Math.min(max, Math.max(min, Math.round(raw)))
    if (v !== value) onCommit(v)
    setText(fmt(v))
  }
  const btn =
    "grid size-6 shrink-0 place-items-center text-muted-foreground transition-colors hover:text-cyan disabled:pointer-events-none disabled:opacity-30"
  // Wide enough for the largest formatted value; identical focused or not.
  const width = `${fmt(max).length + 0.6}ch`

  return (
    <div className={cn("flex items-center gap-0.5", className)}>
      <button type="button" tabIndex={-1} className={btn} disabled={value <= min} onClick={() => commit(value - 1)} aria-label={`Decrease ${label}`}>
        <Minus className="size-4" />
      </button>
      <input
        ref={ref}
        inputMode="numeric"
        aria-label={label}
        title={`${label} (${fmt(min)}–${fmt(max)})`}
        style={{ width }}
        className={cn(
          "rounded-sm bg-transparent text-center font-heading font-semibold tabular-nums outline-none transition-shadow",
          "hover:shadow-[inset_0_-1px_0_var(--color-cyan-dim)] focus:bg-cyan/5 focus:shadow-[0_0_0_1px_var(--color-cyan-dim)]",
          valueClassName,
        )}
        value={focused ? text : fmt(value)}
        onFocus={(e) => {
          // The text doesn't change on focus (same formatting), so selecting synchronously is
          // safe: the first keystroke replaces the whole value even when typing fast after Tab.
          setFocused(true)
          setText(fmt(value))
          justFocused.current = true
          e.currentTarget.select()
        }}
        onMouseUp={(e) => {
          // keep the select-all from focus instead of placing the caret
          if (justFocused.current) e.preventDefault()
          justFocused.current = false
        }}
        onChange={(e) => setText(e.target.value.replace(/[^\d,]/g, ""))}
        onBlur={() => {
          setFocused(false)
          if (skipCommit.current) skipCommit.current = false
          else commit(parse(text))
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") ref.current?.blur()
          else if (e.key === "Escape") {
            skipCommit.current = true
            ref.current?.blur()
          } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault()
            const typed = parse(text)
            const base = Number.isFinite(typed) ? typed : value
            const next = Math.min(max, Math.max(min, base + (e.key === "ArrowUp" ? 1 : -1)))
            setText(fmt(next))
            if (next !== value) onCommit(next)
            requestAnimationFrame(() => ref.current?.select())
          }
        }}
      />
      <button type="button" tabIndex={-1} className={btn} disabled={value >= max} onClick={() => commit(value + 1)} aria-label={`Increase ${label}`}>
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
