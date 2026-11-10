import { useEffect, useState } from "react"
import { Download, FolderOpen, Loader2, Save } from "lucide-react"
import { toast } from "sonner"
import { open as openDialog } from "@tauri-apps/plugin-dialog"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Checkbox } from "@/components/ui/checkbox"
import { CompatibilityNotice } from "@/components/changes-panel"
import { useOpenSave } from "@/state/save-context"
import { desktop, downloadZip, editedFiles, platform, suggestName } from "@/lib/storage"

const VALID = /^[A-Za-z0-9 _.-]+$/

/** Once confirmed, the backup checkbox stays ticked for the rest of the session. */
let backupConfirmed = false

export function SaveDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (o: boolean) => void }) {
  const save = useOpenSave()
  const [name, setName] = useState("")
  const [parent, setParent] = useState<string | null>(null)
  const [existing, setExisting] = useState<string[]>([])
  const [busy, setBusy] = useState(false)
  const [backedUp, setBackedUp] = useState(backupConfirmed)

  useEffect(() => {
    if (!open) return
    if (platform === "desktop") {
      ;(async () => {
        const p = parent ?? (await desktop.defaultSavesDir())
        setParent(p)
        const list = await desktop.listSaves(p).catch(() => [])
        const names = list.map((s) => s.folder)
        setExisting(names)
        setName(suggestName(names))
      })()
    } else {
      setName(suggestName([save.open.files.folder]))
    }
  }, [open]) // eslint-disable-line react-hooks/exhaustive-deps

  const clash = existing.includes(name)
  const valid = VALID.test(name) && !clash

  const pickParent = async () => {
    const d = await openDialog({ directory: true, defaultPath: parent ?? undefined, title: "Where should the new save folder go?" })
    if (typeof d === "string") {
      setParent(d)
      const names = (await desktop.listSaves(d).catch(() => [])).map((s) => s.folder)
      setExisting(names)
    }
  }

  const run = async () => {
    setBusy(true)
    try {
      await new Promise((r) => setTimeout(r, 16))
      const { session, files } = save.open
      const sav = session.write()
      const meta = files.metadataText != null ? session.patchMetadata(files.metadataText, sav.length) : null
      const out = editedFiles(files, sav, meta)
      if (platform === "desktop") {
        const path = await desktop.writeSave(parent!, name, out)
        toast.success("Save written", { description: path })
      } else {
        downloadZip(name, out)
        toast.success(`Downloaded ${name}.zip`, { description: "Extract it into your saves folder." })
      }
      onOpenChange(false)
    } catch (e) {
      toast.error("Couldn't write the save", { description: String(e instanceof Error ? e.message : e) })
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="hud-label text-xl text-salmon">Save as new save</DialogTitle>
          <DialogDescription>
            The original save is never modified. A new save folder is {platform === "desktop" ? "created" : "downloaded as a .zip"} with your{" "}
            {save.changes.length} change{save.changes.length === 1 ? "" : "s"}.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <CompatibilityNotice />
          <div className="space-y-1.5">
            <Label htmlFor="save-name">Folder name</Label>
            <Input id="save-name" value={name} onChange={(e) => setName(e.target.value)} className="font-mono" spellCheck={false} />
            {clash && <p className="text-xs text-destructive">A save with this name already exists there.</p>}
            {!VALID.test(name) && name && <p className="text-xs text-destructive">Use letters, digits, space, dot, dash or underscore.</p>}
          </div>
          {platform === "desktop" ? (
            <div className="space-y-1.5">
              <Label>Location</Label>
              <div className="flex items-center gap-2">
                <span className="flex-1 truncate font-mono text-xs text-muted-foreground" title={parent ?? ""}>
                  {parent ?? "…"}
                </span>
                <Button size="sm" variant="outline" onClick={pickParent}>
                  <FolderOpen /> Change
                </Button>
              </div>
            </div>
          ) : (
            <p className="text-xs text-muted-foreground">
              Extract the zip into your saves folder so it contains <span className="font-mono">{name || "…"}/sav.dat</span>.
            </p>
          )}
          <label className="flex cursor-pointer items-start gap-3 border border-gold/40 bg-gold/[0.06] p-3 text-sm">
            <Checkbox
              checked={backedUp}
              onCheckedChange={(c) => {
                backupConfirmed = c === true
                setBackedUp(backupConfirmed)
              }}
              className="mt-0.5"
            />
            <span>
              <span className="font-medium text-gold">I have a backup of my saves folder.</span>{" "}
              <span className="text-muted-foreground">
                Edited saves are untested territory; if the game misbehaves, you'll want your original saves back.
              </span>
            </span>
          </label>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={run} disabled={!valid || !backedUp || busy || (platform === "desktop" && !parent)} className="hud-label">
            {busy ? <Loader2 className="animate-spin" /> : platform === "desktop" ? <Save /> : <Download />}
            {platform === "desktop" ? "Write save" : "Download"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
