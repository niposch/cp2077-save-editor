/**
 * Typed wrapper around the Rust core compiled to WebAssembly (crates/wasm).
 * The same module runs in the browser and inside the Tauri desktop shell.
 */
import init, { Session, fact_hash } from "@/wasm/pkg/core"

let ready: Promise<unknown> | null = null
export function initCore() {
  ready ??= init()
  return ready
}

export interface SaveInfo {
  saveVersion: number
  gameVersion: number
  archiveVersion: number
  nodes: number
  dataSize: number
}

export type AttributeKey = "body" | "reflexes" | "tech" | "intelligence" | "cool"
export type PointKey = "attribute" | "perk" | "relic" | "secondary"

export interface PlayerData {
  level: number | null
  streetCred: number | null
  attributes: { key: AttributeKey; stat: string; value: number | null; modifier: number | null }[]
  points: { key: PointKey; label: string; spent: number | null; unspent: number | null }[]
  resetUsed: boolean
}

export interface FactInfo {
  hash: string
  value: number | null
  table: number | null
}

export interface ChunkSummary {
  index: number
  type: string
  props: number
}

export type TreeValue =
  | { k: "struct"; props: { name: string; ty: string; v: TreeValue }[] }
  | { k: "array"; ty: string; items: TreeValue[] }
  | { k: "buffer"; len: number }
  | { k: "scalar"; kind: ScalarKind; text: string }

export type ScalarKind = "int" | "float" | "bool" | "enum" | "cname" | "tdbid" | "str" | "handle"

/** One open save. Mutating methods apply immediately to the in-memory save. */
export class SaveSession {
  private s: Session

  constructor(sav: Uint8Array) {
    this.s = new Session(sav)
  }

  info(): SaveInfo {
    return JSON.parse(this.s.info())
  }
  player(): PlayerData {
    return JSON.parse(this.s.player())
  }
  /** Returns a warning message, or "" */
  setAttribute(key: AttributeKey, value: number): string {
    return this.s.set_attribute(key, value)
  }
  setPoints(key: PointKey, value: number) {
    this.s.set_points(key, value)
  }
  unlockReset(): boolean {
    return this.s.unlock_reset()
  }
  getFact(key: string): FactInfo {
    return JSON.parse(this.s.get_fact(key))
  }
  setFact(key: string, value: number) {
    this.s.set_fact(key, value)
  }
  removeFact(key: string): boolean {
    return this.s.remove_fact(key)
  }
  factCount(): number {
    return this.s.fact_count()
  }
  packageNodes(): string[] {
    return JSON.parse(this.s.package_nodes())
  }
  packageChunks(node: string): ChunkSummary[] {
    return JSON.parse(this.s.package_chunks(node))
  }
  chunk(node: string, index: number): { index: number; type: string; value: TreeValue } {
    return JSON.parse(this.s.chunk(node, index))
  }
  /** Returns the previous value text. */
  setValue(node: string, path: string, value: string): string {
    return this.s.set_value(node, path, value)
  }
  changes(): string[] {
    return JSON.parse(this.s.changes())
  }
  write(): Uint8Array {
    return this.s.write()
  }
  patchMetadata(json: string, fileSize: number): string {
    return this.s.patch_metadata(json, fileSize)
  }
  free() {
    this.s.free()
  }
}

export const factHash = (name: string) => fact_hash(name)
