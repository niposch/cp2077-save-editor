import { useState } from "react"
import { ArrowLeft, Save, Undo2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { SaveBrowser } from "@/components/save-browser"
import { CharacterTab } from "@/components/character-tab"
import { FactsTab } from "@/components/facts-tab"
import { AdvancedTab } from "@/components/advanced-tab"
import { SaveDialog } from "@/components/save-dialog"
import { ChangesPanel, SaveInfoPanel } from "@/components/changes-panel"
import { useSave, useOpenSave } from "@/state/save-context"
import { platform } from "@/lib/storage"

function Brand() {
  return (
    <div className="flex items-baseline gap-2 select-none">
      <span className="hud-label text-lg text-foreground">Save Editor</span>
      <span className="font-heading text-xs tracking-[0.2em] text-salmon uppercase">for Cyberpunk 2077</span>
    </div>
  )
}

const TAB_TRIGGER =
  "hud-label rounded-none px-3 text-[15px] !text-salmon/80 hover:!text-salmon data-active:!text-cyan after:!bg-cyan after:!bottom-[-9px]"

function Editor() {
  const { open, changes, close, discard, player } = useOpenSave()
  const [saveOpen, setSaveOpen] = useState(false)

  return (
    <Tabs defaultValue="character" className="min-h-screen gap-0">
      <header className="sticky top-0 z-10 border-b border-salmon-dim/70 bg-background/85 backdrop-blur">
        <div className="mx-auto flex max-w-[1600px] flex-wrap items-center gap-x-8 gap-y-2 px-6 py-3">
          <div className="flex items-center gap-6">
            <Button variant="ghost" size="icon" onClick={close} aria-label="Back to saves">
              <ArrowLeft />
            </Button>
            <div className="flex items-baseline gap-2">
              <span className="font-heading text-3xl font-semibold text-cyan">{player?.level ?? open.meta?.level ?? "–"}</span>
              <span className="hud-label text-sm text-cyan-dim">Level</span>
            </div>
            <div className="flex items-baseline gap-2">
              <span className="font-heading text-3xl font-semibold text-emerald-300">{player?.streetCred ?? open.meta?.streetCred ?? "–"}</span>
              <span className="hud-label text-sm text-emerald-300/60">Street cred</span>
            </div>
            {player?.money != null && (
              <div className="hidden items-baseline gap-1.5 lg:flex">
                <span className="font-heading text-lg font-semibold text-gold/80">€$</span>
                <span className="font-heading text-3xl font-semibold text-gold tabular-nums">{player.money.toLocaleString("en-US")}</span>
              </div>
            )}
          </div>
          <TabsList variant="line" className="h-9 flex-1 justify-center">
            <TabsTrigger value="character" className={TAB_TRIGGER}>Character</TabsTrigger>
            <TabsTrigger value="facts" className={TAB_TRIGGER}>Facts</TabsTrigger>
            <TabsTrigger value="advanced" className={TAB_TRIGGER}>Advanced</TabsTrigger>
          </TabsList>
          <div className="flex items-center gap-2">
            <span className="hidden font-mono text-xs text-muted-foreground xl:inline">{open.files.folder}</span>
            <Button variant="ghost" size="sm" onClick={discard} disabled={changes.length === 0}>
              <Undo2 /> Discard
            </Button>
            <Button size="sm" onClick={() => setSaveOpen(true)} disabled={changes.length === 0} className="hud-label cut cut-sm">
              <Save /> Save as… {changes.length > 0 && <span className="tabular-nums opacity-70">({changes.length})</span>}
            </Button>
          </div>
        </div>
      </header>
      <main className="mx-auto grid w-full max-w-[1600px] gap-6 px-6 py-8 lg:grid-cols-[minmax(0,1fr)_280px]">
        <div className="min-w-0">
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
        {/* visible on every tab */}
        <aside className="flex flex-col gap-6 self-start lg:sticky lg:top-24 lg:max-h-[calc(100vh-7rem)]">
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
        <div className="mx-auto flex max-w-7xl items-center justify-between px-6 py-4">
          <Brand />
          <span className="font-heading text-xs tracking-widest text-muted-foreground uppercase">
            {platform === "desktop" ? "Desktop" : "Web"} · unofficial fan tool
          </span>
        </div>
      </header>
      <SaveBrowser />
      <footer className="mx-auto max-w-7xl px-6 pb-8 text-xs text-muted-foreground/80">
        MIT licensed. Unofficial fan tool, not affiliated with or endorsed by CD PROJEKT RED. Cyberpunk 2077 is a trademark of CD
        PROJEKT S.A.
      </footer>
    </div>
  )
}
