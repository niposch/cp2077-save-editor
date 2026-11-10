import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from "react"
import { toast } from "sonner"
import { SaveSession, initCore, type Change } from "@/lib/core"
import { parseMetadata, type SaveEntry, type SaveFiles, type SaveMetadata } from "@/lib/storage"

export interface OpenSave {
  files: SaveFiles
  meta: SaveMetadata | null
  session: SaveSession
}

interface Ctx {
  open: OpenSave | null
  /** Bumped after every edit so derived views re-read the session. */
  rev: number
  loading: string | null
  openEntry: (e: SaveEntry) => Promise<void>
  close: () => void
  /** Drop all edits by re-parsing the original bytes. */
  discard: () => void
  /** Run an edit; errors become toasts. Returns undefined on failure. */
  mutate: <T>(label: string, fn: (s: SaveSession) => T) => T | undefined
  changes: Change[]
  /** Undo a single pending change. */
  revert: (index: number) => void
}

const SaveCtx = createContext<Ctx | null>(null)

export function SaveProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState<OpenSave | null>(null)
  const [rev, setRev] = useState(0)
  const [loading, setLoading] = useState<string | null>(null)
  const current = useRef<OpenSave | null>(null)

  const replace = (next: OpenSave | null) => {
    current.current?.session.free()
    current.current = next
    // dev-only handle for automated checks from the browser console
    if (import.meta.env.DEV) (window as unknown as { __cpse?: unknown }).__cpse = current
    setOpen(next)
    setRev((r) => r + 1)
  }

  const openEntry = useCallback(async (e: SaveEntry) => {
    setLoading(e.folder)
    try {
      await initCore()
      const files = await e.load()
      // give the spinner a frame before the (synchronous) parse
      await new Promise((r) => setTimeout(r, 16))
      const session = new SaveSession(files.sav)
      replace({ files, meta: parseMetadata(files.metadataText) ?? e.meta, session })
    } catch (err) {
      toast.error(`Couldn't open ${e.folder}`, { description: String(err instanceof Error ? err.message : err) })
    } finally {
      setLoading(null)
    }
  }, [])

  const close = useCallback(() => replace(null), [])

  const discard = useCallback(() => {
    const cur = current.current
    if (!cur) return
    replace({ ...cur, session: new SaveSession(cur.files.sav) })
    toast("Changes discarded")
  }, [])

  const mutate = useCallback(<T,>(label: string, fn: (s: SaveSession) => T): T | undefined => {
    const cur = current.current
    if (!cur) return undefined
    try {
      const r = fn(cur.session)
      setRev((x) => x + 1)
      return r
    } catch (err) {
      toast.error(label, { description: String(err instanceof Error ? err.message : err) })
      setRev((x) => x + 1)
      return undefined
    }
  }, [])

  const changes = useMemo(() => (open ? open.session.changes() : []), [open, rev])

  const revert = useCallback(
    (index: number) => {
      const c = changes[index]
      if (c && mutate(`Couldn't revert ${c.target}`, (s) => (s.revert(index), true))) toast(`Reverted ${c.target}`)
    },
    [changes, mutate],
  )

  return (
    <SaveCtx.Provider value={{ open, rev, loading, openEntry, close, discard, mutate, changes, revert }}>{children}</SaveCtx.Provider>
  )
}

export function useSave() {
  const c = useContext(SaveCtx)
  if (!c) throw new Error("useSave outside SaveProvider")
  return c
}

/** The open save (only call inside the editor). */
export function useOpenSave() {
  const c = useSave()
  if (!c.open) throw new Error("no save open")
  return { ...c, open: c.open }
}
