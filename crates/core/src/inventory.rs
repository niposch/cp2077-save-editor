//! Inventories (`inventory` node) and money.
//!
//! ```text
//! inventory node: u32 nodeId, u32 inventoryCount, then per inventory:
//!   u64 ownerEntityId, u32 itemCount,
//!   itemCount x { ItemID (16 bytes), itemData child node }
//! ItemID: u64 TweakDBID, u32 rngSeed, u16 uniqueCounter, u8 structure, u8 pad
//! itemData (simple item, 29 bytes): u32 nodeId, ItemID, u8 flags, u32 creationTime, u32 quantity
//! ```
//! Entity 1 is V; entity 1000000 is the stash. Money is the item `Items.money` in V's inventory.

use crate::SaveFile;
use anyhow::{bail, ensure, Context, Result};

pub const PLAYER_ENTITY: u64 = 1;
pub const STASH_ENTITY: u64 = 1_000_000;
/// Upper bound we allow for money (the game keeps it in a 32-bit integer).
pub const MONEY_MAX: u32 = 999_999_999;

pub fn tweakdb_id(path: &str) -> u64 {
    crc32fast::hash(path.as_bytes()) as u64 | ((path.len() as u64) << 32)
}

#[derive(Debug, Clone)]
pub struct ItemRef {
    /// Index of the `itemData` node.
    pub node: usize,
    pub tdbid: u64,
    pub seed: u32,
}

#[derive(Debug, Clone)]
pub struct Inventory {
    pub entity: u64,
    pub items: Vec<ItemRef>,
}

fn u32_at(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(b.get(p..p + 4).context("eof")?.try_into()?))
}
fn u64_at(b: &[u8], p: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(b.get(p..p + 8).context("eof")?.try_into()?))
}

impl SaveFile {
    /// Parse the inventory list, validating that item headers line up with the child nodes.
    pub fn inventories(&self) -> Result<Vec<Inventory>> {
        let ni = self.find_node("inventory").context("no inventory node")?;
        let node = &self.nodes[ni];
        let base = node.offset as usize; // virtual offsets
        let end = base + node.size as usize;
        let at = |v: usize| v - self.data_start;
        let d = &self.data;
        let count = u32_at(d, at(base) + 4)? as usize;
        let children = self.children(ni);
        let mut ci = 0;
        let mut pos = base + 8;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let entity = u64_at(d, at(pos))?;
            let n = u32_at(d, at(pos) + 8)? as usize;
            pos += 12;
            let mut items = Vec::with_capacity(n);
            for _ in 0..n {
                let child = *children.get(ci).context("fewer itemData nodes than the inventory header says")?;
                ci += 1;
                let c = &self.nodes[child];
                ensure!(c.offset as usize == pos + 16, "itemData node {child} is not where the inventory header puts it");
                let tdbid = u64_at(d, at(pos))?;
                let seed = u32_at(d, at(pos) + 8)?;
                let nb = self.node_bytes(child);
                ensure!(u64_at(nb, 4)? == tdbid, "itemData node {child} disagrees with its ItemID header");
                items.push(ItemRef { node: child, tdbid, seed });
                pos = c.offset as usize + c.size as usize;
            }
            out.push(Inventory { entity, items });
        }
        ensure!(ci == children.len() && pos == end, "inventory node has unparsed data");
        Ok(out)
    }

    /// The money item in V's inventory: (itemData node index, offset of the quantity within it).
    fn money_item(&self) -> Result<Option<(usize, usize)>> {
        let money = tweakdb_id("Items.money");
        let invs = self.inventories()?;
        let player = invs.iter().find(|i| i.entity == PLAYER_ENTITY).context("no player inventory")?;
        let hits: Vec<&ItemRef> = player.items.iter().filter(|i| i.tdbid == money).collect();
        match hits.as_slice() {
            [] => Ok(None),
            [it] => {
                let b = self.node_bytes(it.node);
                // simple item: u32 id, ItemID(16), u8 flags, u32 time, u32 quantity
                ensure!(b.len() == 29 && b[18] == 0, "unexpected money item layout ({} bytes)", b.len());
                Ok(Some((it.node, 25)))
            }
            _ => bail!("V has {} separate money items; refusing to guess", hits.len()),
        }
    }

    /// V's money (None if the inventory has no money item at all).
    pub fn money(&self) -> Result<Option<u32>> {
        Ok(match self.money_item()? {
            Some((n, off)) => Some(u32_at(self.node_bytes(n), off)?),
            None => None,
        })
    }

    /// Set V's money. Returns the previous amount.
    pub fn set_money(&mut self, value: u32) -> Result<u32> {
        ensure!(value <= MONEY_MAX, "money must be at most {MONEY_MAX}");
        let (n, off) = self
            .money_item()?
            .context("this save has no money item yet (pick up or earn some eddies in-game first)")?;
        let old = u32_at(self.node_bytes(n), off)?;
        self.node_bytes_mut(n)[off..off + 4].copy_from_slice(&value.to_le_bytes());
        Ok(old)
    }
}
