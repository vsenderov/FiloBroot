# FiloBroot

**A native macOS file manager where typing searches the whole tree with [broot](https://github.com/Canop/broot)'s fuzzy matcher.** It is a fork of [FiloFinder](https://github.com/bipul0525/FiloFinder) by bipul0525 (a fast, MIT-licensed Finder alternative with an AppKit UI over a Rust core), with broot's search engine linked in as a library and a few other changes.

<!-- TODO: add a screenshot or GIF here, e.g. docs/filobroot.png, then reference it with a Markdown image -->

- **Type to search.** Typing in a folder runs broot's tree search on a background thread: fuzzy by default, plus `/regex/`, `p/path` and `c/content` patterns. Every keystroke cancels the search still running, so it stays responsive. Results are ordinary rows of real files, shown as a tree with the matched characters highlighted, so Quick Look, drag and drop, copy/cut/paste and Trash all work on them.
- **Whole-UI zoom.** ⌘+ / ⌘− / ⌘0 scale fonts, row heights, icons and bars together (75%–200%), and the level is remembered.
- **Tab** moves keyboard focus between the Explorer (folder tree) and the file list, with a visible focus ring.
- **Everything else is FiloFinder:** clickable path crumbs, tabs, dual pane (⌘M), folder tree, Quick Look, real cut and paste with undo, tags, Trash with Put Back, grid and list views. Sparkle auto-updates and the upstream release tooling were removed, and the bundle ID is `com.vsenderov.filobroot`.

## Requirements

- macOS 13 (Ventura) or newer.
- Apple Silicon is what it is developed and tested on. The build script can also produce an Intel or universal binary, but that path is untested. The prebuilt release zip is Apple Silicon (arm64) only.
- **Full Disk Access** is recommended: it is a file manager, so without it macOS prompts folder by folder and some locations stay hidden.

## Install from a release

The app is **not notarized** (that needs a paid Apple Developer account), so macOS will warn the first time. It is open source and built by GitHub Actions from the tagged source, so you can also [build it yourself](#build-from-source).

1. Download `FiloBroot-<version>-arm64.zip` from the [Releases page](https://github.com/vsenderov/FiloBroot/releases) and unzip it.
2. Move `FiloBroot.app` to `/Applications`.
3. Clear the quarantine flag that macOS puts on downloads:

   ```sh
   xattr -dr com.apple.quarantine /Applications/FiloBroot.app
   ```

   Alternatively, try to open the app once, then go to **System Settings → Privacy & Security**, scroll down to the message about FiloBroot and click **Open Anyway**. (On older macOS versions, right-click the app and choose **Open** also works. On macOS 15 and later the Settings route or `xattr` is needed.)
4. Open FiloBroot. When it offers to, or whenever you like, grant it **Full Disk Access** in **System Settings → Privacy & Security → Full Disk Access** (click +, pick FiloBroot, and restart the app).

The release is signed ad hoc, and an ad-hoc signature is different for every build, so macOS may ask for Full Disk Access again after you install a new version. Remove the old FiloBroot entry from the list and add the new one if it does not stick.

## Build from source

You need a Mac, because the UI is AppKit. There is no Xcode project: the script builds the Rust core, generates the Swift bindings with UniFFI, builds the Swift package and assembles the `.app` bundle.

Prerequisites:

- **Xcode 16 or the Command Line Tools for it** (Swift 6.0 or newer; check with `swift --version`). Full Xcode is not required: `xcode-select --install` is enough.
- **Rust 1.88 or newer via [rustup](https://rustup.rs)** (the dependencies need 1.88, and the workspace uses edition 2024). Use rustup's toolchain, not Homebrew's `rust`, because the build script puts `~/.cargo/bin` first in `PATH`.

```sh
# one-time setup
xcode-select --install                                # skip if `swift --version` already works
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # skip if you have rustup
rustup update stable
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# build
git clone https://github.com/vsenderov/FiloBroot.git
cd FiloBroot
zsh scripts/build.sh --native        # this Mac's architecture only; produces build/FiloBroot.app
open build/FiloBroot.app
```

The first build downloads and compiles all the crates (broot pulls in a lot), so allow several minutes. Other options: `zsh scripts/build.sh` builds a universal (arm64 and x86_64) app and takes about twice as long, `--debug` makes a fast debug build and `--run` launches the app after building. The script signs the app ad hoc unless a signing identity named "Filofinder Dev" is in the author's keychain, so it works on a fresh machine. Because an ad-hoc signature changes with every build, macOS may ask for Full Disk Access again after a rebuild. More detail, a manual test checklist and notes on the default-file-viewer feature are in [BUILD-ON-MAC.md](BUILD-ON-MAC.md).

To run the Rust tests: `cargo test --manifest-path core/Cargo.toml`.

## Tip: make search fast by skipping ~/Library

FiloBroot reads your normal broot configuration (`$BROOT_CONFIG_DIR/conf.hjson`, else `~/.config/broot/conf.hjson`, else `~/Library/Application Support/org.dystroy.broot/conf.hjson`). It never creates or writes a config. Searching from your home folder is much faster if broot does not descend into `~/Library`, which holds a huge number of files. Create `~/.config/broot/conf.hjson` with:

```hjson
special_paths: {
    "~/Library": "no-enter"
}
```

`~/Library` itself can still match by name, but nothing inside it is searched. Like broot, the search respects `.gitignore` and shows hidden files only when "Show Hidden Files" is on.

## Keys

| Key | Action |
|---|---|
| type letters | broot search in the current folder (fuzzy; `/regex/`, `p/path`, `c/content`) |
| ↑ / ↓, Enter | move through results, open a folder as the new root or open a file |
| ⌫ | delete the last search character, or go to the parent folder when the search is empty |
| Esc | clear the search, then the selection |
| ⌘↑ | go to the parent folder |
| ⌘[ / ⌘] | back / forward |
| ⌘L or ⇧⌘G | Go to Folder (shows the current path selected, so ⌘C copies it) |
| ⇥ / ⇧⇥ | switch keyboard focus between the file list and the Explorer |
| ⌘+ / ⌘− / ⌘0 | zoom the whole UI in / out / reset |
| ⌃⌘+ / ⌃⌘− | grid icon size |
| Space | Quick Look |
| ⌘X / ⌘C / ⌘V | cut / copy / paste (cut and paste moves files) |
| ⌘⌫ | move to Trash |
| ⌥⌘⌫ | delete permanently (asks first) |
| ⌘Z | undo |
| ⌘T / ⌘N / ⌘W | new tab / new window / close |
| ⌘M | dual pane |
| ⌘1 / ⌘2 | list / grid view |
| ⇧⌘. | show hidden files |
| ⌥⌘C | copy path |

## Known issues

- Finder tag colour dots are turned off in the file list and grid. Upstream FiloFinder reads tags on the main thread, which can freeze the window while macOS checks privacy permission ([#2](https://github.com/vsenderov/FiloBroot/issues/2)), so FiloBroot skips them. Tags still work in the sidebar and the right-click menu.
- [#5](https://github.com/vsenderov/FiloBroot/issues/5): a row can stay highlighted (stuck hover) while scrolling.
- [#6](https://github.com/vsenderov/FiloBroot/issues/6): the window's traffic-light buttons are small.
- Making FiloBroot the system-wide default folder handler did not work on macOS 26 in testing. The **FiloBroot → Default File Browser…** menu item and the `defaults write -g NSFileViewer` approach in [BUILD-ON-MAC.md](BUILD-ON-MAC.md) are experimental.
- Intel and universal builds are untested.
- Renaming a nested search result can go wrong; rename from the unfiltered folder.

See all [open issues](https://github.com/vsenderov/FiloBroot/issues), and please report new ones there.

## Performance

broot itself does the search, with broot's defaults (it respects `.gitignore`, shows hidden files only when the window does). Measured on a synthetic tree of 100,000 empty files in 1,000 folders (`cargo run --release -p filofinder-broot --example perf`), on a Linux x86-64 box, 500-row limit:

| query | FiloBroot (library) | broot CLI |
|---|---|---|
| `echo` (fuzzy, 51 rows) | 115–119 ms | ~134 ms |
| `jul_ind` | 77 ms | |
| `/delta_[0-9]+\.rs$/` | 62 ms | |
| `p/golf_3/hotel` | 79 ms | |
| no match | 66 ms | |
| cancel mid-search | returns 0.2 ms after cancel | |

The broot CLI number is its own search time for the same query and tree. Numbers on a Mac will differ (APFS, a different CPU); the point is that it is the same engine at the same speed.

## Credits and licence

- **FiloFinder** by [bipul0525](https://github.com/bipul0525/FiloFinder), MIT. FiloBroot is a fork of it; the original README is kept in [docs/UPSTREAM-README.md](docs/UPSTREAM-README.md).
- **broot** by [Denys Séguret (Canop)](https://github.com/Canop/broot), MIT, used as a library for search.
- Released under the MIT licence, see [LICENSE](LICENSE). The copyright notice of the FiloFinder contributors is kept as required.
