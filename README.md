# Cyberpunk 2077 Save Editor (Rust)

A save reader/writer for Cyberpunk 2077, built by reverse engineering game 2.5 saves
(`saveVersion` 277) and the game binary. Also tested on a 1.6-era save.

**Status**
* Container (decompress / recompress / node tree / node resizing): done, round-trips exactly.
* Quest facts: read/write. **Restoring the once-per-playthrough attribute reset works in-game.**
* Embedded object packages (ScriptableSystemsContainer, StatsSystem, ...): generic reader +
  writer, byte-exact round trip on all test saves. No editing commands on top yet.
* Not decoded yet: `inventory`/`itemData`, `PersistencySystem2`, `DataBuffer` contents, and more.

## Usage

```
cargo build --release
target/release/cp2077-save <command> ...
```

| command | |
|---|---|
| `info <sav.dat>` | header + sizes |
| `nodes <sav.dat>` | node tree |
| `node <sav.dat> <NodeName> <out.bin>` | extract one node's raw bytes |
| `dump <sav.dat> <out.bin>` | whole decompressed stream |
| `diff <a/sav.dat> <b/sav.dat>` | which leaf nodes differ |
| `roundtrip <sav.dat> [out]` | decompress + recompress check |
| `facts <sav.dat> [-v]` / `get-fact <sav.dat> <name\|0xHASH>` | quest facts |
| `set-fact <in_dir> <out_dir> <name\|0xHASH> <value>` | write a fact |
| `remove-fact <in_dir> <out_dir> <name\|0xHASH>` | delete a fact |
| `unlock-reset <in_dir> <out_dir>` | restore the "Reset Attributes" button |
| `pkg-dump <sav.dat> <NodeName> [ChunkType]` | readable dump of an object package |
| `pkg-check <sav.dat>` | parse + re-serialize every package node, require identical bytes |

Edit commands never modify their input: they write a new save folder and refuse to overwrite.
Copy the result into `%USERPROFILE%\Saved Games\CD Projekt Red\Cyberpunk 2077\` yourself.

## Documentation

* [docs/save-container.md](docs/save-container.md): `sav.dat` layout, compression, node tree, version gates
* [docs/facts.md](docs/facts.md): quest facts and `ResetAttributeDisabled`
* [docs/package-format.md](docs/package-format.md): embedded RTTI object packages
* [docs/player-development.md](docs/player-development.md): attributes/perks/points, and what a reset changes
* [docs/game-binary.md](docs/game-binary.md): reverse-engineering notes (function RVAs, node API, tools in `re/`)

## Layout

```
src/lib.rs       container reader/writer, node tree, node resizing
src/facts.rs     FactsDB / FactsTable
src/package.rs   embedded object packages
src/main.rs      CLI
re/              Python RE helpers (pefile + capstone)
docs/            findings
saves/ gamefiles/ work/ output/   local copies & scratch (git-ignored)
```
