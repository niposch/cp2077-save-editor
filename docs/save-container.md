# `sav.dat` container format

Verified on game 2.5 (`saveVersion` 277, `gameVersion` 2500, build `3.0.5813403 P4CL 13555055`)
and on an older 1.6-era save (`saveVersion` 224, `gameVersion` 1600).
Confirmed both by parsing real saves and by disassembling the game's reader (see [game-binary.md](game-binary.md)).

A save folder contains `sav.dat`, `metadata.9.json` (UI summary, includes `fileSize`) and `screenshot.png`.

## Layout

```
offset 0x00  "VASC"                      magic (u32 0x43534156)
             u32  saveVersion            277 on 2.5
             u32  gameVersion            2500 on 2.5
             str  gameDef                usually empty
             u64  timestamp              (only if saveVersion >= 5)
             u32  archiveVersion         195 (game rejects > 0xC3)
             "FZLC"                      chunk table magic
             u32  chunkCount
             N x { u32 offset, u32 compressedSize, u32 decompressedSize }
                                         N = table capacity: 512 on 2.5, 256 on 1.6
data_start   chunks, back to back:
               "4ZLX" u32 decompressedSize <raw LZ4 block>
               (compressedSize includes this 8-byte header; 256 KiB decompressed each)
             "EDON"                      node table magic ("NODE" reversed)
             vlq  nodeCount
             nodeCount x { str name, i32 nextSibling, i32 firstChild, i32 offset, i32 size }
             u32  nodeTableOffset
             "ENOD"                      footer ("DONE" reversed)
```

`data_start` = header size + 12·capacity (0x1821 on 2.5, 0xC21 on 1.6).

## Primitive encodings

* **vlq** (CR2W packed int): first byte `[sign:1][continue:1][value:6]`, then
  `[continue:1][value:7]` per byte.
* **str**: vlq length. Negative = that many 1-byte chars; positive = UTF-16 code units.

## Node tree

* Node `offset`s are **virtual**: they address the decompressed stream as if it began at
  `data_start`. Subtract `data_start` to index the decompressed buffer.
* Nodes nest; a parent's `[offset, offset+size)` range contains its children.
* Every node's data begins with its own `u32` node index, written by the engine's `BeginNode`.
  Node-specific data follows. Children come wherever the system opens them.
* Node names are CNames in the engine (FNV-1a 64 hashes); the table stores the plain string.
* 2.5 saves have ~2080 nodes. Large ones: `PersistencySystem2` (~10 MB, world state),
  `CommunitySystem`, `StatsSystem`, `ScriptableSystemsContainer`, `inventory`, `questSystem`.

## Resizing a node

Changing a node's length requires:
1. shift `offset` of every node starting after it,
2. grow/shrink `size` of every ancestor (nodes whose range contains it).

No checksums or other size fields were found. A save with a resized `FactsTable` loaded fine in-game.
Implemented in `SaveFile::replace_node` ([src/lib.rs](../src/lib.rs)).

## Writing

Recompress the stream in 256 KiB chunks with any LZ4 block compressor (we use `lz4_flex`),
keep the header size identical (so node offsets stay valid), rebuild the chunk table,
append the node table and footer, then update `fileSize` in `metadata.9.json`.
Our output is ~2% smaller than the game's and loads fine.

## Version gates in the game's reader

From `ReadVersionInfo` (RVA `0xfa55f0`):

| check | result |
|---|---|
| `saveVersion > 277` or `gameVersion > 2500` | rejected (error 5: save from newer game) |
| `saveVersion < 125` | rejected |
| `saveVersion` 167–168 with `gameVersion == 4` | rejected |
| required DLC/content missing | error 6 |
| `archiveVersion > 195` | rejected |
