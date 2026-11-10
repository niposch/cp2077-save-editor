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

    /// Locate the attribute's constant stat modifier in StatsSystem: (offset of f32 within node, value).
    fn attribute_modifier(&self, stat: &str) -> Result<(usize, f32)> {
        let i = self.find_node("StatsSystem").context("no StatsSystem node")?;
        let b = self.node_bytes(i);
        let mut pat = CONST_MOD_TAG.to_vec();
        pat.extend_from_slice(&fnv1a64(stat).to_le_bytes());
        let hits: Vec<usize> = b.windows(pat.len()).enumerate().filter(|(_, w)| *w == pat.as_slice()).map(|(i, _)| i).collect();
        ensure!(
            hits.len() == 1,
            "expected exactly one {stat} stat modifier in StatsSystem, found {} (attribute edits are only supported for 2.x saves)",
            hits.len()
        );
        let off = hits[0] + pat.len() + 4; // skip u32 modifier type
        let v = f32::from_le_bytes(b[off..off + 4].try_into()?);
        Ok((off, v))
    }

    pub fn player_summary(&self) -> Result<PlayerSummary> {
        let pkg = self.package(SSC)?.package;
        let get = |p: &str| pkg.get_path(&format!("{PDD}.{p}")).ok().flatten();
        let mut s = PlayerSummary {
            level: int_of(get("proficiencies[type=Level].currentLevel")),
            street_cred: int_of(get("proficiencies[type=StreetCred].currentLevel")),
            ..Default::default()
        };
        for (_, stat, _) in ATTRIBUTES {
            let v = int_of(get(&format!("attributes[attributeName={stat}].value")));
            let m = self.attribute_modifier(stat).ok().map(|x| x.1);
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
        let (off, modval) = self.attribute_modifier(stat)?;
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
