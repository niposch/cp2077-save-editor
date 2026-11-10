# Embedded RTTI object packages

Several nodes hold a self-describing serialized object graph rather than a hand-written layout:

`ScriptableSystemsContainer` (all script systems: PlayerDevelopmentData, EquipmentSystem,
CraftingSystem, vendors, fast travel, ...), `StatsSystem`, `StatPoolsSystem`, `godModeSystem`,
`tierSystem`, `MovingPlatformSystem`, `GuardAreaManager`, `PreventionSpawnSystem`,
`scanningController`, `RenderGameplayEffectsManagerSystem`.

In the game, the ScriptableSystemsContainer loader (RVA `0xe18240`) opens the node and hands
the stream to a generic package reader (`0x4b7ed4`).

Implementation: [src/package.rs](../src/package.rs). **Every package node in every test save
(2.5 and 1.6) parses and re-serializes byte-for-byte.** Check with `cp2077-save pkg-check`.

## Node wrapper

```
u32 nodeId
u32 packageLength
package[packageLength]
(trailing bytes, kept verbatim; none seen so far)
```

## Package

```
u16 version            4
u16 numSections        6 (7 would add a ref pool; not seen)
u32 numComponents
u32 nameDescOffset     (relative to base; always 0)
u32 nameDataOffset
u32 chunkDescOffset
u32 chunkDataOffset
[u32 numCruids, u64 cruids[numCruids]]   present only in some packages (ScriptableSystemsContainer)
base:
  name desc:  u32 per name = offset (low 24 bits) | length incl. NUL (high 8 bits)
  name data:  NUL-terminated strings: class, field, type and enum value names
  chunk desc: { u32 typeNameIndex, u32 offset } per object
  chunk data: objects
```

The CRUID list is detected by checking whether the first name-desc entry points at `nameDataOffset`.

## Objects / structs

```
u16 propCount
propCount x { u16 nameIndex, u16 typeIndex, u32 offsetFromStructStart }
values, in the same order, packed back to back
```

**Properties at their default value are omitted.** That's why e.g. the first array entry often
lacks `type`: it holds the enum's first value. To change a default-valued field you must
*insert* the property; the writer regenerates all offsets and appends new names to the pool.

## Value encodings (by type name)

| type | encoding |
|---|---|
| `Bool`, `Int8`, `Uint8` | 1 byte |
| `Int16`, `Uint16` | 2 bytes |
| `Int32`, `Uint32`, `Float` | 4 bytes |
| `Int64`, `Uint64`, `Double`, `CRUID`, `EngineTime` | 8 bytes |
| `TweakDBID` | u64: crc32 of record path (low 32) + path length (bits 32–39) |
| `CName` | u16 name-pool index |
| enum types (`gamedataStatType`, `ELightState`, ...) | u16 name-pool index of the value name |
| `handle:T`, `whandle:T` | i32 chunk index |
| `NodeRef`, `String` | u16 length + bytes |
| `DataBuffer` | u32 length + opaque bytes (nested serialization, not decoded yet) |
| `array:T`, `[N]T` | u32 count + elements |
| anything else | nested struct (same layout as objects) |

### Enum vs. struct

The package doesn't say which type names are enums. The parser decides like this:
1. A value with a known size of exactly 2 bytes and a non-zero u16 is an enum. `00 00` is an
   *empty struct* (0 props), and name index 0 is never an enum value.
2. An `array:T` whose byte size is exactly 2·count with all non-zero u16s → `T` is an enum.
3. Otherwise, check whether the bytes form a valid struct header (indices in range, first value
   offset == 2 + 8·propCount, offsets non-decreasing). If not → enum.
4. Learned enum types are reused; the package is re-parsed until the set is stable.

A full RTTI dump from the game would remove the need for heuristics, but the byte-exact round
trip shows they're right for every save tested.

## Examples

```
cp2077-save pkg-dump saves/ManualSave-17/sav.dat ScriptableSystemsContainer PlayerDevelopmentData
cp2077-save pkg-dump saves/ManualSave-17/sav.dat StatsSystem
```

See [player-development.md](player-development.md) for what the attribute reset changes.
