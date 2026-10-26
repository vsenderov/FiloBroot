# FiloBroot

A bare-bones native macOS file manager: [FiloFinder](https://github.com/bipul0525/FiloFinder) with [broot](https://github.com/Canop/broot)'s search built in, plus three things:

1. **Editable, copyable address bar.** The path row is a text field with the full POSIX path. ⌘L edits it; Enter goes there (`~`, relative paths, `file://` URLs, quoted or shell-escaped paths; a file path opens its folder with the file selected); Esc cancels; ⌘C or the copy button copies it. The old clickable crumbs are one button away.
2. **broot-powered navigation.** Typing in a folder runs broot's own tree search (the `broot` crate, pinned at `=1.60.2`, linked as a library through UniFFI) on a background thread. Every keystroke cancels the search still running (it stops in well under a millisecond) and stale results are dropped. Results are ordinary list rows of real files, indented as a tree with the matched characters highlighted, so Space (Quick Look), drag and drop, copy/cut/paste and Trash work on them. Keys are broot's: type to filter, ↑/↓ to move, Enter opens a folder as the new root (or opens a file), ⌫ edits the pattern or goes to the parent, ⌘↑ goes to the parent, Esc clears. Patterns are broot's too: fuzzy by default, `/regex/`, `p/path`, `c/content`.
3. **Whole-UI zoom.** ⌘+ / ⌘− / ⌘0 scale fonts, row heights, icons, bars and the address bar together (75%–200%); the level is remembered.

It can also act as the macOS **default file viewer**, so "Reveal in Finder" / "Show in Folder" in other apps opens it (see [BUILD-ON-MAC.md](BUILD-ON-MAC.md)).

Everything else is FiloFinder: tabs, split view, folder tree, Quick Look, drag copy/move, cut & paste with undo, tags, Trash with Put Back. Sparkle auto-updates and the upstream release tooling were removed; the bundle ID is `com.vsenderov.filobroot`.

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

## Build

See [BUILD-ON-MAC.md](BUILD-ON-MAC.md). Short version, with Command Line Tools and rustup, no Xcode:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
scripts/build.sh --native   # → build/FiloBroot.app
```

## Credits and licence

- **FiloFinder** by [bipul0525](https://github.com/bipul0525/FiloFinder), MIT. This is a fork of it; its original README is in [docs/UPSTREAM-README.md](docs/UPSTREAM-README.md).
- **broot** by [Denys Séguret (Canop)](https://github.com/Canop/broot), MIT, used as a library for search.
- MIT, see [LICENSE](LICENSE).
