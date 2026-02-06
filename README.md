# grit

A terminal user interface (TUI) for browsing and interacting with GitHub repositories, pull requests, issues, commits, and actions.

![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Home Dashboard** - View PRs requiring your review and your open PRs with CI status
- **Repository Browser** - Browse your GitHub repositories sorted by recent activity
- **Pull Requests** - View, merge, close, comment on, and review PRs
- **Issues** - Browse and close issues, add comments via `$EDITOR`
- **Commits** - View commit history with full diff display
- **Actions** - Monitor GitHub Actions workflow runs
- **Search** - Filter lists and search content with `/`, navigate matches with `n`/`N`
- **External Pager** - View diffs in your configured pager (less, delta, bat, etc.)
- **Vim Keybindings** - Navigate with familiar vim motions
- **Disk Cache** - Instant startup with stale-while-revalidate caching
- **OAuth Device Flow** - Authenticate without manually creating tokens

## Installation

### Pre-built binaries

#### macOS (Apple Silicon)

```bash
curl -fsSL https://github.com/pders01/grit/releases/latest/download/grit-macos-aarch64.tar.gz \
  | tar xz && sudo mv grit /usr/local/bin/
```

#### macOS (Intel)

```bash
curl -fsSL https://github.com/pders01/grit/releases/latest/download/grit-macos-x86_64.tar.gz \
  | tar xz && sudo mv grit /usr/local/bin/
```

#### Linux (x86_64)

```bash
curl -fsSL https://github.com/pders01/grit/releases/latest/download/grit-linux-x86_64.tar.gz \
  | tar xz && sudo mv grit /usr/local/bin/
```

#### Linux (aarch64)

```bash
curl -fsSL https://github.com/pders01/grit/releases/latest/download/grit-linux-aarch64.tar.gz \
  | tar xz && sudo mv grit /usr/local/bin/
```

#### Windows

Download `grit-windows-x86_64.zip` from the [Releases](https://github.com/pders01/grit/releases/latest) page and add `grit.exe` to your PATH.

### From source

```bash
git clone https://github.com/pders01/grit.git
cd grit
cargo install --path .
```

## Authentication

grit tries multiple authentication methods in order:

1. **`GITHUB_TOKEN` environment variable** - instant, no setup needed if already set
2. **Stored token** (`~/.config/grit/token`) - fast file read from previous session
3. **GitHub CLI** (`gh auth token`) - if you have `gh` installed and authenticated
4. **OAuth device flow** - interactive setup, opens browser for authorization

On first run without any existing token, grit will walk you through the OAuth device flow. The token is saved for future sessions.

## Usage

```bash
grit
```

### Keybindings

#### Navigation

| Key | Action |
|-----|--------|
| `q` / `Esc` | Back / Quit |
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `g` / `Home` | Go to top |
| `G` / `End` | Go to bottom |
| `Ctrl+d` / `Ctrl+f` / `PageDown` | Page down |
| `Ctrl+u` / `Ctrl+b` / `PageUp` | Page up |
| `h` / `l` / `Tab` | Switch tabs / sections |
| `Enter` | Select / Open |

#### Search

| Key | Action |
|-----|--------|
| `/` | Enter search mode |
| `n` | Next match |
| `N` | Previous match |
| `Esc` | Clear search |

#### Actions (all views)

| Key | Action |
|-----|--------|
| `r` | Refresh current view (on Home: open repo list) |
| `o` | Open in browser |
| `y` | Copy URL to clipboard |

#### PR Detail

| Key | Action |
|-----|--------|
| `d` | View diff in external pager |
| `m` | Merge PR (choose method) |
| `x` | Close PR |
| `C` | Comment (opens `$EDITOR`) |
| `R` | Submit review (approve / request changes / comment) |

#### Commit Detail

| Key | Action |
|-----|--------|
| `d` | View diff in external pager |

#### Repo View (Issues tab)

| Key | Action |
|-----|--------|
| `x` | Close issue |
| `C` | Comment (opens `$EDITOR`) |

#### Repo View (tab shortcuts)

| Key | Action |
|-----|--------|
| `p` | Pull Requests tab |
| `i` | Issues tab |
| `c` | Commits tab |
| `a` | Actions tab |

### External Pager

grit detects your preferred pager in this order:

1. `GIT_PAGER` environment variable
2. `git config core.pager`
3. `PAGER` environment variable
4. `less` (fallback)

This works with diff-aware pagers like [delta](https://github.com/dandavison/delta) and [bat](https://github.com/sharkdp/bat).

## Architecture

```
src/
├── main.rs            # Entry point, event loop, TUI suspend/resume
├── app.rs             # Application state machine and event handling
├── action.rs          # Action enum for state transitions
├── event.rs           # Event types (key, tick, render)
├── tui.rs             # Terminal setup, event stream, cleanup
├── github.rs          # GitHub API client (octocrab + reqwest)
├── auth.rs            # Token loading chain and OAuth device flow
├── cache.rs           # XDG-compatible disk cache
├── pager.rs           # External pager detection and invocation
├── types.rs           # Domain models (repos, PRs, issues, commits, etc.)
├── error.rs           # Error types
└── ui/
    ├── mod.rs           # Main UI router, status bar, search bar
    ├── home.rs          # Home dashboard
    ├── repo_list.rs     # Repository list
    ├── repo_view.rs     # Repository tabs view
    ├── pr_detail.rs     # Pull request detail with search highlighting
    ├── commit_detail.rs # Commit detail with diff and search highlighting
    └── popup.rs         # Modal overlays (confirm, select)
```

### Event-driven architecture

```
Input → EventHandler → app.handle_event() → Action → app.update() → render
```

All data loading is async via tokio, with an mpsc channel for dispatching actions. A generation counter (`load_id`) prevents stale async responses from corrupting state when the user navigates away before a response arrives.

### Caching

Data is cached to `~/.cache/grit/` as JSON. On navigation, cached data is served immediately for instant rendering, then a background API call refreshes the data in place without resetting scroll position.

## Dependencies

- [ratatui](https://github.com/ratatui-org/ratatui) + [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal UI
- [octocrab](https://github.com/XAMPPRocky/octocrab) + [reqwest](https://github.com/seanmonstar/reqwest) - GitHub API
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime
- [open](https://github.com/Byron/open-rs) - Open URLs in browser
- [arboard](https://github.com/1Password/arboard) - Clipboard access
- [dirs](https://github.com/dirs-dev/dirs-rs) - XDG directory resolution

## License

MIT License - see [LICENSE](LICENSE) for details.
