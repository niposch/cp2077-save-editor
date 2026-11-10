# Cyberpunk 2077 Save Editor (Rust)

A save reader/writer for Cyberpunk 2077, built by reverse engineering game 2.5 saves
(`saveVersion` 277) and the game binary. Also tested on a 1.6-era save.

**Status**
* Container (decompress / recompress / node tree / node resizing): done, round-trips exactly.
* Quest facts: read/write. **Restoring the once-per-playthrough attribute reset works in-game.**
* Embedded object packages (ScriptableSystemsContainer, StatsSystem, ...): generic reader +
  writer, byte-exact round trip on all test saves, path-based get/set.
* Player edits: attributes (kept in sync with StatsSystem), attribute/perk/relic points.
  See [docs/editing.md](docs/editing.md).
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
| `player <save>` | level, attributes, points, reset status |
| `pkg-dump <sav.dat> <NodeName> [ChunkType]` | readable dump of an object package |
| `pkg-get <save> <NodeName> <path>` | one value from a package |
| `pkg-check <sav.dat>` | parse + re-serialize every package node, require identical bytes |
| `edit <in_dir> <out_dir> [ops]` | `--attr`, `--points`, `--unlock-reset`, `--fact`, `--remove-fact`, `--set` ([docs/editing.md](docs/editing.md)) |
| `set-fact` / `remove-fact` / `unlock-reset <in_dir> <out_dir> ...` | single-op shortcuts |

Every command also accepts a save folder where it says `<sav.dat>`. Run with `--help` for the full list.

Edit commands never modify their input: they write a new save folder and refuse to overwrite.
Copy the result into `%USERPROFILE%\Saved Games\CD Projekt Red\Cyberpunk 2077\` yourself.

## Documentation

* [docs/editing.md](docs/editing.md): how to edit, path syntax, what each edit touches
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
src/edit.rs      path-based get/set (with insertion of omitted fields)
src/player.rs    player-level edits (attributes, points)
src/savedir.rs   writing a new save folder + metadata patches
src/main.rs      CLI
re/              Python RE helpers (pefile + capstone)
docs/            findings
saves/ gamefiles/ work/ output/   local copies & scratch (git-ignored)
```
