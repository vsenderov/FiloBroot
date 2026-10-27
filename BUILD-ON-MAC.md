# Building and testing FiloBroot on a Mac

Everything here was written on a Linux box without a Mac. The Rust side was compiled and tested there (and type-checked for `aarch64-apple-darwin`); the Swift side was only syntax-checked (`swiftc -parse`), plus a type-check of the broot calls against the generated UniFFI bindings. **The app has never been compiled against AppKit or run.** Expect a few compile errors on the first build; they should be small (a wrong AppKit method name, a missing label).

## 1. Prerequisites (once)

Command Line Tools (no Xcode needed) and rustup's Rust (not Homebrew's):

```sh
xcode-select --install            # if `swift --version` doesn't work yet
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # if no rustup yet
rustup update stable              # broot 1.60.2 and edition 2024 need a recent Rust
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Swift 6.0 or newer is needed (`swift --version`; CLT for macOS 15 ships 6.x).

## 2. Build

```sh
tar xzf filofinder-broot.tgz && cd filofinder-broot
scripts/build.sh --native         # this Mac's architecture only, release
# or: scripts/build.sh            # universal (arm64 + x86_64), about twice as long
# or: scripts/build.sh --native --run   # build, then launch
```

The script builds the Rust core, regenerates the UniFFI Swift bindings and header from the freshly built library (so they always match), builds the Swift package with SwiftPM, assembles the bundle and signs it ad hoc. The first build downloads and compiles the crates (broot pulls in a lot), so allow several minutes. It ends with:

```
✓ /…/filofinder-broot/build/FiloBroot.app (arm64)
```

To run the Rust tests on the Mac too (they include broot itself on a temp dir):

```sh
cargo test --manifest-path core/Cargo.toml
cargo run --release --manifest-path core/Cargo.toml -p filofinder-broot --example perf   # ~100k-file timing
```

## 3. First launch (Gatekeeper)

The app is signed ad hoc, not notarized. Open `build/FiloBroot.app` (double-click or `open build/FiloBroot.app`). If macOS refuses ("cannot be opened" / "could not verify"), go to **System Settings → Privacy & Security**, scroll to the message about FiloBroot and click **Open Anyway**, then confirm. Built locally it usually isn't quarantined, so this may not appear.

Optionally move it to /Applications. For protected folders (Desktop, Documents, Downloads, external drives) macOS asks for access per folder, or grant Full Disk Access in Privacy & Security. Note that an ad-hoc signature changes with each build, so macOS may ask again after a rebuild.

## 4. Default file viewer (optional, unverified)

So that "Reveal in Finder" / "Show in Folder" in other apps (Cursor and other Electron apps via `shell.showItemInFolder`, browsers' downloads, anything calling `NSWorkspace activateFileViewerSelectingURLs`) opens FiloBroot:

```sh
defaults write -g NSFileViewer -string com.vsenderov.filobroot
```

Or use **FiloBroot → Default File Browser…** in the app menu, which sets the same preference. To go back to Finder:

```sh
defaults delete -g NSFileViewer
```

Apps read this preference when they start, so quit and reopen the apps you reveal from (or log out and in). Finder stays in charge of the Desktop, the Dock, and the system Open/Save panels.

How it works: the bundle has the stable identifier `com.vsenderov.filobroot` and declares folders (`public.folder`, `public.directory`) as a Viewer document type. Revealing sends the reveal Apple Event (`misc`/`mvis`), which opens the parent folder with the items selected. An Open Documents event (`application(_:open:)`) opens a folder in a new tab and reveals a file (or package) in its folder. A folder that launches the app becomes its start folder. There is no Services menu entry.

## 5. Keys that changed from FiloFinder

| Key | Now | Was |
|---|---|---|
| typing letters | broot filter (live) | type-select |
| ⌫ | delete the last filter character, or go to the parent folder | Move to Trash |
| ⌘⌫ / Del | Move to Trash (as in Finder) | delete permanently (⌘⌫) |
| ⌥⌘⌫ / ⌘Del | delete permanently (asks first) | |
| Esc | clear the filter, then the selection | clear selection |
| ⌘L (also ⇧⌘G) | Go to Folder sheet with the current path selected, so ⌘C copies it | ⇧⌘G sheet |
| ⌘+ / ⌘− / ⌘0 | whole-UI zoom / actual size | icon size |
| ⌃⌘+ / ⌃⌘− | grid icon size | |
| S | types "s" into the filter | multi-select mode (the toolbar button remains) |

Settings in `defaults` (domain `com.vsenderov.filobroot`): `uiZoom` (0.75–2.0), `brootMaxRows` (default 500).

## 6. Manual test checklist

Build and launch:
- [ ] `scripts/build.sh --native` ends with `✓ …/build/FiloBroot.app`; the app opens on your home folder.

Path and parent folder:
- [ ] The path crumbs in the bar are clickable; clicking one opens that folder, and the last crumb is the current folder.
- [ ] ⌘↑ and ⌫ (with an empty filter) go to the parent folder.
- [ ] ⌘L (and ⇧⌘G) opens the Go to Folder sheet with the current path already selected; ⌘C copies it. Type `~/Downloads` and press Go to open it; focus returns to the list.
- [ ] Go on `/etc/hosts` opens /etc with `hosts` selected.
- [ ] Go on `../`, on `'~/My Folder'`, on `file:///Applications`, and on `~/My\ Folder` all work.
- [ ] Go on `/nope` beeps and shows "No such file or folder" and stays in the current folder.
- [ ] ⌥⌘C copies the path of the selection (or of the folder when nothing is selected).

broot filter:
- [ ] In a big folder (e.g. a source checkout), typing filters as you type and the status bar shows `broot N ms · N rows`; rows are indented as a tree, the matched letters are bold and coloured, and broot's best match is selected.
- [ ] Typing fast never shows stale results and the UI doesn't stall.
- [ ] Try `/\.md$/`, `p/src/main`, and a nonsense pattern (shows "No matching files"); a bad regex like `/[/` shows the broot error.
- [ ] ↑/↓ move the selection; Enter on a folder makes it the root (filter cleared); Enter on a file opens it.
- [ ] Space opens Quick Look on a nested match; ←/→ in Quick Look step through the rows.
- [ ] Dragging a nested match to another app or folder copies or moves it as FiloFinder did before; dropping files onto the window works.
- [ ] ⌘⌫ trashes a nested match; ⌘Z puts it back; copy/cut/paste work on nested matches.
- [ ] ⌫ removes the last pattern character, and with an empty pattern goes to the parent folder; Esc clears the pattern and restores the folder with the selection kept (a nested selection maps to its top-level folder).
- [ ] The filter field in the bar mirrors what you type; typing in the field itself works the same, and ↑/↓/Enter there act on the list.
- [ ] With grid view on, a filter shows a list; clearing it goes back to the grid.
- [ ] Show Hidden Files (⇧⌘.) affects what broot finds; files in `.gitignore` are not found (broot's default).
- [ ] Tag tabs (sidebar tags) still use a plain substring filter.

Zoom:
- [ ] ⌘+ / ⌘− / ⌘0 scale everything together (sidebar, tabs, toolbar, path crumbs, list rows, grid icons, status bar) and a HUD shows the percentage; the View menu shows "Actual Size (N%)".
- [ ] The zoom level survives a relaunch.
- [ ] Zooming with a filter active and with the preview pane open keeps things working.

Explorer:
- [ ] Arrow down onto Trash in the Explorer: the Trash opens and its row stays highlighted (it doesn't jump back to your home folder). Opening Trash from the toolbar highlights it too.

Unchanged behaviour (regression):
- [ ] Dual pane is hidden (broken upstream): no split button in the toolbar, no Split View item in the View menu, and ⌘M does nothing.
- [ ] Tabs, the folder tree, rename (Enter on a selected item), new folder, duplicate, tags, Trash with Put Back, zip extraction, and live refresh when files change elsewhere.

Default file viewer:
- [ ] Run the `defaults write` above, restart Cursor, right-click a file → Reveal in Finder: FiloBroot opens its folder with the file selected.
- [ ] Do the same with FiloBroot not running (it launches and reveals).
- [ ] `open -a FiloBroot ~/Documents` opens a tab there.
- [ ] `defaults delete -g NSFileViewer` restores Finder.

## 7. Unverified / known risks

- **Nothing in Swift has been compiled against AppKit.** First-build compile errors are likely; fix as they come.
- **Rename in a filtered tree**: row names are paths relative to the root (`a/b/c.txt`); renaming a nested match may get the path wrong. Rename from the unfiltered folder if it misbehaves.
- **Live refresh in a filtered view** watches only the root folder; changes deep inside don't re-run the search until you type again.
- **Zoom** scales sizes and fonts but not every padding or spacing constant, so very high zoom can look cramped. The ⌘+ key match (with a hidden ⌘= alternate) depends on the keyboard layout and is untested.
- **Typing goes to the filter** only when the list has focus; toolbar buttons clicked while a text field is being edited act on files.
- **Default file viewer**: whether Electron's `showItemInFolder` and the system honour `NSFileViewer` for this app is untested. If it doesn't work, check that the bundle ID in `build/FiloBroot.app/Contents/Info.plist` matches, and run `/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f build/FiloBroot.app` (or move the app to /Applications and launch it once).
- **Performance on macOS** was not measured; the numbers in the README are from a Linux box. broot's search on APFS should be similar or faster.
- The universal (x86_64) build was not tried; `--native` is the tested path.
