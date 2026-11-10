import { Archive } from "lucide-react"
import { platform } from "@/lib/storage"
import { cn } from "@/lib/utils"

export const SAVES_DIR_HINT = String.raw`%USERPROFILE%\Saved Games\CD Projekt Red\Cyberpunk 2077`

/** Reminder shown on the start screen: back up saves before editing. */
export function BackupNotice({ className }: { className?: string }) {
  return (
    <div className={cn("flex gap-3 border border-gold/40 bg-gold/[0.06] px-4 py-3 text-sm", className)} role="note">
      <Archive className="mt-0.5 size-5 shrink-0 text-gold" />
      <div className="space-y-1">
        <p className="hud-label text-gold">Back up your saves first</p>
        <p className="text-foreground/85">
          This is an unofficial tool working on a reverse-engineered file format. It never changes your existing saves
          {platform === "desktop" ? " (edits always go into a new save folder)" : " (edits are downloaded as a new save)"}, but
          please keep a copy of your whole saves folder anyway, before you load an edited save in the game.
        </p>
        <p className="text-muted-foreground">
          Copy the folder <span className="font-mono text-xs text-foreground/80">{SAVES_DIR_HINT}</span> somewhere safe, e.g. a zip on another drive.
          Steam/GOG cloud saves sync this same folder, so they're not a substitute for your own copy.
        </p>
      </div>
    </div>
  )
}
