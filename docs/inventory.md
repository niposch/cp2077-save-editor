# Inventory and money

Node: `inventory` (top level), with one `itemData` child node per item.

```
inventory: u32 nodeId, u32 inventoryCount, then per inventory:
  u64 ownerEntityId
  u32 itemCount
  itemCount x {
    ItemID (16 bytes, stored in the parent before each child)
    itemData child node
  }

ItemID:  u64 TweakDBID, u32 rngSeed, u16 uniqueCounter, u8 structure, u8 pad
itemData (simple item, 29 bytes):
  u32 nodeId, ItemID (repeated), u8 flags, u32 creationTime, u32 quantity
```

Items with mods/stats have larger `itemData` nodes (`structure != 0`); those aren't decoded yet.
The ItemID header in the parent and the copy inside the child always agree. The parser checks
this, and that every child sits exactly where the headers say, with nothing left over.

| owner entity | what |
|---|---|
| `1` | V |
| `1000000` | stash |
| others | vehicles, containers, NPCs, ... |

On 2.5 save 17: 12 inventories, V has 642 items, the stash 1162.

## Money

Money is the simple item `Items.money` (TweakDBID = `crc32("Items.money") | 11 << 32`) in V's
inventory, and its `quantity` is the eddies amount. The amount occurs only once in the whole save
(no stat or cache copy), so editing it is a same-size in-place change.

* `cp2077-save edit <in> <out> --money <n>` (max 999,999,999).
* If V has no money item (seen in a 1.6 save mid-quest, where V's gear sat in the stash), the
  edit is refused instead of inventing an item.
