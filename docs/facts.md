# Quest facts

Node path: `questSystem > FactsDB > FactsTable` (10 tables on 2.5).

```
FactsDB:    u32 nodeId, vlq tableCount (game reads at most 10), then FactsTable children
FactsTable: u32 nodeId, vlq count, u32 hash[count] (sorted ascending), u32 value[count]
```

* Fact names are hashed with **FNV-1a 32** (`offset 0x811c9dc5`, `prime 0x01000193`).
  Check: `player_drunk` → `0x025d58c5` appears in the table.
* An absent fact reads as 0.
* Readable names can be recovered by hashing every printable string in
  `r6/cache/final.redscripts` (the compiled game scripts) and matching hashes.
* Game code: `FactsDB::Load` at RVA `0x7399bc`, `FactsTable::Serialize` at `0x739a68`
  (see [game-binary.md](game-binary.md)).

## `ResetAttributeDisabled` (the once-per-playthrough attribute reset)

| | ManualSave-17 (before reset) | ManualSave-18 (after reset) |
|---|---|---|
| fact `ResetAttributeDisabled` (`0x2b0475c8`) | absent | `1` (FactsTable #1) |

Found by diffing every node of the two saves: the only fact that changed. Pressing
**Reset Attributes** sets this fact, and the character screen hides the button while it is set.

**Removing the fact restores the button.** Tested in-game on 2026-11-10: the edited
ManualSave-18 loads, the attributes stay reset, and the button is available again.

```
cp2077-save unlock-reset <in_save_dir> <out_save_dir>
```

Other edits: `set-fact`, `remove-fact`, `get-fact`, `facts [-v]`.
