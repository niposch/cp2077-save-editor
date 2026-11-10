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
