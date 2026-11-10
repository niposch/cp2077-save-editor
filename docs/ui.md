# UI (web + desktop)

One React UI, two targets:

| | web | desktop |
|---|---|---|
| shell | static site (any static host) | Tauri 2 (WebView2 on Windows), single `.exe` |
| save logic | Rust core compiled to wasm, runs in the page | **same wasm**, inside the webview |
| open saves | pick a save folder (or the whole saves folder), or drag & drop | lists `%USERPROFILE%\Saved Games\CD Projekt Red\Cyberpunk 2077` directly |
| write | downloads `<name>.zip` (extract into the saves folder) | creates a new save folder next to the others |

Saves never leave the machine in either version.

## Layout

```
crates/core      save format + edits (pure Rust, no I/O)
crates/cli       command-line tool
crates/wasm      wasm-bindgen bindings: `Session` (open save, edit, write), JSON in/out
app/             Vite + React 19 + TypeScript + Tailwind 4 + shadcn/ui (Radix, Nova preset)
  src/lib/core.ts         typed wrapper around the wasm Session
  src/lib/storage.ts      save sources/targets: web (file picker, zip), desktop (Tauri commands), dev
  src/state/save-context  open save, edit dispatch (`mutate`), change log, discard
  src/components/         save browser, Character / Facts / Advanced tabs, save dialog
  src-tauri/              desktop shell: narrow file commands + dialog plugin
```

## Running

```
npm --prefix app install
npm --prefix app run dev            # web, http://localhost:5173 (builds the wasm first)
npm --prefix app run desktop        # desktop window with hot reload
npm --prefix app run build          # web production build → app/dist
npm --prefix app run build:desktop  # desktop installer (NSIS/MSI); `npx tauri build --no-bundle` for just the exe
```

Needs Rust with the `wasm32-unknown-unknown` target, `wasm-pack`, and Node 20+.

### Dev-only helpers

Only active on the Vite dev server, so they never ship:
* `GET /__dev/saves`: lists the project's copied saves (`saves/`) so the UI can be tested without a picker.
* `POST /__dev/out/<file>`: writes into `work/web-out/`, to capture what the browser would download.
* `window.__cpse`: the open save, for console checks.

## Publishing the website (GitHub Pages)

`.github/workflows/pages.yml` builds the site (wasm core + React + `examples/`) on every push to
`main`/`master` and deploys `app/dist` to GitHub Pages. Turn it on once under
*Settings → Pages → Build and deployment → Source: GitHub Actions*. The build uses relative asset
paths (`base: './'`), so it works under `https://<user>.github.io/<repo>/`.

## Example saves

The web start screen offers the saves listed in `examples/index.json`, with their authors credited
on the card and linked below. The Vite plugin in `app/vite.config.ts` serves `examples/` in dev and
copies it into `dist/examples` for web builds. Desktop builds (run by the Tauri CLI) leave it out.

## Interaction details

* −/+ clicks and ↑/↓ update the shown number instantly and are applied as one edit once you
  pause (250 ms), or immediately on blur, Enter or when the field goes away (e.g. switching tabs).
  Each applied edit takes ~15 ms: enum-type knowledge is kept between package parses (one pass
  instead of two or more), the stats buffer is indexed in one scan, and the player summary is
  computed once per edit and shared.
* Panels draw their straight edges with a real CSS border (pixel-snapped, so it never disappears at
  fractional sizes) and only the two cut corners as 1px diagonal lines.
* Number fields are always real inputs with a fixed width: focusing never shifts the layout,
  Tab moves between fields (the −/+ buttons are skipped), the value is selected on focus so typing
  replaces it, ↑/↓ step, Enter commits, Esc reverts. Out-of-range values are clamped.
* **Pending changes** sit in a sidebar next to every tab. Each entry shows `old → new` and has
  its own revert button. Reverting re-parses the original save and replays every other change, so
  the result is identical to never having made that edit (unit-tested against a real save).
  Repeated edits of the same value merge into one entry; editing a value back drops the entry.
* Long lists (objects in a system, the value tree, listed facts) are virtualized with
  `@tanstack/react-virtual`. Everything is scrollable and only visible rows are rendered.
  The value tree is flattened into rows from its expanded branches.

## Protecting users' saves

* The start screen always shows a **"Back up your saves first"** notice with the saves folder path.
* The Save-as dialog requires ticking **"I have a backup of my saves folder"** before it will write
  or download. Once ticked, it stays ticked for the session.
* Edits never touch the opened save: output always goes to a *new* folder (desktop) or a zip (web).
* The Advanced tab warns that single-value edits may leave copies elsewhere out of sync.

## Desktop file access

The Tauri commands are deliberately narrow (see `app/src-tauri/src/lib.rs`):
* reads only files inside a folder that contains `sav.dat`;
* `create_save_dir` makes a new folder and fails if it exists;
* `write_new_file` only writes into folders created this session, with `create_new` (never overwrites).

## Verification done

* Web: opened saves, edited attributes, points and facts, restored the reset, edited through the
  Advanced tree, discarded edits. The browser-written `sav.dat` decompresses to **exactly** the same
  data as the CLI's for the same edits, and `metadata.9.json` matches apart from `fileSize`.
* The wasm build of `lz4_flex` compresses slightly differently from the native one (~2% larger file);
  it's still a valid LZ4 stream that any LZ4 decoder reads identically.
* Desktop: the frontend and the Rust shell both build. The file commands have unit tests. The desktop
  window hasn't been exercised against a real saves folder yet.

## Look & licensing

The look borrows the game menus' palette (near-black red, salmon-red labels, cyan values, yellow
points) and cut-corner panels. It uses no game assets. Third-party pieces are all permissively licensed:

| what | license |
|---|---|
| shadcn/ui components, Radix UI, Tailwind CSS, React, Vite, Tauri | MIT / MIT-Apache |
| Icons: Lucide | ISC |
| Fonts: Inter, Rajdhani, JetBrains Mono (via Fontsource, bundled) | SIL OFL 1.1 |
| fflate (zip), @tanstack/react-virtual | MIT |

The app icon/favicon is original (`app/public/favicon.svg`).
