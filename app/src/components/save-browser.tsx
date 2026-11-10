import { useEffect, useMemo, useRef, useState } from "react"
import { FolderOpen, Loader2, RefreshCw, ShieldCheck, ImageOff, FlaskConical, ExternalLink } from "lucide-react"
import { open as openDialog } from "@tauri-apps/plugin-dialog"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { HudHeading, formatPlayTime, parseTimestamp } from "@/components/hud"
import { desktop, dev, listExamples, platform, savesFromFileList, type ExampleEntry, type SaveEntry } from "@/lib/storage"
import { useSave } from "@/state/save-context"
import { BackupNotice, SAVES_DIR_HINT } from "@/components/backup-notice"
import { cn } from "@/lib/utils"

function sortSaves(list: SaveEntry[]) {
  return [...list].sort((a, b) => (parseTimestamp(b.meta?.timestampString)?.getTime() ?? 0) - (parseTimestamp(a.meta?.timestampString)?.getTime() ?? 0))
}

function Thumb({ entry }: { entry: SaveEntry }) {
  const [url, setUrl] = useState<string | null>(null)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!entry.screenshot || !ref.current) return
    let cancelled = false
    let created: string | null = null
    const io = new IntersectionObserver(async ([e]) => {
      if (!e.isIntersecting) return
      io.disconnect()
      const u = await entry.screenshot!()
      if (cancelled) return
      if (u?.startsWith("blob:")) created = u
      setUrl(u)
    })
    io.observe(ref.current)
    return () => {
      cancelled = true
      io.disconnect()
      if (created) URL.revokeObjectURL(created)
    }
  }, [entry])
  return (
    <div ref={ref} className="relative aspect-video overflow-hidden bg-black/40">
      {url ? (
        <img src={url} alt="" className="size-full object-cover opacity-85 transition-opacity group-hover:opacity-100" />
      ) : (
        <div className="grid size-full place-items-center text-muted-foreground/40">
          <ImageOff className="size-6" />
        </div>
      )}
      <div className="absolute inset-x-0 bottom-0 h-1/2 bg-gradient-to-t from-black/80 to-transparent" />
    </div>
  )
}

function SaveCard({ entry, onOpen, busy }: { entry: SaveEntry; onOpen: () => void; busy: boolean }) {
  const m = entry.meta
  const ts = parseTimestamp(m?.timestampString)
  const example = "example" in entry ? (entry as ExampleEntry).example : null
  return (
    <button
      type="button"
      onClick={onOpen}
      disabled={busy}
      className="group hud-panel !p-0 text-left transition-transform hover:-translate-y-0.5 focus-visible:outline-2 focus-visible:outline-cyan disabled:opacity-70"
    >
      <Thumb entry={entry} />
      <div className="relative -mt-9 px-3 pb-3">
        <div className="flex items-end justify-between gap-2">
          <span className="hud-label truncate text-lg text-foreground drop-shadow">{example?.title ?? entry.folder}</span>
          {busy && <Loader2 className="mb-1 size-4 animate-spin text-cyan" />}
        </div>
        <div className="mt-2 flex items-baseline gap-4 font-heading">
          <span>
            <span className="text-xl font-semibold text-cyan">{m?.level ?? "–"}</span>{" "}
            <span className="text-xs uppercase tracking-wider text-cyan-dim">Level</span>
          </span>
          <span>
            <span className="text-xl font-semibold text-emerald-300">{m?.streetCred ?? "–"}</span>{" "}
            <span className="text-xs uppercase tracking-wider text-emerald-300/60">Street cred</span>
          </span>
        </div>
        <div className="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
          <span>{ts ? ts.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : "–"}</span>
          <span>{formatPlayTime(m?.playTime)} played</span>
        </div>
        <div className="mt-2 flex flex-wrap gap-1.5">
          {m?.buildPatch && <Badge variant="outline" className="border-salmon-dim text-salmon">{m.buildPatch}</Badge>}
          {m?.lifePath && <Badge variant="outline">{m.lifePath}</Badge>}
          {m?.difficulty && <Badge variant="outline">{m.difficulty}</Badge>}
        </div>
        {example && (
          <p className="mt-2 text-xs text-muted-foreground">
            by <span className="text-foreground/85">{example.author}</span> · {example.source}
          </p>
        )}
      </div>
    </button>
  )
}

function SaveGrid({ saves, title, right }: { saves: SaveEntry[]; title: string; right?: React.ReactNode }) {
  const { openEntry, loading } = useSave()
  const sorted = useMemo(() => sortSaves(saves), [saves])
  return (
    <section>
      <HudHeading right={right}>{title}</HudHeading>
      {sorted.length === 0 ? (
        <p className="text-sm text-muted-foreground">No saves found here.</p>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-4">
          {sorted.map((s) => (
            <SaveCard key={s.id} entry={s} busy={loading === s.folder} onOpen={() => openEntry(s)} />
          ))}
        </div>
      )}
    </section>
  )
}

function DesktopSource() {
  const [dir, setDir] = useState<string | null>(null)
  const [saves, setSaves] = useState<SaveEntry[] | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = async (d: string) => {
    setSaves(null)
    setError(null)
    try {
      setSaves(await desktop.listSaves(d))
    } catch (e) {
      setError(String(e))
      setSaves([])
    }
  }
  useEffect(() => {
    desktop.defaultSavesDir().then((d) => {
      setDir(d)
      load(d)
    })
  }, [])

  const pick = async () => {
    const d = await openDialog({ directory: true, defaultPath: dir ?? undefined, title: "Choose a Cyberpunk 2077 saves folder" })
    if (typeof d === "string") {
      setDir(d)
      load(d)
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
        <span className="font-mono text-xs break-all">{dir ?? "…"}</span>
        <Button size="sm" variant="outline" onClick={pick}>
          <FolderOpen /> Change folder
        </Button>
        <Button size="sm" variant="ghost" onClick={() => dir && load(dir)} aria-label="Refresh">
          <RefreshCw />
        </Button>
      </div>
      {error && <p className="text-sm text-destructive">{error}</p>}
      {saves === null ? <Loader2 className="size-5 animate-spin text-cyan" /> : <SaveGrid title="Your saves" saves={saves} />}
    </div>
  )
}

function WebSource() {
  const input = useRef<HTMLInputElement>(null)
  const [saves, setSaves] = useState<SaveEntry[] | null>(null)
  const [dragging, setDragging] = useState(false)
  const { openEntry } = useSave()

  const accept = async (files: FileList | File[]) => {
    const found = await savesFromFileList(files)
    setSaves(found)
    // a single picked save opens straight away
    if (found.length === 1) openEntry(found[0])
  }

  return (
    <div className="space-y-8">
      <div
        className={cn(
          "hud-panel flex flex-col items-center gap-4 px-6 py-10 text-center transition-colors",
          dragging && "is-active",
        )}
        onDragOver={(e) => {
          e.preventDefault()
          setDragging(true)
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={async (e) => {
          e.preventDefault()
          setDragging(false)
          const files = await filesFromDrop(e.dataTransfer)
          if (files.length) accept(files)
        }}
      >
        <h1 className="hud-label text-3xl text-foreground">Open a save</h1>
        <p className="max-w-xl text-sm text-muted-foreground">
          Pick a single save folder (e.g. <span className="font-mono text-xs">ManualSave-17</span>) or the whole saves folder. Saves are in
          <span className="mt-1 block font-mono text-xs text-foreground/80">{SAVES_DIR_HINT}</span>
        </p>
        <Button size="lg" onClick={() => input.current?.click()} className="hud-label cut cut-sm">
          <FolderOpen /> Choose folder
        </Button>
        <p className="text-xs text-muted-foreground">or drop a folder here</p>
        <p className="flex items-center gap-1.5 text-xs text-emerald-300/80">
          <ShieldCheck className="size-3.5" /> Everything runs locally in your browser. Nothing is uploaded.
        </p>
        <input
          ref={input}
          type="file"
          className="hidden"
          // @ts-expect-error non-standard but widely supported directory picker
          webkitdirectory=""
          multiple
          onChange={(e) => e.target.files && accept(e.target.files)}
        />
      </div>
      {saves && <SaveGrid title={`Found ${saves.length} save${saves.length === 1 ? "" : "s"}`} saves={saves} />}
    </div>
  )
}

/** Recursively collect files from a drag & drop of folders, keeping relative paths. */
async function filesFromDrop(dt: DataTransfer): Promise<File[]> {
  const out: File[] = []
  const walk = async (entry: FileSystemEntry, prefix: string): Promise<void> => {
    if (entry.isFile) {
      const f = await new Promise<File>((res, rej) => (entry as FileSystemFileEntry).file(res, rej))
      Object.defineProperty(f, "webkitRelativePath", { value: prefix + f.name })
      out.push(f)
    } else if (entry.isDirectory) {
      const reader = (entry as FileSystemDirectoryEntry).createReader()
      let batch: FileSystemEntry[]
      do {
        batch = await new Promise((res, rej) => reader.readEntries(res, rej))
        for (const e of batch) await walk(e, `${prefix}${entry.name}/`)
      } while (batch.length)
    }
  }
  const entries = Array.from(dt.items)
    .map((i) => i.webkitGetAsEntry())
    .filter((e): e is FileSystemEntry => !!e)
  for (const e of entries) await walk(e, "")
  return out
}

/** Example saves bundled with the website, so visitors can try the editor without a save. */
function ExampleSource() {
  const [examples, setExamples] = useState<ExampleEntry[]>([])
  useEffect(() => {
    listExamples().then(setExamples)
  }, [])
  if (!examples.length) return null
  return (
    <div className="space-y-3">
      <SaveGrid title="Try an example save" saves={examples} />
      <p className="text-xs text-muted-foreground">
        Example saves are shared by their authors and aren't covered by this site's license.{" "}
        {examples.map((e, i) => (
          <span key={e.id}>
            {i > 0 && " · "}
            <a href={e.example.sourceUrl} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 text-cyan-dim hover:text-cyan">
              {e.example.title} by {e.example.author} <ExternalLink className="size-3" />
            </a>
          </span>
        ))}
      </p>
    </div>
  )
}

function DevSource() {
  const [saves, setSaves] = useState<SaveEntry[] | null>(null)
  useEffect(() => {
    dev.listSaves().then(setSaves).catch(() => setSaves([]))
  }, [])
  if (!saves?.length) return null
  return (
    <SaveGrid
      title="Dev · project saves"
      saves={saves}
      right={
        <Badge variant="outline" className="gap-1 border-gold/50 text-gold">
          <FlaskConical className="size-3" /> dev server only
        </Badge>
      }
    />
  )
}

export function SaveBrowser() {
  return (
    <div className="mx-auto w-full max-w-7xl space-y-10 px-6 py-8">
      <BackupNotice />
      {platform === "desktop" ? <DesktopSource /> : <WebSource />}
      {platform === "web" && <ExampleSource />}
      {dev.available && <DevSource />}
    </div>
  )
}
