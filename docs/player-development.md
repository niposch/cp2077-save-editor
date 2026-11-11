# PlayerDevelopmentData

Location: `ScriptableSystemsContainer` package, chunk type `PlayerDevelopmentData`
(referenced from `PlayerDevelopmentSystem.playerData`).

| field | type | notes |
|---|---|---|
| `ownerID` | `entEntityID` | |
| `queuedCombatExp` | `array:SExperiencePoints` | ~880 entries |
| `proficiencies` | `array:SProficiency` | skill levels/xp: `type`, `currentLevel`, `currentExp`; `Level` = character level, `StreetCred` |
| `attributes` | `array:SAttribute` | `attributeName` (gamedataStatType), `value`, `id` (TweakDBID) |
| `perkAreas` | `array:SPerkArea` | legacy perk areas |
| `traits` | `array:STrait` | |
| `devPoints` | `array:SDevelopmentPoints` | `type` (Attribute/Espionage/Primary/Secondary), `spent`, `unspent` |
| `attributesData` | `array:SAttributeData` | 2.0 perk trees: `unlockedPerks: array:SNewPerk {type, currLevel}` per attribute |
| `highestCompletedMinigameLevel` | `Int32` | |

The first entries of `proficiencies`, `devPoints` and `traits` have no `type`: the default
(first) enum value is omitted. For `devPoints` that entry is the attribute-point pool, since its
numbers match the attribute totals.

## What "Reset Attributes" changes (ManualSave-17 → 18)

| | before | after |
|---|---|---|
| Cool / Intelligence / Reflexes / Body / Tech | 20 / 15 / 5 / 9 / 20 | 3 / 3 / 3 / 3 / 3 |
| devPoints Attribute (spent / unspent) | 47 / 0 | -7 / 54 |
| devPoints Primary (perk points, spent / unspent) | 57 / 4 | 57 / 59 |
| devPoints Espionage (relic, unspent) | 2 | 6 |
| `attributesData[*].unlockedPerks` | populated | emptied |
| fact `ResetAttributeDisabled` | absent | 1 (see [facts.md](facts.md)) |
| StatsSystem `gameConstantStatModifier` per attribute | 20 / 15 / 5 / 9 / 20 | 3.0 each |
| `metadata.9.json` strength/intelligence/... | old values | 3 |

Everything except the fact is the reset itself, which is why removing only the fact keeps
the attributes reset and brings the button back.

## Attributes are stored twice

Besides `PlayerDevelopmentData.attributes`, each attribute has a constant stat modifier in
`StatsSystem` → `gameStatsStateMapStructure.sharedDataBuffer` (a `DataBuffer`):

```
str  "gameConstantStatModifier"     (vlq -24 + chars)
u64  fnv1a64(stat name)             e.g. fnv1a64("Cool") = 0x36540f9d1aca1bdc
u32  modifier type                  0 (additive)
f32  value                          20.0
```

The reset rewrote both (only 9 bytes differed in StatsSystem between saves 17 and 18), so
`--attr` updates both. On 2.5 saves each attribute's record occurs exactly once. 1.6-era saves
have no such records.

Attribute TweakDBIDs are `crc32("BaseStats.<Stat>") | len << 32`, e.g. `BaseStats.Cool` →
`0xb458146e`, length 14.

## Which PlayerDevelopmentData is V's

2.5 saves have one; older saves have several (NPCs). V's has `ownerID.hash == 1`.

## Level and street cred

Both are `proficiencies` entries (`type = Level` / `StreetCred`, `currentLevel`, `currentExp` =
XP within the current level). Like attributes, they have more copies:

| copy | level | street cred |
|---|---|---|
| `proficiencies[type=…].currentLevel` | ✓ | ✓ |
| StatsSystem `gameConstantStatModifier` (`fnv1a64("Level")` / `("StreetCred")`) | 48.0 | 50.0 |
| `LevelAssignmentSystem_Ver1` node: `u32 id, f32 level, u8` | 48.0 | – |
| `metadata.9.json` `level` / `streetCred` | ✓ | ✓ |

Found by decoding the stat modifier buffer: the player's block lists every skill level, then
`StreetCred`, the five attributes, then `Level`. Stat names were resolved by FNV-1a 64 hashing
every identifier in `final.redscripts`. `LevelAssignmentSystem_Ver1` was found by comparing a
level-48 and a level-32 save. The skill levels (Headhunter etc.) sit in the same block, so they
could be edited the same way, but their milestone rewards are granted on level-up and wouldn't follow.

### Level-up points

Each character level-up grants **1 attribute point and 1 perk point** (street cred grants none).
Verified from saves: the attribute pool (spent + unspent, excluding the 7 from character creation)
is exactly level − 1 in a level-48 2.5 save and a level-32 1.6 save. Perk totals are higher than
level − 1 because skill progression also grants perk points (at skill levels 15 and 35 in each of
the five skills) and perk shards add more. The editor's level edit only adds/removes the
level-up share.

### Perk points: what "spent" and "unspent" mean

`devPoints[type=Primary].unspent` is exactly the "Perk points available" number the game shows
(edited to 10 in ManualSave-17, shown as 10 in-game); nothing recomputes it on load.
`spent` is a **lifetime counter**: it doesn't go down when perks are refunded. ManualSave-18 (after a
reset that refunded every perk) still says `spent = 57`. The perks actually owned are
`attributesData[*].unlockedPerks[*].currLevel`.

| save | perk levels bought | spent | unspent |
|---|---|---|---|
| ManualSave-17, level 48 | 56 | 57 | 4 |
| ManualSave-18, after the reset | 0 | 57 | 59 |
| example ManualSave-818, level 50 | 0 | 444 | 221 |

ManualSave-17's 61 points match what the game grants by level 48: 47 from levels, 7 from skill
milestones (15/35 in each skill) and the rest from perk shards. A legitimate level-50 save has at
most about 70, so the example save's 221 unspent points were added outside normal play before it
was published. The editor shows the stored value, which is what the game shows.

## Life path

`PlayerDevelopmentData.lifePath: gamedataLifePath`. `Corporate` is the enum's first value, so Corpo
saves omit the field (the 1.6 Street Kid save has `lifePath = StreetKid`). The prologue also sets
one of the facts `q000_corpo_background` / `q000_street_kid_background` /
`q000_nomad_background` (all three seen in real saves, Nomad in the 2.31 example save). The
editor switches both, and a Corpo → Nomad change was verified in-game.
Corpo → Nomad → Corpo restores the original save data byte-for-byte.
