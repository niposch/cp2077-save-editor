# Cyberpunk 2077 Save Editor

An unofficial save editor for Cyberpunk 2077 that runs in your browser.

**[Open the editor →](https://niposch.de/cp2077-save-editor/)**

Everything happens locally: your save is read and edited in the page itself (the editing core is
Rust compiled to WebAssembly) and is never uploaded anywhere. Edits are downloaded as a **new** save
folder; your original save is never touched.

## What you can edit

| | |
|---|---|
| Attributes | Body, Reflexes, Technical Ability, Intelligence, Cool (3–20) |
| Points | unspent attribute, perk and relic points |
| Level | 1–60. Like a real level-up, each level adds 1 attribute and 1 perk point |
| Street cred | 1–50 |
| Money | eddies |
| Life path | Street Kid / Nomad / Corpo |
| Attribute reset | make the once-per-playthrough "Reset Attributes" available again |
| Quest facts | look up and change the game's quest flags by name |
| Advanced | browse and edit any value in the decoded game systems (experimental) |

Every edit on the Character tab has been verified in-game. Pending changes are listed with
`old → new` and can be reverted one by one before you save.

Tested with saves from game **2.0 – 2.5** (also a 2.31 save). Saves from a newer patch show a
warning: a game update can change the save format. Pre-2.0 saves load, but attribute and level
edits need the 2.0 progression system.

## How to use it

1. **Back up your saves.** Copy the whole folder
   `%USERPROFILE%\Saved Games\CD Projekt Red\Cyberpunk 2077` somewhere safe.
   (Steam/GOG cloud saves sync this same folder, so they're not a backup.)
2. Open the [editor](https://niposch.de/cp2077-save-editor/) and choose a save folder
   (e.g. `ManualSave-17`), or drop it onto the page. You can also try the bundled example save.
3. Make your changes, then **Save as…** to download the edited save as a `.zip`.
4. Extract it into your saves folder under a new name (e.g. `ManualSave-99/sav.dat`) and load it
   in the game.

Not supported (yet): items/inventory editing beyond money, perk trees, skill progression.

## Example saves

[`examples/`](examples/) holds saves the website offers under "Try an example save" (see
[examples/README.md](examples/README.md) for credits). They belong to their authors and **are not
covered by this repository's license**.

## License

[MIT](LICENSE), except the third-party example saves in `examples/`.

This is an unofficial fan tool, not affiliated with or endorsed by CD PROJEKT RED. Cyberpunk 2077
is a trademark of CD PROJEKT S.A.

---

## For developers

```
npm --prefix app install
npm --prefix app run dev        # website on http://localhost:5173 (builds the wasm core first)
npm --prefix app run build      # production build → app/dist (deployed to GitHub Pages by CI)
cargo test -p cp2077-save -p cp2077-save-wasm
```

Needs Rust with the `wasm32-unknown-unknown` target, `wasm-pack` and Node 20+. The same UI can also
be built as a Tauri desktop app (`npm --prefix app run desktop`); it's not distributed yet.

```
crates/core      save format: container, facts, object packages, path edits, player edits (no I/O)
crates/cli       command-line tool (`cp2077-save`, see docs/editing.md)
crates/wasm      WebAssembly bindings used by the UI
app/             React + shadcn/ui frontend; app/src-tauri = desktop shell
examples/        example saves for the website (third-party, credited)
re/              reverse-engineering helpers (Python: pefile + capstone)
docs/            findings and format documentation
```

### Documentation

* [docs/save-container.md](docs/save-container.md): `sav.dat` layout, compression, node tree, version gates
* [docs/package-format.md](docs/package-format.md): embedded RTTI object packages
* [docs/player-development.md](docs/player-development.md): attributes, points, level, street cred, life path
* [docs/inventory.md](docs/inventory.md): inventory layout and money
* [docs/facts.md](docs/facts.md): quest facts and `ResetAttributeDisabled`
* [docs/editing.md](docs/editing.md): CLI edits and the generic path syntax
* [docs/ui.md](docs/ui.md): UI architecture, publishing, interaction details
* [docs/game-binary.md](docs/game-binary.md): notes from the game executable
