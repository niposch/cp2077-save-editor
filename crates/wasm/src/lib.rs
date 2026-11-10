//! WebAssembly bindings used by the React UI (web and desktop).
//!
//! Everything crosses the boundary as JSON strings or byte arrays to keep the interface small.
//! A `Session` holds one open save; edits are applied in memory and `write()` produces the new
//! `sav.dat`.

use cp2077_save::{
    edit::{scalar_str, SetResult},
    facts::parse_fact_key,
    metadata::patch_for_save,
    package::{array_elem, PackageNode, Struct, Value},
    player::{resolve_attribute, resolve_point_kind, ATTRIBUTES, POINT_KINDS},
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

#[wasm_bindgen]
pub struct Session {
    save: SaveFile,
    /// Extra numeric metadata.9.json fields to update on write (attribute values).
    meta: Vec<(String, String)>,
    /// Applied edits as (target, first old value, latest new value); repeated edits of the same
    /// target are merged, and edits that end at the original value are dropped.
    changes: Vec<(String, String, String)>,
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

    fn edited(&mut self, target: impl Into<String>, old: impl Into<String>, new: impl Into<String>) {
        self.cache.clear();
        let (target, old, new) = (target.into(), old.into(), new.into());
        match self.changes.iter().position(|c| c.0 == target) {
            Some(i) => {
                if self.changes[i].1 == new {
                    self.changes.remove(i);
                } else {
                    self.changes[i].2 = new;
                }
            }
            None => self.changes.push((target, old, new)),
        }
    }

    fn set_meta(&mut self, key: &str, value: String) {
        self.meta.retain(|(k, _)| k != key);
        self.meta.push((key.to_string(), value));
    }
}

#[wasm_bindgen]
impl Session {
    /// Parse a `sav.dat`.
    #[wasm_bindgen(constructor)]
    pub fn new(sav: &[u8]) -> R<Session> {
        let save = SaveFile::parse(sav).map_err(err)?;
        Ok(Session { save, meta: vec![], changes: vec![], cache: HashMap::new() })
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
        })
        .to_string())
    }

    /// Set an attribute (body/reflexes/tech/intelligence/cool). Returns a warning or "".
    pub fn set_attribute(&mut self, name: &str, value: i32) -> R<String> {
        let (stat, meta_key) = resolve_attribute(name).map_err(err)?;
        let r = self.save.set_attribute(stat, value).map_err(err)?;
        self.set_meta(meta_key, format!("{value}.0"));
        self.edited(stat, r.old.map_or("default".into(), |o| o.to_string()), value.to_string());
        Ok(r.warning.unwrap_or_default())
    }

    /// Set unspent points (attribute/perk/relic).
    pub fn set_points(&mut self, kind: &str, value: i32) -> R<()> {
        let ty = resolve_point_kind(kind).map_err(err)?;
        let old = self.save.set_unspent_points(ty, value).map_err(err)?;
        self.edited(format!("{kind} points"), old.unwrap_or(0).to_string(), value.to_string());
        Ok(())
    }

    /// Remove `ResetAttributeDisabled`. Returns false if the reset was already available.
    pub fn unlock_reset(&mut self) -> R<bool> {
        let removed = self.save.remove_fact(parse_fact_key("ResetAttributeDisabled").map_err(err)?).map_err(err)?;
        if removed {
            self.edited("Attribute reset", "used", "available");
        }
        Ok(removed)
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
        let old = self.save.get_fact(parse_fact_key(key).map_err(err)?).map_err(err)?.map_or(0, |f| f.1);
        self.save.set_fact(parse_fact_key(key).map_err(err)?, value).map_err(err)?;
        self.edited(format!("fact {key}"), old.to_string(), value.to_string());
        Ok(())
    }

    pub fn remove_fact(&mut self, key: &str) -> R<bool> {
        let removed = self.save.remove_fact(parse_fact_key(key).map_err(err)?).map_err(err)?;
        if removed {
            self.edited(format!("fact {key}"), "set", "removed");
        }
        Ok(removed)
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
        let r = self.save.edit_package(node, |p| p.set_path(path, value)).map_err(err)?;
        let old = match r {
            SetResult::Changed { old } => scalar_str(&old).unwrap_or_default(),
            SetResult::Inserted => "(default)".into(),
        };
        self.edited(format!("{node}:{path}"), old.clone(), value);
        Ok(old)
    }

    /// Edits applied so far, oldest first (JSON string array).
    pub fn changes(&self) -> String {
        let list: Vec<String> = self.changes.iter().map(|(t, o, n)| format!("{t}: {o} → {n}")).collect();
        json!(list).to_string()
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
