//! Player-level edits that keep every copy of a value in sync.
//!
//! Attributes live in two places (see docs/player-development.md):
//! 1. `ScriptableSystemsContainer` → `PlayerDevelopmentData.attributes[].value` (Int32)
//! 2. `StatsSystem` → `sharedDataBuffer`: a `gameConstantStatModifier` record per attribute:
//!    `str "gameConstantStatModifier", u64 fnv1a64(statName), u32 modifierType, f32 value`
//!
//! The UI summary in `metadata.9.json` (strength/intelligence/...) is updated by the caller.

use crate::edit::{scalar_str, SetResult};
use crate::package::{Package, PackageNode, Value};
use crate::SaveFile;
use anyhow::{bail, ensure, Context, Result};
use std::collections::HashMap;

pub const SSC: &str = "ScriptableSystemsContainer";
/// The player's PlayerDevelopmentData (older saves also hold NPC copies). Entity ID 1 = V.
const PDD: &str = "PlayerDevelopmentData[ownerID.hash=1]";

/// (accepted names, gamedataStatType, metadata.9.json key)
pub const ATTRIBUTES: &[(&[&str], &str, &str)] = &[
    (&["body", "strength", "str"], "Strength", "strength"),
    (&["reflexes", "ref"], "Reflexes", "reflexes"),
    (&["tech", "technicalability", "technical"], "TechnicalAbility", "technicalAbility"),
    (&["intelligence", "int"], "Intelligence", "intelligence"),
    (&["cool"], "Cool", "cool"),
];

/// (accepted names, gamedataDevelopmentPointType or "" for the default/omitted value, description)
pub const POINT_KINDS: &[(&[&str], &str, &str)] = &[
    (&["attribute", "attr"], "", "attribute points"),
    (&["perk", "primary"], "Primary", "perk points"),
    (&["relic", "espionage"], "Espionage", "relic points"),
    (&["secondary"], "Secondary", "secondary points (legacy)"),
];

pub fn resolve_attribute(name: &str) -> Result<(&'static str, &'static str)> {
    let n = name.to_ascii_lowercase();
    ATTRIBUTES
        .iter()
        .find(|(names, ..)| names.contains(&n.as_str()))
        .map(|(_, stat, meta)| (*stat, *meta))
        .with_context(|| format!("unknown attribute {name:?} (body, reflexes, tech, intelligence, cool)"))
}

pub fn resolve_point_kind(name: &str) -> Result<&'static str> {
    let n = name.to_ascii_lowercase();
    POINT_KINDS
        .iter()
        .find(|(names, ..)| names.contains(&n.as_str()))
        .map(|(_, ty, _)| *ty)
        .with_context(|| format!("unknown point kind {name:?} (attribute, perk, relic)"))
}

pub fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

const CONST_MOD_TAG: &[u8] = b"\x98gameConstantStatModifier";

fn int_of(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::I32(x) => Some(*x as i64),
        other => scalar_str(other)?.parse().ok(),
    }
}

struct ModifierIndex {
    node: usize,
    by_hash: HashMap<u64, Vec<usize>>,
}

/// (offset of the f32 within the StatsSystem node, value) of the player's modifier for `stat`.
fn lookup_modifier(index: &ModifierIndex, b: &[u8], stat: &str) -> Result<(usize, f32)> {
    let hits = index.by_hash.get(&fnv1a64(stat)).map_or(&[][..], |v| v.as_slice());
    ensure!(
        hits.len() == 1,
        "expected exactly one {stat} stat modifier in StatsSystem, found {} (only supported for 2.x saves)",
        hits.len()
    );
    let off = hits[0];
    Ok((off, f32::from_le_bytes(b[off..off + 4].try_into()?)))
}

#[derive(Debug)]
pub struct AttributeEdit {
    /// Previous value (None = field was absent/default).
    pub old: Option<i64>,
    /// Set when the two copies of the attribute disagreed before the edit.
    pub warning: Option<String>,
}

#[derive(Debug, Default)]
pub struct PlayerSummary {
    pub level: Option<i64>,
    pub street_cred: Option<i64>,
    pub attributes: Vec<(String, Option<i64>, Option<f32>)>,
    pub points: Vec<(String, Option<i64>, Option<i64>)>,
    pub life_path: Option<String>,
}

/// (accepted names, proficiency type / stat name, max value, metadata.9.json key)
pub const PROGRESSION: &[(&[&str], &str, i32, &str)] = &[
    (&["level", "lvl"], "Level", 60, "level"),
    (&["streetcred", "street-cred", "street_cred", "sc"], "StreetCred", 50, "streetCred"),
];

pub fn resolve_progression(name: &str) -> Result<(&'static str, i32, &'static str)> {
    let n = name.to_ascii_lowercase();
    PROGRESSION
        .iter()
        .find(|(names, ..)| names.contains(&n.as_str()))
        .map(|(_, ty, max, meta)| (*ty, *max, *meta))
        .with_context(|| format!("unknown progression {name:?} (level, streetcred)"))
}

/// Life paths: (gamedataLifePath value, quest fact set by the prologue).
/// `Corporate` is the enum's default, so a Corpo save simply has no `lifePath` field.
pub const LIFE_PATHS: &[(&str, &str)] = &[
    ("Corporate", "q000_corpo_background"),
    ("Nomad", "q000_nomad_background"),
    ("StreetKid", "q000_street_kid_background"),
];

pub fn resolve_life_path(name: &str) -> Result<&'static str> {
    let n = name.to_ascii_lowercase().replace(['-', '_', ' '], "");
    Ok(match n.as_str() {
        "corporate" | "corpo" => "Corporate",
        "nomad" => "Nomad",
        "streetkid" | "street" => "StreetKid",
        _ => bail!("unknown life path {name:?} (corpo, nomad, streetkid)"),
    })
}

fn life_path_of(pkg: &Package) -> String {
    match pkg.get_path(&format!("{PDD}.lifePath")) {
        Ok(Some(Value::Enum(v))) => v.clone(),
        _ => "Corporate".to_string(),
    }
}

#[derive(Debug)]
pub struct ProgressionEdit {
    pub old: Option<i64>,
    pub warning: Option<String>,
}

impl SaveFile {
    pub fn package(&self, node: &str) -> Result<PackageNode> {
        let i = self.find_node(node).with_context(|| format!("no {node} node"))?;
        PackageNode::parse(self.node_bytes(i)).with_context(|| format!("parsing {node}"))
    }

    /// Parse a package node, let `f` edit it, and write it back (resizing the node as needed).
    pub fn edit_package<R>(&mut self, node: &str, f: impl FnOnce(&mut Package) -> Result<R>) -> Result<R> {
        let i = self.find_node(node).with_context(|| format!("no {node} node"))?;
        let mut pn = PackageNode::parse(self.node_bytes(i)).with_context(|| format!("parsing {node}"))?;
        let r = f(&mut pn.package)?;
        let bytes = pn.write()?;
        // sanity: what we wrote must parse back to the same tree
        let back = PackageNode::parse(&bytes).context("re-parsing edited package")?;
        ensure!(back.package.chunks == pn.package.chunks, "edited {node} does not re-parse identically");
        self.replace_node(i, bytes)?;
        Ok(r)
    }

    /// Locate the player's constant stat modifier for `stat` in StatsSystem:
    /// (offset of the f32 value within the node, value).
    fn stat_modifier(&self, stat: &str) -> Result<(usize, f32)> {
        let index = self.const_modifier_index()?;
        lookup_modifier(&index, self.node_bytes(index.node), stat)
    }

    /// One pass over StatsSystem: offsets of every `gameConstantStatModifier` value, by stat hash.
    fn const_modifier_index(&self) -> Result<ModifierIndex> {
        let node = self.find_node("StatsSystem").context("no StatsSystem node")?;
        let b = self.node_bytes(node);
        let mut by_hash: HashMap<u64, Vec<usize>> = HashMap::new();
        let mut pos = 0;
        while let Some(off) = b[pos..].iter().position(|&x| x == CONST_MOD_TAG[0]) {
            let at = pos + off;
            pos = at + 1;
            if b[at..].starts_with(CONST_MOD_TAG) && at + CONST_MOD_TAG.len() + 16 <= b.len() {
                let h = u64::from_le_bytes(b[at + CONST_MOD_TAG.len()..at + CONST_MOD_TAG.len() + 8].try_into()?);
                // value follows the u64 stat hash and the u32 modifier type
                by_hash.entry(h).or_default().push(at + CONST_MOD_TAG.len() + 12);
            }
        }
        Ok(ModifierIndex { node, by_hash })
    }

    pub fn player_summary(&self) -> Result<PlayerSummary> {
        self.player_summary_with(&self.package(SSC)?.package)
    }

    /// Player summary from an already parsed ScriptableSystemsContainer package.
    pub fn player_summary_with(&self, pkg: &Package) -> Result<PlayerSummary> {
        let get = |p: &str| pkg.get_path(&format!("{PDD}.{p}")).ok().flatten();
        let mut s = PlayerSummary {
            level: int_of(get("proficiencies[type=Level].currentLevel")),
            street_cred: int_of(get("proficiencies[type=StreetCred].currentLevel")),
            life_path: Some(life_path_of(pkg)),
            ..Default::default()
        };
        let index = self.const_modifier_index().ok();
        for (_, stat, _) in ATTRIBUTES {
            let v = int_of(get(&format!("attributes[attributeName={stat}].value")));
            let m = index.as_ref().and_then(|ix| lookup_modifier(ix, self.node_bytes(ix.node), stat).ok()).map(|x| x.1);
            s.attributes.push((stat.to_string(), v, m));
        }
        for (_, ty, desc) in POINT_KINDS {
            let base = format!("devPoints[type={ty}]");
            if pkg.get_path(&format!("{PDD}.{base}")).is_err() {
                continue;
            }
            s.points.push((desc.to_string(), int_of(get(&format!("{base}.spent"))), int_of(get(&format!("{base}.unspent")))));
        }
        Ok(s)
    }

    /// Set an attribute in PlayerDevelopmentData and the matching StatsSystem modifier.
    pub fn set_attribute(&mut self, stat: &str, value: i32) -> Result<AttributeEdit> {
        ensure!((3..=20).contains(&value), "attribute values must be 3..=20 (the game's own range)");
        let (off, modval) = self.stat_modifier(stat)?;
        let old = self.edit_package(SSC, |p| {
            let path = format!("{PDD}.attributes[attributeName={stat}].value");
            Ok(match p.set_path(&path, &value.to_string())? {
                SetResult::Changed { old } => int_of(Some(&old)),
                SetResult::Inserted => None,
            })
        })?;
        let warning = old.filter(|o| (*o as f32 - modval).abs() > 0.01).map(|o| {
            format!("{stat} was {o} in PlayerDevelopmentData but {modval} in StatsSystem; both now {value}")
        });
        let i = self.find_node("StatsSystem").unwrap();
        self.node_bytes_mut(i)[off..off + 4].copy_from_slice(&(value as f32).to_le_bytes());
        Ok(AttributeEdit { old, warning })
    }

    /// Set character level ("Level", 1..=60) or street cred ("StreetCred", 1..=50).
    ///
    /// Updates every copy found in 2.5 saves: the proficiency in PlayerDevelopmentData
    /// (XP within the level is reset to 0), the player's constant stat modifier in StatsSystem,
    /// and for the level also `LevelAssignmentSystem_Ver1` (a float used for level scaling).
    pub fn set_progression(&mut self, ty: &str, value: i32) -> Result<ProgressionEdit> {
        let max = PROGRESSION.iter().find(|p| p.1 == ty).map(|p| p.2).context("unknown progression")?;
        ensure!((1..=max).contains(&value), "{ty} must be 1..={max} (the game's own range)");
        let (off, modval) = self.stat_modifier(ty)?;
        let las = if ty == "Level" { Some(self.level_assignment()?) } else { None };
        let old = self.edit_package(SSC, |p| {
            let base = format!("{PDD}.proficiencies[type={ty}]");
            let old = match p.set_path(&format!("{base}.currentLevel"), &value.to_string())? {
                SetResult::Changed { old } => int_of(Some(&old)),
                SetResult::Inserted => None,
            };
            // XP is tracked within the current level; keep it from overflowing the new level.
            if p.get_path(&format!("{base}.currentExp"))?.is_some() {
                p.set_path(&format!("{base}.currentExp"), "0")?;
            }
            Ok(old)
        })?;
        let warning = old.filter(|o| (*o as f32 - modval).abs() > 0.01).map(|o| {
            format!("{ty} was {o} in PlayerDevelopmentData but {modval} in StatsSystem; both now {value}")
        });
        let si = self.find_node("StatsSystem").unwrap();
        self.node_bytes_mut(si)[off..off + 4].copy_from_slice(&(value as f32).to_le_bytes());
        if let Some((ni, _)) = las {
            self.node_bytes_mut(ni)[4..8].copy_from_slice(&(value as f32).to_le_bytes());
        }
        Ok(ProgressionEdit { old, warning })
    }

    /// `LevelAssignmentSystem_Ver1`: `u32 nodeId, f32 playerLevel, u8`. Returns (node index, level).
    fn level_assignment(&self) -> Result<(usize, f32)> {
        let i = self.find_node("LevelAssignmentSystem_Ver1").context("no LevelAssignmentSystem_Ver1 node")?;
        let b = self.node_bytes(i);
        ensure!(b.len() == 9, "unexpected LevelAssignmentSystem_Ver1 size {}", b.len());
        Ok((i, f32::from_le_bytes(b[4..8].try_into()?)))
    }

    pub fn life_path(&self) -> Result<String> {
        Ok(life_path_of(&self.package(SSC)?.package))
    }

    /// Change the life path: `PlayerDevelopmentData.lifePath` plus the prologue's background fact.
    /// Returns the previous life path.
    pub fn set_life_path(&mut self, life_path: &str) -> Result<String> {
        let (lp, fact) = LIFE_PATHS.iter().find(|l| l.0 == life_path).copied().context("unknown life path")?;
        let old = self.life_path()?;
        self.edit_package(SSC, |p| {
            if lp == "Corporate" {
                p.remove_path(&format!("{PDD}.lifePath"))?;
            } else {
                p.set_path(&format!("{PDD}.lifePath:gamedataLifePath"), lp)?;
            }
            Ok(())
        })?;
        for (_, f) in LIFE_PATHS {
            self.remove_fact(crate::facts::fnv1a32(f))?;
        }
        self.set_fact(crate::facts::fnv1a32(fact), 1)?;
        Ok(old)
    }

    /// Set unspent points of a kind ("" = attribute points). Returns the previous value.
    pub fn set_unspent_points(&mut self, kind_ty: &str, value: i32) -> Result<Option<i64>> {
        if value < 0 {
            bail!("point counts can't be negative");
        }
        self.edit_package(SSC, |p| {
            let path = format!("{PDD}.devPoints[type={kind_ty}].unspent");
            Ok(match p.set_path(&path, &value.to_string())? {
                SetResult::Changed { old } => int_of(Some(&old)),
                SetResult::Inserted => None,
            })
        })
    }
}
