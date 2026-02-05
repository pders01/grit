# grit

A terminal user interface (TUI) for browsing GitHub repositories, pull requests, issues, commits, and actions.

![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## Features

- **Home Dashboard** - View PRs requiring your review and your open PRs with CI status
- **Repository Browser** - Browse your GitHub repositories sorted by recent activity
- **Pull Requests** - View open PRs with author, status, and metadata
- **Issues** - Browse open issues with labels and comments
- **Commits** - View commit history with full diff display
- **Actions** - Monitor GitHub Actions workflow runs
- **Vim Keybindings** - Navigate with familiar vim motions

## Installation

### From source

```bash
git clone https://github.com/yourusername/grit.git
cd grit
cargo install --path .
```

### Pre-built binaries

Download from the [Releases](https://github.com/yourusername/grit/releases) page.

## Usage

Set your GitHub token:

```bash
export GITHUB_TOKEN=ghp_your_token_here
```

Run grit:

```bash
grit
```

### Keybindings

#### Global

| Key | Action |
|-----|--------|
| `q` / `Esc` | Back / Quit |
| `j` / `Down` | Move down |
| `k` / `Up` | Move up |
| `g` / `Home` | Go to top |
| `G` / `End` | Go to bottom |
| `Ctrl+d` / `Ctrl+f` / `PageDown` | Page down |
| `Ctrl+u` / `Ctrl+b` / `PageUp` | Page up |
| `Enter` | Select / Open |

#### Home Screen

| Key | Action |
|-----|--------|
| `h` / `l` / `Tab` | Switch sections |
| `r` | Open repository list |

#### Repository View

| Key | Action |
|-----|--------|
| `h` / `l` / `Tab` | Switch tabs |
| `p` | Pull Requests tab |
| `i` | Issues tab |
| `c` | Commits tab |
| `a` | Actions tab |

## Configuration

grit requires a GitHub personal access token with the following scopes:

- `repo` - Full control of private repositories
- `read:org` - Read org membership (for organization repos)

Create a token at [GitHub Settings > Developer settings > Personal access tokens](https://github.com/settings/tokens).

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

## Architecture

```
src/
├── main.rs          # Entry point and event loop
├── app.rs           # Application state and event handling
├── action.rs        # Action enum for state transitions
├── event.rs         # Event types (key, tick, render)
├── tui.rs           # Terminal setup and event stream
├── github.rs        # GitHub API client (octocrab)
├── types.rs         # Data types for GitHub entities
├── error.rs         # Error types
└── ui/
    ├── mod.rs           # Main UI router
    ├── home.rs          # Home dashboard
    ├── repo_list.rs     # Repository list
    ├── repo_view.rs     # Repository tabs view
    ├── pr_detail.rs     # Pull request detail
    └── commit_detail.rs # Commit detail with diff
```

## Dependencies

- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation
- [octocrab](https://github.com/XAMPPRocky/octocrab) - GitHub API client
- [tokio](https://github.com/tokio-rs/tokio) - Async runtime

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.
