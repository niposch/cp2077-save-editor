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
| `metadata.9.json` strength/intelligence/... | old values | 3 |

Everything except the fact is the reset itself, which is why removing only the fact keeps
the attributes reset and brings the button back.
