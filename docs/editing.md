# Editing saves

All edits go through one command that reads a save folder and writes a **new** folder
(it refuses to overwrite anything):

```
cp2077-save edit <in_dir> <out_dir> [ops...]
  --attr <body|reflexes|tech|intelligence|cool>=<3..20>
  --points <attribute|perk|relic>=<n>      unspent points
  --level <1..60>   --street-cred <1..50>
  --life-path <corpo|nomad|streetkid>
  --money <eddies>
  --unlock-reset                           allow "Reset Attributes" again
  --fact <name|0xHASH>=<n>
  --remove-fact <name|0xHASH>
  --set <Node>:<path>=<value>              generic package edit
```

Ops run in order. Example:

```
cp2077-save player saves/ManualSave-17
cp2077-save edit saves/ManualSave-17 output/ManualSave-17-edited --attr body=10 --attr int=16 --points perk=10
```

Copy the output folder into `%USERPROFILE%\Saved Games\CD Projekt Red\Cyberpunk 2077\`
under a new name (e.g. `ManualSave-99`) and load it. Keep a backup.

## What each op touches

| op | where | in-game test |
|---|---|---|
| `--attr` | `PlayerDevelopmentData.attributes[attributeName=X].value` **and** the attribute's `gameConstantStatModifier` in StatsSystem's `sharedDataBuffer`; also `metadata.9.json` | ✅ works |
| `--points` | `PlayerDevelopmentData.devPoints[type=T].unspent` (attribute = entry without `type`, perk = `Primary`, relic = `Espionage`) | ✅ works |
| `--level` | `proficiencies[type=Level].currentLevel` (+ `currentExp` → 0), the `Level` stat modifier in StatsSystem, `LevelAssignmentSystem_Ver1` (float), metadata `level`; ±1 unspent attribute and perk point per level | ✅ works |
| `--street-cred` | `proficiencies[type=StreetCred].currentLevel` (+ `currentExp` → 0), the `StreetCred` stat modifier, metadata `streetCred` | ✅ works |
| `--life-path` | `PlayerDevelopmentData.lifePath` (absent = Corporate) and the facts `q000_corpo/nomad/street_kid_background`; metadata `lifePath` | ✅ works |
| `--money` | quantity of V's `Items.money` item ([inventory.md](inventory.md)) | ✅ works |
| `--unlock-reset` | removes fact `ResetAttributeDisabled` | ✅ works |
| `--fact` / `--remove-fact` | FactsTable | (unlock-reset uses it) |
| `--set` | anything in an object package | n/a |

Safety checks:
* Attribute values are limited to the game's own range, 3–20.
* `--attr` requires exactly one matching stat modifier in StatsSystem. 1.6-era saves don't
  have these, so they're refused.
* After every package edit the package is re-serialized and re-parsed, and the result must be
  identical to the edited tree.
* `PlayerDevelopmentData` is selected by `ownerID.hash=1` (V). Older saves contain several
  copies for NPCs.

Changing the level grants (or, when lowering, takes back unspent) one attribute point and one perk
point per level, like a real level-up. If points were already spent, lowering the level warns
instead of going negative.
Difficulty isn't stored in the save. It's an in-game setting, so it's not part of the editor.

Changing an attribute doesn't touch `devPoints.spent` for attributes. The game doesn't seem to
validate totals on load: there's no such script function, only `EspionageAttributeRetrofix`.

## Generic paths (`pkg-get`, `--set`)

```
<ChunkType>[n] | <ChunkType>[field=value] | #<chunkIndex>     first segment: which object
.field                                                         struct field
.field[i] | .field[key=value] | .field[a.b=value]              array element
.field:Type                                                    type, when inserting an omitted field
```

* `[key=]` (empty value) matches elements whose `key` is absent, i.e. holds its default value.
* Setting an absent field **inserts** it. The type and field position are copied from another
  struct of the same type in the package; if there's none, give the type explicitly
  (`.foo:Int32=5`).
* Value syntax: numbers, `true/false`, enum value names (`Primary`), CNames as plain text,
  TweakDBIDs as `0x...` or a record path (`Items.Preset_Lexington_Default` →
  `crc32(path) | len << 32`).
* Whole structs/arrays can't be inserted yet.

Examples:

```
cp2077-save pkg-get saves/ManualSave-18 ScriptableSystemsContainer "PlayerDevelopmentData[ownerID.hash=1].devPoints[type=Primary]"
cp2077-save edit saves/ManualSave-18 out/x --set "ScriptableSystemsContainer:PlayerDevelopmentData[ownerID.hash=1].proficiencies[type=Espionage].currentExp=10"
```

**Caution:** `--set` changes exactly one value. If the game keeps a copy elsewhere (as with
attributes in StatsSystem), the copies may disagree. Diff a before/after save made in-game first
(`cp2077-save diff`) to find every place a value lives.
