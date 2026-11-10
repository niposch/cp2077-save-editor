/**
 * Where saves come from and where edited saves go.
 *
 * - desktop (Tauri): lists the game's save folder directly, writes a new save folder next to it
 * - web: the user picks a save folder (or the whole saves folder); results are downloaded as .zip
 * - dev: the Vite dev server exposes the project's copied saves (../saves) for testing
 */
import { invoke, isTauri } from "@tauri-apps/api/core"
import { zipSync } from "fflate"

export type Platform = "desktop" | "web"
export const platform: Platform = isTauri() ? "desktop" : "web"

export interface SaveMetadata {
  name?: string
  level?: number
  streetCred?: number
  playTime?: number
  timestampString?: string
  buildPatch?: string
  gameVersion?: number
  saveVersion?: number
  lifePath?: string
  difficulty?: string
  isModded?: boolean
  facts?: string[]
  [k: string]: unknown
}

/** A save as listed in the browser (contents loaded lazily). */
export interface SaveEntry {
  id: string
  folder: string
  meta: SaveMetadata | null
  /** Lazily produce an object URL for the screenshot. */
  screenshot?: () => Promise<string | null>
  load: () => Promise<SaveFiles>
}

/** Everything in one save folder. */
export interface SaveFiles {
  folder: string
  sav: Uint8Array
  metadataName: string | null
  metadataText: string | null
  /** Other files copied unchanged (screenshot.png, ...). */
  others: { name: string; bytes: Uint8Array }[]
}

export function parseMetadata(text: string | null): SaveMetadata | null {
  if (!text) return null
  try {
    const j = JSON.parse(text)
    return (j?.Data?.metadata as SaveMetadata) ?? null
  } catch {
    return null
  }
}

const isMetadataName = (n: string) => /^metadata(\.\d+)?\.json$/i.test(n)

/** Next free "ManualSave-N" folder name given existing folder names. */
export function suggestName(existing: string[]): string {
  let max = 0
  for (const e of existing) {
    const m = /^ManualSave-(\d+)$/i.exec(e)
    if (m) max = Math.max(max, Number(m[1]))
  }
  return `ManualSave-${max + 1}`
}

/** Build the output folder's files for an edited save. */
export function editedFiles(src: SaveFiles, sav: Uint8Array, metadataText: string | null) {
  const files: { name: string; bytes: Uint8Array }[] = [{ name: "sav.dat", bytes: sav }]
  if (src.metadataName && metadataText != null) {
    files.push({ name: src.metadataName, bytes: new TextEncoder().encode(metadataText) })
  }
  files.push(...src.others)
  return files
}

// ------------------------------------------------------------------ web

/** Group files picked with <input webkitdirectory> (or dropped) into save folders. */
export async function savesFromFileList(list: FileList | File[]): Promise<SaveEntry[]> {
  const byFolder = new Map<string, File[]>()
  for (const f of Array.from(list)) {
    const rel = (f as File & { webkitRelativePath?: string }).webkitRelativePath || f.name
    const key = rel.split("/").slice(0, -1).join("/") || "save"
    if (!byFolder.has(key)) byFolder.set(key, [])
    byFolder.get(key)!.push(f)
  }
  const out: SaveEntry[] = []
  for (const [key, files] of byFolder) {
    const sav = files.find((f) => f.name === "sav.dat")
    if (!sav) continue
    const folder = key.split("/").pop()!
    const metaFile = files.find((f) => isMetadataName(f.name))
    const metaText = metaFile ? await metaFile.text() : null
    const shot = files.find((f) => f.name.toLowerCase() === "screenshot.png")
    const others = files.filter((f) => f !== sav && f !== metaFile && !f.name.startsWith("."))
    out.push({
      id: `web:${key}`,
      folder,
      meta: parseMetadata(metaText),
      screenshot: shot ? async () => URL.createObjectURL(shot) : undefined,
      load: async () => ({
        folder,
        sav: new Uint8Array(await sav.arrayBuffer()),
        metadataName: metaFile?.name ?? null,
        metadataText: metaText,
        others: await Promise.all(others.map(async (f) => ({ name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) }))),
      }),
    })
  }
  return out
}

export function downloadZip(folder: string, files: { name: string; bytes: Uint8Array }[]) {
  const tree: Record<string, Uint8Array> = {}
  for (const f of files) tree[`${folder}/${f.name}`] = f.bytes
  const zip = zipSync(tree, { level: 6 })
  const url = URL.createObjectURL(new Blob([zip as BlobPart], { type: "application/zip" }))
  const a = document.createElement("a")
  a.href = url
  a.download = `${folder}.zip`
  a.click()
  setTimeout(() => URL.revokeObjectURL(url), 10_000)
}

// ------------------------------------------------------------------ desktop (Tauri)

interface DesktopSave {
  folder: string
  path: string
  metadata: string | null
  hasScreenshot: boolean
}

async function readBytes(path: string): Promise<Uint8Array> {
  const buf = await invoke<ArrayBuffer>("read_file", { path })
  return new Uint8Array(buf)
}

export const desktop = {
  defaultSavesDir: () => invoke<string>("default_saves_dir"),

  async listSaves(dir: string): Promise<SaveEntry[]> {
    const list = await invoke<DesktopSave[]>("list_saves", { dir })
    return list.map((s) => ({
      id: `desktop:${s.path}`,
      folder: s.folder,
      meta: parseMetadata(s.metadata),
      screenshot: s.hasScreenshot
        ? async () => URL.createObjectURL(new Blob([(await readBytes(`${s.path}/screenshot.png`)) as BlobPart], { type: "image/png" }))
        : undefined,
      load: async () => {
        const names = await invoke<string[]>("list_files", { dir: s.path })
        const metaName = names.find(isMetadataName) ?? null
        const others = names.filter((n) => n !== "sav.dat" && n !== metaName)
        return {
          folder: s.folder,
          sav: await readBytes(`${s.path}/sav.dat`),
          metadataName: metaName,
          metadataText: metaName ? new TextDecoder().decode(await readBytes(`${s.path}/${metaName}`)) : null,
          others: await Promise.all(others.map(async (n) => ({ name: n, bytes: await readBytes(`${s.path}/${n}`) }))),
        }
      },
    }))
  },

  /** Writes `<parent>/<folder>/...`; the Rust side refuses to overwrite anything. */
  async writeSave(parent: string, folder: string, files: { name: string; bytes: Uint8Array }[]) {
    await invoke("create_save_dir", { parent, folder })
    for (const f of files) {
      await invoke("write_new_file", f.bytes, { headers: { "x-parent": encodeURIComponent(parent), "x-folder": encodeURIComponent(folder), "x-name": encodeURIComponent(f.name) } })
    }
    return `${parent}/${folder}`
  },
}

// ------------------------------------------------------------------ dev

export const dev = {
  available: import.meta.env.DEV,
  async listSaves(): Promise<SaveEntry[]> {
    const r = await fetch("/__dev/saves")
    if (!r.ok) return []
    const dirs: string[] = await r.json()
    const get = async (d: string, f: string) => {
      const res = await fetch(`/__dev/saves/${encodeURIComponent(d)}/${f}`)
      return res.ok ? new Uint8Array(await res.arrayBuffer()) : null
    }
    return Promise.all(
      dirs.map(async (d) => {
        const metaBytes = await get(d, "metadata.9.json")
        const metaText = metaBytes ? new TextDecoder().decode(metaBytes) : null
        return {
          id: `dev:${d}`,
          folder: d,
          meta: parseMetadata(metaText),
          screenshot: async () => `/__dev/saves/${encodeURIComponent(d)}/screenshot.png`,
          load: async () => ({
            folder: d,
            sav: (await get(d, "sav.dat"))!,
            metadataName: metaText ? "metadata.9.json" : null,
            metadataText: metaText,
            others: [{ name: "screenshot.png", bytes: (await get(d, "screenshot.png")) ?? new Uint8Array() }].filter((f) => f.bytes.length),
          }),
        } satisfies SaveEntry
      }),
    )
  },
}
