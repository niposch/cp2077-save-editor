import { useState } from "react"
import { ArrowLeft, Save, Undo2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { SaveBrowser } from "@/components/save-browser"
import { CharacterTab } from "@/components/character-tab"
import { FactsTab } from "@/components/facts-tab"
import { AdvancedTab } from "@/components/advanced-tab"
import { SaveDialog } from "@/components/save-dialog"
import { ChangesPanel, CompatibilityNotice, SaveInfoPanel } from "@/components/changes-panel"
import { useSave, useOpenSave } from "@/state/save-context"
import { platform } from "@/lib/storage"

function Brand() {
  return (
    <div className="flex flex-col select-none sm:flex-row sm:items-baseline sm:gap-2">
      <span className="hud-label text-lg leading-tight text-foreground">Save Editor</span>
      <span className="font-heading text-xs tracking-[0.2em] text-salmon uppercase">for Cyberpunk 2077</span>
    </div>
  )
}

const TAB_TRIGGER =
  "hud-label rounded-none px-3 text-[15px] !text-salmon/80 hover:!text-salmon data-active:!text-cyan after:!bg-cyan after:!bottom-[-9px]"

/**
 * A header number in a fixed-width, right-aligned box: changing the value never moves the
 * label or anything else in the header (Rajdhani's digits aren't all the same width).
 */
function Stat({ value, chars, label, shortLabel, color, labelColor, prefix }: { value: string; chars: number; label?: string; shortLabel?: string; color: string; labelColor?: string; prefix?: string }) {
  return (
    <div className="flex items-baseline gap-1.5">
      {/* the prefix sits inside the fixed box so it stays next to the digits */}
      <span className={`inline-block text-right font-heading text-2xl font-semibold whitespace-nowrap tabular-nums sm:text-3xl ${color}`} style={{ width: `${chars + 0.3 + (prefix ? prefix.length * 0.5 + 0.4 : 0)}ch` }}>
        {prefix && <span className={`mr-1.5 text-base sm:text-lg ${labelColor}`}>{prefix}</span>}
        {value}
      </span>
      {label && (
        <span className={`hud-label text-xs whitespace-nowrap sm:text-sm ${labelColor}`}>
          {shortLabel ? (
            <>
              <span className="sm:hidden">{shortLabel}</span>
              <span className="hidden sm:inline">{label}</span>
            </>
          ) : (
            label
          )}
        </span>
      )}
    </div>
  )
}

function Editor() {
  const { open, changes, close, discard, player } = useOpenSave()
  const [saveOpen, setSaveOpen] = useState(false)
  const level = String(player?.level ?? open.meta?.level ?? "–")
  const cred = String(player?.streetCred ?? open.meta?.streetCred ?? "–")

  return (
    <Tabs defaultValue="character" className="min-h-screen gap-0">
      <header className="sticky top-0 z-10 border-b border-salmon-dim/70 bg-background/85 backdrop-blur">
        {/* phones: stats + actions, then tabs; desktop: stats | tabs (centred) | actions */}
        <div className="mx-auto grid max-w-[1600px] grid-cols-[1fr_auto] items-center gap-x-4 gap-y-1 px-4 py-2 sm:px-6 sm:py-3 lg:grid-cols-[1fr_auto_1fr]">
          <div className="col-start-1 row-start-1 flex min-w-0 items-center gap-3 sm:gap-6">
            <Button variant="ghost" size="icon" onClick={close} aria-label="Back to saves" className="-ml-2 shrink-0">
              <ArrowLeft />
            </Button>
            <Stat value={level} chars={2} label="Level" color="text-cyan" labelColor="text-cyan-dim" />
            <Stat value={cred} chars={2} label="Street cred" shortLabel="Cred" color="text-emerald-300" labelColor="text-emerald-300/60" />
            {player?.money != null && (
              <div className="hidden xl:block">
                <Stat value={player.money.toLocaleString("en-US")} chars={11} prefix="€$" color="text-gold" labelColor="text-gold/80" />
              </div>
            )}
          </div>
          <TabsList variant="line" className="col-span-2 row-start-2 h-9 w-full justify-around sm:justify-center lg:col-span-1 lg:col-start-2 lg:row-start-1 lg:w-auto">
            <TabsTrigger value="character" className={TAB_TRIGGER}>Character</TabsTrigger>
            <TabsTrigger value="facts" className={TAB_TRIGGER}>Facts</TabsTrigger>
            <TabsTrigger value="advanced" className={TAB_TRIGGER}>Advanced</TabsTrigger>
          </TabsList>
          <div className="col-start-2 row-start-1 flex items-center justify-end gap-1 sm:gap-2 lg:col-start-3">
            <span className="hidden max-w-40 truncate font-mono text-xs text-muted-foreground 2xl:inline">{open.files.folder}</span>
            <Button variant="ghost" size="sm" onClick={discard} disabled={changes.length === 0} aria-label="Discard all changes">
              <Undo2 /> <span className="hidden sm:inline">Discard</span>
            </Button>
            <Button size="sm" onClick={() => setSaveOpen(true)} disabled={changes.length === 0} className="hud-label cut cut-sm">
              <Save /> Save<span className="hidden sm:inline"> as…</span>
              <span className="inline-block w-[3ch] text-left tabular-nums opacity-70">{changes.length > 0 ? `(${changes.length})` : ""}</span>
            </Button>
          </div>
        </div>
      </header>
      <main className="mx-auto grid w-full max-w-[1600px] gap-6 px-4 py-6 sm:px-6 sm:py-8 lg:grid-cols-[minmax(0,1fr)_280px]">
        <div className="min-w-0">
          <CompatibilityNotice className="mb-6" />
          <TabsContent value="character">
            <CharacterTab />
          </TabsContent>
          <TabsContent value="facts">
            <FactsTab />
          </TabsContent>
          <TabsContent value="advanced">
            <AdvancedTab />
          </TabsContent>
        </div>
        {/* visible on every tab (below the tab content on narrow screens) */}
        <aside className="flex min-w-0 flex-col gap-6 self-start lg:sticky lg:top-24 lg:max-h-[calc(100vh-7rem)]">
          <ChangesPanel />
          <SaveInfoPanel />
        </aside>
      </main>
      <SaveDialog open={saveOpen} onOpenChange={setSaveOpen} />
    </Tabs>
  )
}

export default function App() {
  const { open } = useSave()
  if (open) return <Editor />
  return (
    <div className="min-h-screen">
      <header className="border-b border-salmon-dim/70">
        <div className="mx-auto flex max-w-7xl items-center justify-between gap-4 px-4 py-3 sm:px-6 sm:py-4">
          <Brand />
          <span className="hidden font-heading text-xs tracking-widest text-muted-foreground uppercase sm:inline">
            {platform === "desktop" ? "Desktop" : "Web"} · unofficial fan tool
          </span>
        </div>
      </header>
      <SaveBrowser />
      <footer className="mx-auto max-w-7xl px-4 pb-8 text-xs text-muted-foreground/80 sm:px-6">
        <a href="https://github.com/niposch/cp2077-save-editor" target="_blank" rel="noreferrer" className="text-cyan-dim hover:text-cyan">
          Source on GitHub
        </a>{" "}
        · MIT licensed. Unofficial fan tool, not affiliated with or endorsed by CD PROJEKT RED. Cyberpunk 2077 is a trademark of CD
        PROJEKT S.A.
      </footer>
    </div>
  )
}
