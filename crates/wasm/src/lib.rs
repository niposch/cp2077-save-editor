//! WebAssembly bindings used by the React UI (web and desktop).
//!
//! Everything crosses the boundary as JSON strings or byte arrays to keep the interface small.
//! A `Session` holds one open save; edits are applied in memory and `write()` produces the new
//! `sav.dat`.

use cp2077_save::{
    edit::{scalar_str, SetResult},
    facts::parse_fact_key,
    metadata::{json_string, patch_for_save},
    package::{array_elem, PackageNode, Struct, Value},
    inventory::MONEY_MAX,
    player::{resolve_attribute, resolve_life_path, resolve_point_kind, resolve_progression, ATTRIBUTES, POINT_KINDS},
    SaveFile,
};
use serde_json::{json, Value as J};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

type R<T> = Result<T, JsError>;

fn err(e: anyhow::Error) -> JsError {
    JsError::new(&format!("{e:#}"))
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Package node names recognised by their header (`u32 id, u32 len, u16 4, u16 6|7`).
fn is_package(b: &[u8]) -> bool {
    b.len() >= 16 && b[8..10] == [4, 0] && (b[10] == 6 || b[10] == 7)
}

fn value_json(v: &Value, ty: &str) -> J {
    match v {
        Value::Struct(s) => struct_json(s),
        Value::Array(a) => {
            let et = array_elem(ty).unwrap_or(ty);
            json!({ "k": "array", "ty": et, "items": a.iter().map(|e| value_json(e, et)).collect::<Vec<_>>() })
        }
        Value::Buffer(b) => json!({ "k": "buffer", "len": b.len() }),
        other => {
            let kind = match other {
                Value::Bool(_) => "bool",
                Value::F32(_) | Value::F64(_) => "float",
                Value::Enum(_) => "enum",
                Value::CName(_) => "cname",
                Value::TweakDBID(_) => "tdbid",
                Value::Str(_) => "str",
                Value::Handle(_) => "handle",
                _ => "int",
            };
            json!({ "k": "scalar", "kind": kind, "text": scalar_str(other).unwrap_or_default() })
        }
    }
}

fn struct_json(s: &Struct) -> J {
    json!({
        "k": "struct",
        "props": s.props.iter().map(|p| json!({ "name": p.name, "ty": p.ty, "v": value_json(&p.value, &p.ty) })).collect::<Vec<_>>()
    })
}

/// One user edit, kept in a replayable form so individual edits can be reverted.
#[derive(Debug, Clone)]
enum Op {
    Attribute(String, i32),
    Points(String, i32),
    Progression(String, i32),
    LifePath(String),
    Money(u32),
    UnlockReset,
    SetFact(String, u32),
    RemoveFact(String),
    SetValue { node: String, path: String, value: String },
}

/// A pending change as shown in the UI: `target: old → new`.
#[derive(Debug, Clone)]
struct Change {
    target: String,
    /// Value before the first edit of this target.
    old: String,
    /// Value after the latest edit.
    new: String,
    /// The latest edit of this target (replayed when other changes are reverted).
    op: Op,
}

struct Applied {
    target: String,
    old: String,
    new: String,
    warning: Option<String>,
}

#[wasm_bindgen]
pub struct Session {
    /// The unedited `sav.dat`, for reverting individual changes.
    original: Vec<u8>,
    save: SaveFile,
    /// Extra metadata.9.json fields to update on write (raw JSON values).
    meta: Vec<(String, String)>,
    /// Pending changes, one per target, in first-edit order. Edits returning a target to its
    /// original value drop the change.
    changes: Vec<Change>,
    cache: HashMap<String, PackageNode>,
}

impl Session {
    fn package(&mut self, node: &str) -> anyhow::Result<&PackageNode> {
        if !self.cache.contains_key(node) {
            let pn = self.save.package(node)?;
            self.cache.insert(node.to_string(), pn);
        }
        Ok(&self.cache[node])
    }

    fn set_meta(&mut self, key: &str, value: String) {
        self.meta.retain(|(k, _)| k != key);
        self.meta.push((key.to_string(), value));
    }

    fn fact_value(&self, key: &str) -> anyhow::Result<String> {
        Ok(self.save.get_fact(parse_fact_key(key)?)?.map_or("unset".into(), |f| f.1.to_string()))
    }

    /// Apply one edit to the save (and metadata), describing what changed.
    fn apply(&mut self, op: &Op) -> anyhow::Result<Applied> {
        let a = |target: &str, old: String, new: String, warning: Option<String>| Applied { target: target.to_string(), old, new, warning };
        Ok(match op {
            Op::Attribute(name, v) => {
                let (stat, meta_key) = resolve_attribute(name)?;
                let r = self.save.set_attribute(stat, *v)?;
                self.set_meta(meta_key, format!("{v}.0"));
                a(stat, r.old.map_or("default".into(), |o| o.to_string()), v.to_string(), r.warning)
            }
            Op::Progression(key, v) => {
                let (ty, _, meta_key) = resolve_progression(key)?;
                let r = self.save.set_progression(ty, *v)?;
                self.set_meta(meta_key, format!("{v}.0"));
                let label = if ty == "Level" { "Level" } else { "Street cred" };
                a(label, r.old.map_or("default".into(), |o| o.to_string()), v.to_string(), r.warning)
            }
            Op::Points(kind, v) => {
                let ty = resolve_point_kind(kind)?;
                let old = self.save.set_unspent_points(ty, *v)?;
                a(&format!("{kind} points"), old.unwrap_or(0).to_string(), v.to_string(), None)
            }
            Op::LifePath(name) => {
                let lp = resolve_life_path(name)?;
                let old = self.save.set_life_path(lp)?;
                self.set_meta("lifePath", json_string(lp));
                a("Life path", old, lp.to_string(), None)
            }
            Op::Money(v) => {
                let old = self.save.set_money(*v)?;
                a("Money", old.to_string(), v.to_string(), None)
            }
            Op::UnlockReset => {
                let removed = self.save.remove_fact(parse_fact_key("ResetAttributeDisabled")?)?;
                let old = if removed { "used" } else { "available" };
                a("Attribute reset", old.into(), "available".into(), None)
            }
            Op::SetFact(key, v) => {
                let old = self.fact_value(key)?;
                self.save.set_fact(parse_fact_key(key)?, *v)?;
                a(&format!("fact {key}"), old, v.to_string(), None)
            }
            Op::RemoveFact(key) => {
                let old = self.fact_value(key)?;
                self.save.remove_fact(parse_fact_key(key)?)?;
                a(&format!("fact {key}"), old, "unset".into(), None)
            }
            Op::SetValue { node, path, value } => {
                let r = self.save.edit_package(node, |p| p.set_path(path, value))?;
                let old = match r {
                    SetResult::Changed { old } => scalar_str(&old).unwrap_or_default(),
                    SetResult::Inserted => "(default)".into(),
                };
                a(&format!("{node}:{path}"), old, value.clone(), None)
            }
        })
    }

    /// Apply an edit and record it in the change list. Returns (old value, warning).
    fn record(&mut self, op: Op) -> anyhow::Result<(String, Option<String>)> {
        let r = self.apply(&op)?;
        self.cache.clear();
        match self.changes.iter().position(|c| c.target == r.target) {
            Some(i) if self.changes[i].old == r.new => {
                self.changes.remove(i);
            }
            Some(i) => {
                self.changes[i].new = r.new.clone();
                self.changes[i].op = op;
            }
            // edits that change nothing (e.g. restoring an already available reset) aren't listed
            None if r.old == r.new => {}
            None => self.changes.push(Change { target: r.target.clone(), old: r.old.clone(), new: r.new, op }),
        }
        Ok((r.old, r.warning))
    }

    fn fresh(original: Vec<u8>) -> anyhow::Result<Session> {
        let save = SaveFile::parse(&original)?;
        Ok(Session { original, save, meta: vec![], changes: vec![], cache: HashMap::new() })
    }
}

#[wasm_bindgen]
impl Session {
    /// Parse a `sav.dat`.
    #[wasm_bindgen(constructor)]
    pub fn new(sav: &[u8]) -> R<Session> {
        Session::fresh(sav.to_vec()).map_err(err)
    }

    /// `{ saveVersion, gameVersion, archiveVersion, nodes, dataSize }`
    pub fn info(&self) -> String {
        let h = &self.save.header;
        json!({
            "saveVersion": h.save_version,
            "gameVersion": h.game_version,
            "archiveVersion": h.archive_version,
            "nodes": self.save.nodes.len(),
            "dataSize": self.save.data.len(),
        })
        .to_string()
    }

    /// Player summary: level, street cred, attributes (both copies), point pools, reset status.
    pub fn player(&self) -> R<String> {
        let p = self.save.player_summary().map_err(err)?;
        let reset = self.save.get_fact(parse_fact_key("ResetAttributeDisabled").map_err(err)?).map_err(err)?;
        let attrs: Vec<J> = ATTRIBUTES
            .iter()
            .zip(&p.attributes)
            .map(|((names, stat, _), (_, v, m))| json!({ "key": names[0], "stat": stat, "value": v, "modifier": m }))
            .collect();
        let points: Vec<J> = p
            .points
            .iter()
            .map(|(desc, spent, unspent)| {
                let key = POINT_KINDS.iter().find(|(_, _, d)| d == desc).map(|(n, _, _)| n[0]).unwrap_or("");
                json!({ "key": key, "label": desc, "spent": spent, "unspent": unspent })
            })
            .collect();
        Ok(json!({
            "level": p.level,
            "streetCred": p.street_cred,
            "attributes": attrs,
            "points": points,
            "resetUsed": reset.is_some_and(|r| r.1 != 0),
            "lifePath": p.life_path,
            "money": self.save.money().ok().flatten(),
            "moneyMax": MONEY_MAX,
        })
        .to_string())
    }

    /// Set an attribute (body/reflexes/tech/intelligence/cool). Returns a warning or "".
    pub fn set_attribute(&mut self, name: &str, value: i32) -> R<String> {
        Ok(self.record(Op::Attribute(name.into(), value)).map_err(err)?.1.unwrap_or_default())
    }

    /// Set level ("level", 1..=60) or street cred ("streetcred", 1..=50). Returns a warning or "".
    pub fn set_progression(&mut self, key: &str, value: i32) -> R<String> {
        Ok(self.record(Op::Progression(key.into(), value)).map_err(err)?.1.unwrap_or_default())
    }

    /// Set the life path (corpo/nomad/streetkid).
    pub fn set_life_path(&mut self, name: &str) -> R<()> {
        self.record(Op::LifePath(name.into())).map_err(err).map(|_| ())
    }

    /// Set V's money.
    pub fn set_money(&mut self, value: u32) -> R<()> {
        self.record(Op::Money(value)).map_err(err).map(|_| ())
    }

    /// Set unspent points (attribute/perk/relic).
    pub fn set_points(&mut self, kind: &str, value: i32) -> R<()> {
        self.record(Op::Points(kind.into(), value)).map_err(err).map(|_| ())
    }

    /// Remove `ResetAttributeDisabled`. Returns false if the reset was already available.
    pub fn unlock_reset(&mut self) -> R<bool> {
        let (old, _) = self.record(Op::UnlockReset).map_err(err)?;
        Ok(old == "used")
    }

    /// `{ hash, value, table } | null` for a fact name or `0xHASH`.
    pub fn get_fact(&self, key: &str) -> R<String> {
        let h = parse_fact_key(key).map_err(err)?;
        Ok(match self.save.get_fact(h).map_err(err)? {
            Some((t, v)) => json!({ "hash": format!("{h:#010x}"), "value": v, "table": t }),
            None => json!({ "hash": format!("{h:#010x}"), "value": J::Null, "table": J::Null }),
        }
        .to_string())
    }

    pub fn set_fact(&mut self, key: &str, value: u32) -> R<()> {
        self.record(Op::SetFact(key.into(), value)).map_err(err).map(|_| ())
    }

    pub fn remove_fact(&mut self, key: &str) -> R<bool> {
        let (old, _) = self.record(Op::RemoveFact(key.into())).map_err(err)?;
        Ok(old != "unset")
    }

    pub fn fact_count(&self) -> R<u32> {
        Ok(self.save.facts_tables().map_err(err)?.iter().map(|t| t.facts.len() as u32).sum())
    }

    /// Names of nodes holding object packages (for the advanced editor).
    pub fn package_nodes(&self) -> String {
        let names: Vec<&str> = (0..self.save.nodes.len())
            .filter(|&i| is_package(self.save.node_bytes(i)))
            .map(|i| self.save.nodes[i].name.as_str())
            .collect();
        json!(names).to_string()
    }

    /// `[{ index, type, props }]` for every object in a package node.
    pub fn package_chunks(&mut self, node: &str) -> R<String> {
        let pn = self.package(node).map_err(err)?;
        let list: Vec<J> = pn
            .package
            .chunks
            .iter()
            .enumerate()
            .map(|(i, c)| json!({ "index": i, "type": c.ty, "props": c.data.props.len() }))
            .collect();
        Ok(json!(list).to_string())
    }

    /// Full value tree of one object.
    pub fn chunk(&mut self, node: &str, index: u32) -> R<String> {
        let pn = self.package(node).map_err(err)?;
        let c = pn.package.chunks.get(index as usize).ok_or_else(|| JsError::new("chunk index out of range"))?;
        Ok(json!({ "index": index, "type": c.ty, "value": struct_json(&c.data) }).to_string())
    }

    /// Generic edit: set the scalar at `path` (see docs/editing.md). Returns the old value text.
    pub fn set_value(&mut self, node: &str, path: &str, value: &str) -> R<String> {
        let op = Op::SetValue { node: node.into(), path: path.into(), value: value.into() };
        Ok(self.record(op).map_err(err)?.0)
    }

    /// Pending changes, oldest first: `[{ target, old, new }]`.
    pub fn changes(&self) -> String {
        let list: Vec<J> = self.changes.iter().map(|c| json!({ "target": c.target, "old": c.old, "new": c.new })).collect();
        json!(list).to_string()
    }

    /// Undo one pending change: re-parse the original save and replay every other change.
    /// The session is only replaced if the whole replay succeeds.
    pub fn revert(&mut self, index: u32) -> R<()> {
        let i = index as usize;
        if i >= self.changes.len() {
            return Err(JsError::new("no such change"));
        }
        let mut next = Session::fresh(self.original.clone()).map_err(err)?;
        for (j, c) in self.changes.iter().enumerate() {
            if j != i {
                next.record(c.op.clone()).map_err(|e| err(e.context(format!("re-applying {}", c.target))))?;
            }
        }
        *self = next;
        Ok(())
    }

    /// Serialize the edited save.
    pub fn write(&self) -> R<Vec<u8>> {
        let bytes = self.save.write().map_err(err)?;
        let back = SaveFile::parse(&bytes).map_err(err)?;
        if back.data != self.save.data {
            return Err(JsError::new("internal error: written save does not re-parse identically"));
        }
        Ok(bytes)
    }

    /// Update `metadata.9.json` text for a written save of `file_size` bytes.
    pub fn patch_metadata(&self, json_text: &str, file_size: u32) -> String {
        patch_for_save(json_text, file_size as usize, &self.meta)
    }
}

/// FNV-1a 32 hash used for fact names (exposed for the UI's fact lookup).
#[wasm_bindgen]
pub fn fact_hash(name: &str) -> String {
    format!("{:#010x}", cp2077_save::facts::fnv1a32(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Option<Vec<u8>> {
        // project-local copy of a real save (git-ignored); skip when absent
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../saves/ManualSave-17/sav.dat")).ok()
    }

    #[test]
    fn revert_one_change_equals_never_making_it() {
        let Some(orig) = sample() else { return };
        let mut s = Session::new(&orig).unwrap();
        s.set_progression("level", 50).unwrap();
        s.set_money(1).unwrap();
        s.set_attribute("cool", 15).unwrap();
        s.set_attribute("cool", 16).unwrap(); // merged into one change
        assert_eq!(s.changes.len(), 3);

        s.revert(1).unwrap(); // money
        let mut expected = Session::new(&orig).unwrap();
        expected.set_progression("level", 50).unwrap();
        expected.set_attribute("cool", 16).unwrap();
        assert!(s.save.data == expected.save.data, "revert must equal never having made the change");
        assert_eq!(s.meta, expected.meta);
        assert_eq!(s.save.money().unwrap(), Some(345579));
        let targets: Vec<&str> = s.changes.iter().map(|c| c.target.as_str()).collect();
        assert_eq!(targets, ["Level", "Cool"]);
        assert_eq!(s.changes[1].old, "20");

        s.revert(1).unwrap();
        s.revert(0).unwrap();
        assert!(s.changes.is_empty());
        let fresh = SaveFile::parse(&orig).unwrap();
        assert!(s.save.data == fresh.data && s.save.nodes.iter().zip(&fresh.nodes).all(|(a, b)| a.offset == b.offset && a.size == b.size));
    }

    #[test]
    fn editing_back_to_original_drops_the_change() {
        let Some(orig) = sample() else { return };
        let mut s = Session::new(&orig).unwrap();
        s.set_points("perk", 9).unwrap();
        s.set_points("perk", 4).unwrap();
        assert!(s.changes.is_empty());
        assert!(!s.unlock_reset().unwrap()); // already available: nothing to list
        assert!(s.changes.is_empty());
    }
}
