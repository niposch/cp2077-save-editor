//! Quest facts (`questSystem/FactsDB/FactsTable`).
//!
//! FactsTable node layout:
//! ```text
//! u32 nodeId  vlq count  u32 hashes[count] (sorted asc)  u32 values[count]
//! ```
//! Fact names are hashed with 32-bit FNV-1a.

use crate::{read_vlq, write_vlq, SaveFile};
use anyhow::{ensure, Result};

pub fn fnv1a32(s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

/// A fact name argument: either a name or a raw `0x...` hash.
pub fn parse_fact_key(s: &str) -> Result<u32> {
    Ok(match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16)?,
        None => fnv1a32(s),
    })
}

#[derive(Debug, Clone)]
pub struct FactsTable {
    pub node_idx: usize,
    pub node_id: u32,
    /// (hash, value), kept sorted by hash.
    pub facts: Vec<(u32, u32)>,
}

impl FactsTable {
    pub fn parse(node_idx: usize, b: &[u8]) -> Result<Self> {
        let node_id = u32::from_le_bytes(b[0..4].try_into()?);
        let mut p = 4;
        let n = read_vlq(b, &mut p)? as usize;
        ensure!(p + n * 8 == b.len(), "FactsTable size mismatch");
        let rd = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        let facts = (0..n).map(|i| (rd(p + i * 4), rd(p + (n + i) * 4))).collect();
        Ok(FactsTable { node_idx, node_id, facts })
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = self.node_id.to_le_bytes().to_vec();
        write_vlq(&mut out, self.facts.len() as i64);
        for (h, _) in &self.facts {
            out.extend_from_slice(&h.to_le_bytes());
        }
        for (_, v) in &self.facts {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    pub fn get(&self, h: u32) -> Option<u32> {
        self.facts.binary_search_by_key(&h, |f| f.0).ok().map(|i| self.facts[i].1)
    }

    pub fn set(&mut self, h: u32, v: u32) {
        match self.facts.binary_search_by_key(&h, |f| f.0) {
            Ok(i) => self.facts[i].1 = v,
            Err(i) => self.facts.insert(i, (h, v)),
        }
    }

    pub fn remove(&mut self, h: u32) -> Option<u32> {
        let i = self.facts.binary_search_by_key(&h, |f| f.0).ok()?;
        Some(self.facts.remove(i).1)
    }
}

impl SaveFile {
    pub fn facts_tables(&self) -> Result<Vec<FactsTable>> {
        (0..self.nodes.len())
            .filter(|&i| self.nodes[i].name == "FactsTable")
            .map(|i| FactsTable::parse(i, self.node_bytes(i)))
            .collect()
    }

    pub fn store_facts_table(&mut self, t: &FactsTable) -> Result<()> {
        self.replace_node(t.node_idx, t.serialize())
    }

    /// Look a fact up across all tables: returns (table ordinal, value).
    pub fn get_fact(&self, h: u32) -> Result<Option<(usize, u32)>> {
        Ok(self.facts_tables()?.iter().enumerate().find_map(|(i, t)| t.get(h).map(|v| (i, v))))
    }

    /// Set a fact. Updates it in place if present, otherwise inserts into the first table.
    pub fn set_fact(&mut self, h: u32, v: u32) -> Result<()> {
        let mut tables = self.facts_tables()?;
        let t = match tables.iter().position(|t| t.get(h).is_some()) {
            Some(i) => &mut tables[i],
            None => tables.first_mut().ok_or_else(|| anyhow::anyhow!("no FactsTable"))?,
        };
        t.set(h, v);
        let t = t.clone();
        self.store_facts_table(&t)
    }

    /// Remove a fact from every table. Returns true if anything was removed.
    pub fn remove_fact(&mut self, h: u32) -> Result<bool> {
        let mut removed = false;
        for mut t in self.facts_tables()? {
            if t.remove(h).is_some() {
                self.store_facts_table(&t)?;
                removed = true;
            }
        }
        Ok(removed)
    }
}
