# Notes from the game binary

Static analysis of a **copy** of `bin/x64/Cyberpunk2077.exe` (game 2.5, GOG, linker timestamp
`6ad55daf`, 2026-10-19). RVAs below are for that build only; image base `0x140000000`.

## Tools (in `re/`)

* `re/pe.py`: loads the exe (pefile), function bounds from `.pdata`, finds byte patterns,
  RIP-relative references to an address, and direct `call`/`jmp` callers (numpy scan).
* `re/disfn.py <rva> ...`: disassembles whole functions (capstone), annotating string
  references and call targets.
* `re/find_save.py`: the search that found the save code (container magics + node-name strings).
* `re/pkg.py`: the Python prototype of the package parser (superseded by `src/package.rs`).

Needs `pip install pefile capstone numpy`. Expects copies in `gamefiles/` (git-ignored):
`Cyberpunk2077.exe`, `cyberpunk2077_addresses.json`, `final.redscripts`.

## `cyberpunk2077_addresses.json`

CDPR ships an address database next to the exe for modding frameworks such as RED4ext:
1,118,540 entries `{hash, secondary hash, offset "0001:xxxxxxxx", [symbol]}`. Section 0001 = `.text`,
so RVA = `0x1000 + offset`. Only 84 entries have a symbol name; they're generic engine functions
(`rtti::ClassType::Serialize`, `CBaseEngine::LoadScripts`, ...). No save-specific names, but the
hashes give stable IDs for functions across patches.

## How the save code was found

The container magics appear as 32-bit immediates in exactly two `.text` clusters:

| RVA | what |
|---|---|
| `0xfa5548` | **ReadHeader**: reads `VASC`, calls ReadVersionInfo, reads archiveVersion (≤ `0xC3`) |
| `0xfa55f0` | **ReadVersionInfo**: version gates (see [save-container.md](save-container.md)), gameDef string, u64 timestamp |
| `0xfa5330` | **ReadChunkTable**: `FZLC`, count, 12-byte entries |
| `0xfa5418` | **ReadNodeTable**: seek to end−8, read offset + `ENOD`, seek, `EDON`, read descriptors |
| `0xfa54e8` | node descriptor list reader |
| `0x2fcd394`, `0x2fcd418`, `0x2feedf4` | second cluster with the same magics; probably the **writer** (not examined yet) |
| `0x22cdb40`, `0x22cdbf0`, `0x22cdcb0` | `4ZLX` chunk compression/decompression |

## Node API (how every system serializes)

Each system opens a node by CName, serializes, and closes it:

```
NodeScope scope(stream, CName("FactsTable"), ...);  // 0x7408b0  (BeginNode)
if (stream->IsNodeOpen())                           // vtable +0x90
    ... stream->Serialize(ptr, size)                // vtable +0x10
// ~NodeScope                                       // 0x740a98  (EndNode)
```

* Stream flags at `+8`: bit `0x2` = reading.
* `0x27b824` = read packed count (vlq); `0x2f60040` = write packed count.
* CName getters such as `0x739b24` lazily register a name (`NameBuilder::Register`) and return
  its 64-bit hash (`FactsTable` → `0x69123900eb232858`).

To find a node's (de)serializer: search for the node-name string, find the CName getter that
references it, then its callers.

| RVA | function |
|---|---|
| `0x7399bc` | FactsDB load: vlq count, at most 10 FactsTables |
| `0x739a68` | FactsTable serialize (read): vlq count, `count*4` hashes, `count*4` values |
| `0x2cd95b4` | FactsTable serialize (write path) |
| `0xe18240` | ScriptableSystemsContainer load → package reader `0x4b7ed4` |

## Ideas for next steps

* Dump the game's RTTI (class/enum/property lists) to replace the enum-vs-struct heuristics.
  This could be done statically from the registration functions, or at runtime with a
  RED4ext/CET plugin.
* Examine the writer cluster to confirm there's no extra validation on load.
* Decode `DataBuffer` (stat modifiers in StatsSystem) and the `inventory`/`itemData` nodes.
