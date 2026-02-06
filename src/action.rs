use std::sync::Arc;

use crate::error::GritError;
use crate::forge::Forge;
use crate::types::{
    ActionRun, Commit, CommitDetail, Issue, MergeMethod, MyPr, PrSummary, PullRequest, Repository,
    ReviewEvent, ReviewRequest,
};

/// Tab selection for repo view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepoTab {
    #[default]
    PullRequests,
    Issues,
    Commits,
    Actions,
}

/// What to confirm
#[derive(Debug, Clone)]
pub enum ConfirmAction {
    ClosePr(u64),
    MergePr { number: u64, method: MergeMethod },
    CloseIssue(u64),
}

/// Context for editor suspend
#[derive(Debug, Clone)]
pub enum EditorContext {
    CommentOnPr {
        owner: String,
        repo: String,
        number: u64,
    },
    CommentOnIssue {
        owner: String,
        repo: String,
        number: u64,
    },
    ReviewPr {
        owner: String,
        repo: String,
        number: u64,
        event: ReviewEvent,
    },
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Action {
    Quit,
    Back,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    GoToTop,
    GoToBottom,
    Select,
    NextTab,
    PrevTab,

    // Home screen
    LoadHome,
    HomeLoaded {
        review_requests: Vec<ReviewRequest>,
        my_prs: Vec<MyPr>,
        load_id: u64,
    },

    // Navigation
    SwitchRepoTab(RepoTab),

    // Repo list
    ReposLoaded(Vec<Repository>, u64),

    // PR operations
    PrsLoaded(Vec<PrSummary>, u64),
    PrDetailLoaded(Box<PullRequest>, u64),

    // Issues
    IssuesLoaded(Vec<Issue>, u64),

    // Commits
    CommitsLoaded(Vec<Commit>, u64),
    CommitDetailLoaded(Box<CommitDetail>, u64),

    // Actions (workflow runs)
    ActionRunsLoaded(Vec<ActionRun>, u64),

    // Pagination: append next page to existing list
    ReposAppended(Vec<Repository>, u64),
    PrsAppended(Vec<PrSummary>, u64),
    IssuesAppended(Vec<Issue>, u64),
    CommitsAppended(Vec<Commit>, u64),
    ActionRunsAppended(Vec<ActionRun>, u64),

    // Search
    EnterSearchMode,
    ExitSearchMode,
    SearchInput(char),
    SearchBackspace,
    SearchConfirm,
    SearchNext,
    SearchPrev,
    ClearSearch,

    // Pager
    ViewDiff,
    SuspendForPager(String),

    // Polish
    Refresh,
    OpenInBrowser,
    YankUrl,

    // Mutations - PR
    ShowMergeMethodSelect,
    ShowConfirm(ConfirmAction),
    ConfirmYes,
    ConfirmNo,
    PrMerged,
    PrClosed,
    CommentPosted,

    // Mutations - Issue
    IssueClosed,

    // Review
    ShowReviewSelect,
    ReviewSubmitted,

    // Editor
    SuspendForEditor(EditorContext),

    // Popup navigation
    PopupUp,
    PopupDown,
    PopupSelect,

    // Forge switching
    ShowForgeSelect,
    SwitchForge(usize),
    ForgeReady(Arc<dyn Forge>, String),

    Error(String),
    None,
}

impl From<GritError> for Action {
    fn from(err: GritError) -> Self {
        Action::Error(err.to_string())
    }
}
