# grit: Git Remote Interface TUI

## Overview

**grit** is a terminal user interface for managing git remotes across multiple forge providers (GitHub, GitLab, Bitbucket, Gitea, etc.). It provides a unified interface for common remote operations: browsing repositories, managing pull/merge requests, viewing issues, and handling code review workflows—all without leaving the terminal.

## Goals

1. **Unified experience** across forges — same keybindings, same mental model
2. **Fast** — instant startup, async operations, responsive UI even on slow connections
3. **Keyboard-driven** — vim-inspired navigation, minimal mouse interaction
4. **Context-aware** — automatically detect current repo and relevant remotes
5. **Offline-friendly** — cache aggressively, degrade gracefully

## Non-Goals

- Replacing git CLI for local operations (use `gitui` or `lazygit` for that)
- Full feature parity with web UIs — focus on the 80% of workflows
- Being a notification center or inbox

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                           TUI Layer                             │
│                         (ratatui + crossterm)                   │
├─────────────────────────────────────────────────────────────────┤
│                        Application State                        │
│                    (screens, navigation, cache)                 │
├─────────────────────────────────────────────────────────────────┤
│                        Forge Abstraction                        │
│                      trait Forge { ... }                        │
├──────────┬──────────┬──────────┬──────────┬─────────────────────┤
│  GitHub  │  GitLab  │ Bitbucket│  Gitea   │  ... (pluggable)    │
│  Adapter │  Adapter │  Adapter │  Adapter │                     │
├──────────┴──────────┴──────────┴──────────┴─────────────────────┤
│                        HTTP Client Layer                        │
│                    (reqwest + tower middleware)                 │
├─────────────────────────────────────────────────────────────────┤
│                     Local Git Integration                       │
│                          (git2-rs)                              │
└─────────────────────────────────────────────────────────────────┘
```

## Core Abstractions

### Forge Trait

The central abstraction that all providers implement:

```rust
#[async_trait]
pub trait Forge: Send + Sync {
    /// Forge identity
    fn name(&self) -> &str;
    fn base_url(&self) -> &Url;
    
    /// Authentication
    async fn authenticate(&mut self, config: &AuthConfig) -> Result<()>;
    fn is_authenticated(&self) -> bool;
    
    /// Repository operations
    async fn get_repo(&self, owner: &str, name: &str) -> Result<Repository>;
    async fn list_repos(&self, filter: RepoFilter) -> Result<Vec<RepoSummary>>;
    async fn fork_repo(&self, owner: &str, name: &str) -> Result<Repository>;
    
    /// Pull/Merge Request operations
    async fn list_prs(&self, repo: &RepoId, filter: PrFilter) -> Result<Vec<PrSummary>>;
    async fn get_pr(&self, repo: &RepoId, number: u64) -> Result<PullRequest>;
    async fn create_pr(&self, repo: &RepoId, params: CreatePrParams) -> Result<PullRequest>;
    async fn merge_pr(&self, repo: &RepoId, number: u64, method: MergeMethod) -> Result<()>;
    async fn close_pr(&self, repo: &RepoId, number: u64) -> Result<()>;
    async fn list_pr_comments(&self, repo: &RepoId, number: u64) -> Result<Vec<Comment>>;
    async fn add_pr_comment(&self, repo: &RepoId, number: u64, body: &str) -> Result<Comment>;
    
    /// Review operations
    async fn list_reviews(&self, repo: &RepoId, pr: u64) -> Result<Vec<Review>>;
    async fn submit_review(&self, repo: &RepoId, pr: u64, review: SubmitReview) -> Result<()>;
    
    /// Issue operations
    async fn list_issues(&self, repo: &RepoId, filter: IssueFilter) -> Result<Vec<IssueSummary>>;
    async fn get_issue(&self, repo: &RepoId, number: u64) -> Result<Issue>;
    async fn create_issue(&self, repo: &RepoId, params: CreateIssueParams) -> Result<Issue>;
    async fn close_issue(&self, repo: &RepoId, number: u64) -> Result<()>;
    
    /// CI/CD status
    async fn get_ci_status(&self, repo: &RepoId, ref_: &str) -> Result<CiStatus>;
    async fn list_workflow_runs(&self, repo: &RepoId, filter: WorkflowFilter) -> Result<Vec<WorkflowRun>>;
    
    /// User operations
    async fn get_current_user(&self) -> Result<User>;
    async fn get_user(&self, username: &str) -> Result<User>;
}
```

### Unified Domain Types

Normalized types that abstract over forge-specific representations:

```rust
pub struct Repository {
    pub id: RepoId,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub visibility: Visibility,
    pub fork_of: Option<Box<RepoSummary>>,
    pub topics: Vec<String>,
    pub language: Option<String>,
    pub stats: RepoStats,
    pub permissions: RepoPermissions,
    pub urls: RepoUrls,
    pub timestamps: Timestamps,
}

pub struct PullRequest {
    pub id: PrId,
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: PrState,
    pub author: User,
    pub head: BranchRef,
    pub base: BranchRef,
    pub labels: Vec<Label>,
    pub assignees: Vec<User>,
    pub reviewers: Vec<Reviewer>,
    pub ci_status: Option<CiStatus>,
    pub mergeable: Option<MergeableState>,
    pub diff_stats: DiffStats,
    pub timestamps: Timestamps,
}

pub struct Issue {
    pub id: IssueId,
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: IssueState,
    pub author: User,
    pub labels: Vec<Label>,
    pub assignees: Vec<User>,
    pub milestone: Option<Milestone>,
    pub timestamps: Timestamps,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PrState {
    Open,
    Closed,
    Merged,
    Draft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}
```

## Application Structure

### Screen Hierarchy

```
Root
├── Home (dashboard: recent repos, assigned PRs, mentions)
├── Repos
│   ├── RepoList (all repos, filterable)
│   └── RepoDetail
│       ├── Overview (README, stats)
│       ├── PRs
│       │   ├── PrList
│       │   └── PrDetail
│       │       ├── Conversation
│       │       ├── Commits
│       │       ├── Files (diff viewer)
│       │       └── Checks (CI status)
│       ├── Issues
│       │   ├── IssueList
│       │   └── IssueDetail
│       ├── Actions/Pipelines
│       └── Branches
├── Search (cross-repo search)
└── Settings
    ├── Accounts (manage forge connections)
    └── Preferences
```

### State Management

Using a message-passing architecture inspired by Elm:

```rust
pub struct App {
    state: AppState,
    forge_manager: ForgeManager,
    cache: Cache,
    task_tx: mpsc::Sender<Task>,
}

pub enum Message {
    // Navigation
    Navigate(Screen),
    Back,
    
    // Data loading
    LoadRepos(RepoFilter),
    ReposLoaded(Result<Vec<RepoSummary>>),
    LoadPr(RepoId, u64),
    PrLoaded(Result<PullRequest>),
    
    // Actions
    CreatePr(RepoId, CreatePrParams),
    PrCreated(Result<PullRequest>),
    MergePr(RepoId, u64, MergeMethod),
    PrMerged(Result<()>),
    
    // UI state
    SetFilter(String),
    SelectItem(usize),
    TogglePreview,
    
    // System
    Tick,
    Resize(u16, u16),
    Error(AppError),
}

impl App {
    pub fn update(&mut self, msg: Message) -> Vec<Command> {
        match msg {
            Message::Navigate(screen) => {
                self.state.push_screen(screen);
                vec![self.load_screen_data(&screen)]
            }
            Message::LoadPr(repo, number) => {
                self.state.set_loading(true);
                vec![Command::LoadPr(repo, number)]
            }
            Message::PrLoaded(Ok(pr)) => {
                self.state.set_loading(false);
                self.cache.insert_pr(&pr);
                self.state.set_current_pr(pr);
                vec![]
            }
            // ...
        }
    }
}
```

### Async Task Handling

Background tasks for API calls, keeping the UI responsive:

```rust
pub enum Task {
    FetchRepos { filter: RepoFilter },
    FetchPr { repo: RepoId, number: u64 },
    FetchDiff { repo: RepoId, pr: u64 },
    SubmitReview { repo: RepoId, pr: u64, review: SubmitReview },
    // ...
}

pub struct TaskRunner {
    rx: mpsc::Receiver<Task>,
    msg_tx: mpsc::Sender<Message>,
    forge_manager: Arc<ForgeManager>,
}

impl TaskRunner {
    pub async fn run(mut self) {
        while let Some(task) = self.rx.recv().await {
            let forge_manager = Arc::clone(&self.forge_manager);
            let msg_tx = self.msg_tx.clone();
            
            tokio::spawn(async move {
                let msg = match task {
                    Task::FetchPr { repo, number } => {
                        let result = forge_manager
                            .get_forge(&repo.forge)
                            .and_then(|f| f.get_pr(&repo, number).await);
                        Message::PrLoaded(result)
                    }
                    // ...
                };
                let _ = msg_tx.send(msg).await;
            });
        }
    }
}
```

## UI Components

### Key Components

```rust
// Reusable list with fuzzy filtering
pub struct FilterableList<T> {
    items: Vec<T>,
    filtered_indices: Vec<usize>,
    selected: usize,
    filter: String,
    matcher: SkimMatcherV2,
}

// PR/Issue summary row
pub struct ItemRow {
    number: u64,
    title: String,
    state: ItemState,
    author: String,
    labels: Vec<Label>,
    updated: DateTime<Utc>,
    ci_status: Option<CiStatus>,
}

// Diff viewer with syntax highlighting
pub struct DiffViewer {
    hunks: Vec<Hunk>,
    scroll: usize,
    syntax_set: SyntaxSet,
    theme: Theme,
    side_by_side: bool,
}

// Markdown renderer for PR/issue bodies
pub struct MarkdownView {
    content: String,
    rendered: Vec<RenderedBlock>,
    scroll: usize,
    link_positions: Vec<LinkPosition>,
}
```

### Layout Example (PR Detail Screen)

```
┌─ grit ─────────────────────────────────────────────────────────────┐
│ owner/repo > Pull Requests > #1234                          [?]    │
├────────────────────────────────────────────────────────────────────┤
│ ● #1234: Fix quadratic complexity in booking availability          │
│   paul → main │ +127 -43 │ 3 commits │ 2 hours ago                 │
│   Labels: [bug] [performance]                                      │
│   Reviewers: ◐ alice (requested) │ ✓ bob (approved)                │
│   CI: ● 3/4 passed │ ○ 1 running                                   │
├────────────────────────────────────────────────────────────────────┤
│ [Conversation] [Commits] [Files (4)] [Checks]                      │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  ## Summary                                                        │
│                                                                    │
│  This PR addresses the O(n²) issue in `get_availability` by        │
│  replacing the nested loop with a hash-based lookup.               │
│                                                                    │
│  ## Changes                                                        │
│  - Refactored `Booking::overlaps` to use interval tree             │
│  - Added benchmark suite for availability checks                   │
│  - Updated tests                                                   │
│                                                                    │
│  ────────────────────────────────────────────────────────────────  │
│  💬 alice commented 1 hour ago:                                    │
│  > Could we add a test case for the edge case where...             │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ [m]erge  [c]omment  [r]eview  [a]pprove  [e]dit  [q]uit            │
└────────────────────────────────────────────────────────────────────┘
```

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `q` / `Esc` | Back / Quit |
| `?` | Help |
| `j` / `k` / `↓` / `↑` | Navigate down / up |
| `g` / `G` | Go to top / bottom |
| `Ctrl-d` / `Ctrl-u` | Page down / up |
| `/` | Filter / Search |
| `Enter` | Select / Open |
| `Tab` | Switch pane / tab |
| `r` | Refresh |
| `y` | Yank (copy URL/number) |
| `o` | Open in browser |

### Context-Specific

| Context | Key | Action |
|---------|-----|--------|
| PR List | `n` | New PR |
| PR List | `f` | Filter (open/closed/mine/review) |
| PR Detail | `m` | Merge |
| PR Detail | `c` | Comment |
| PR Detail | `a` | Approve |
| PR Detail | `x` | Request changes |
| Diff View | `]` / `[` | Next / prev file |
| Diff View | `s` | Toggle side-by-side |

## Configuration

### File Location

Following XDG spec: `~/.config/grit/config.toml`

### Config Schema

```toml
[general]
default_forge = "github"  # when not in a repo
editor = "nvim"           # for composing comments
browser = "firefox"       # for 'open in browser'

[ui]
theme = "dark"            # dark, light, or custom
diff_style = "unified"    # unified or side-by-side
show_ci_status = true
date_format = "relative"  # relative, iso, local

[keybindings]
# Override defaults
"g g" = "goto_top"
"Ctrl-c" = "quit"

[aliases]
prs = "repos.current.prs --filter=open"
mine = "repos.current.prs --filter=author:@me"
review = "repos.current.prs --filter=reviewer:@me"

# Forge configurations
[[forges]]
name = "github"
type = "github"
host = "github.com"           # default
auth = "oauth"                # oauth, token, or gh-cli

[[forges]]
name = "work-gitlab"
type = "gitlab"
host = "gitlab.company.com"
auth = "token"

[[forges]]
name = "codeberg"
type = "gitea"
host = "codeberg.org"
auth = "token"
```

### Authentication Storage

Credentials stored in system keyring via `keyring-rs`, falling back to encrypted file at `~/.config/grit/credentials.enc`.

Support for:
- OAuth device flow (GitHub, GitLab)
- Personal access tokens
- Delegation to existing CLIs (`gh auth token`, `glab auth token`)

## Caching Strategy

### Cache Layers

1. **Memory cache**: Hot data for current session (current repo, recently viewed PRs)
2. **Disk cache**: Persistent cache at `~/.cache/grit/`

### Cache Invalidation

- TTL-based: Different TTLs for different data types
  - Repo metadata: 1 hour
  - PR list: 5 minutes
  - PR detail: 2 minutes
  - CI status: 30 seconds
- Event-based: Invalidate on mutation (after creating/merging PR)
- Manual: `r` to refresh current view

### Offline Mode

When network unavailable:
- Show cached data with staleness indicator
- Queue mutations for later (with user confirmation)
- Graceful degradation — never crash

## Dependencies

```toml
[dependencies]
# TUI
ratatui = "0.28"
crossterm = "0.28"

# Async runtime
tokio = { version = "1", features = ["full"] }

# HTTP
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tower = { version = "0.4", features = ["retry", "timeout", "limit"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Git
git2 = "0.19"

# Auth
keyring = "2"
oauth2 = "4"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
url = "2"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
fuzzy-matcher = "0.3"
syntect = "5"             # syntax highlighting for diffs
pulldown-cmark = "0.11"   # markdown parsing

# Forge-specific clients (optional, for bootstrapping)
octocrab = "0.39"         # GitHub (may replace with custom impl)
```

## Project Structure

```
grit/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── app.rs              # Application state & message loop
│   ├── config.rs           # Configuration loading
│   ├── forge/
│   │   ├── mod.rs          # Forge trait definition
│   │   ├── types.rs        # Unified domain types
│   │   ├── github.rs       # GitHub adapter
│   │   ├── gitlab.rs       # GitLab adapter
│   │   ├── gitea.rs        # Gitea/Forgejo adapter
│   │   └── bitbucket.rs    # Bitbucket adapter
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── screens/
│   │   │   ├── home.rs
│   │   │   ├── repo_list.rs
│   │   │   ├── pr_list.rs
│   │   │   ├── pr_detail.rs
│   │   │   ├── diff_view.rs
│   │   │   └── ...
│   │   ├── components/
│   │   │   ├── list.rs
│   │   │   ├── tabs.rs
│   │   │   ├── markdown.rs
│   │   │   ├── diff.rs
│   │   │   └── ...
│   │   ├── theme.rs
│   │   └── keybindings.rs
│   ├── git/
│   │   ├── mod.rs          # Local git operations
│   │   └── remote.rs       # Remote detection
│   ├── cache/
│   │   ├── mod.rs
│   │   ├── memory.rs
│   │   └── disk.rs
│   ├── auth/
│   │   ├── mod.rs
│   │   ├── oauth.rs
│   │   ├── token.rs
│   │   └── keyring.rs
│   └── tasks.rs            # Background task runner
└── tests/
    ├── forge_tests.rs
    └── ui_tests.rs
```

## Implementation Phases

### Phase 1: Foundation (MVP) ✅

- Core app skeleton with ratatui
- GitHub adapter (single forge)
- Basic screens: repo list, PR list, PR detail (read-only)
- Token-based auth
- In-memory cache only

**Deliverable**: Can browse GitHub PRs for repos you have access to.

### Phase 2: Interactivity + Polish ✅

- Merge/close PRs with method selection
- Comment on PRs and issues via `$EDITOR`
- Submit PR reviews (approve, request changes, comment)
- External pager for diffs (delta, less, bat, etc.)
- Search in list views and content views
- OAuth device flow for GitHub
- Disk cache with stale-while-revalidate
- Generation counter for async state consistency
- Open in browser, copy URL, refresh

**Deliverable**: Full PR workflow without leaving terminal.

### Phase 3: Multi-Forge

- GitLab adapter
- Gitea adapter
- Forge auto-detection from git remote
- Config file support

**Deliverable**: Works with multiple forges in the same session.

### Phase 4: Polish

- Keyboard customization
- Themes
- Review submission with inline comments
- Offline mode

**Deliverable**: Production-ready tool.

## Open Questions

1. ~~**Diff viewing**: Build custom renderer or shell out to `delta`/`diff-so-fancy`?~~ **Resolved**: Shell out to the user's configured pager via stdin piping, same as git.
2. **Inline comments**: Worth the complexity for v1?
3. **Notifications**: Polling vs webhooks vs leave to other tools?
4. **Merge conflict resolution**: In scope or defer to git CLI?

## References

- [gitui](https://github.com/extrawurst/gitui) — Rust TUI for local git
- [lazygit](https://github.com/jesseduffield/lazygit) — Go TUI for local git
- [gh-dash](https://github.com/dlvhdr/gh-dash) — Go TUI for GitHub (single forge)
- [ratatui](https://ratatui.rs/) — TUI framework
- [octocrab](https://github.com/XAMPPRocky/octocrab) — GitHub API client
