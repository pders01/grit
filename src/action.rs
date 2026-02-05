use crate::error::GritError;
use crate::types::{ActionRun, Commit, Issue, MyPr, PrSummary, PullRequest, Repository, ReviewRequest};

/// Tab selection for repo view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RepoTab {
    #[default]
    PullRequests,
    Issues,
    Commits,
    Actions,
}

#[derive(Debug, Clone)]
pub enum Action {
    Quit,
    Back,
    ScrollUp,
    ScrollDown,
    Select,

    // Home screen
    LoadHome,
    HomeLoaded {
        review_requests: Vec<ReviewRequest>,
        my_prs: Vec<MyPr>,
    },
    SwitchHomeSection, // Tab to switch between review requests and my PRs

    // Navigation
    SwitchRepoTab(RepoTab),

    // Repo list
    LoadRepos,
    ReposLoaded(Vec<Repository>),

    // PR operations
    PrsLoaded(Vec<PrSummary>),
    PrDetailLoaded(Box<PullRequest>),

    // Issues
    IssuesLoaded(Vec<Issue>),

    // Commits
    CommitsLoaded(Vec<Commit>),

    // Actions (workflow runs)
    ActionRunsLoaded(Vec<ActionRun>),

    Error(String),
    None,
}

impl From<GritError> for Action {
    fn from(err: GritError) -> Self {
        Action::Error(err.to_string())
    }
}
