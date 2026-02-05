use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// GitHub Issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: IssueState,
    pub author: String,
    pub labels: Vec<String>,
    pub comments: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueState {
    Open,
    Closed,
}

impl std::fmt::Display for IssueState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueState::Open => write!(f, "Open"),
            IssueState::Closed => write!(f, "Closed"),
        }
    }
}

/// Git Commit (summary for list view)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: DateTime<Utc>,
}

/// Git Commit (full detail)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitDetail {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: DateTime<Utc>,
    pub stats: CommitStats,
    pub files: Vec<CommitFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitStats {
    pub additions: u64,
    pub deletions: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitFile {
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub patch: Option<String>,
}

/// GitHub Actions workflow run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRun {
    pub id: u64,
    pub name: String,
    pub status: ActionStatus,
    pub conclusion: Option<ActionConclusion>,
    pub branch: String,
    pub event: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionStatus {
    Queued,
    InProgress,
    Completed,
}

impl std::fmt::Display for ActionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionStatus::Queued => write!(f, "Queued"),
            ActionStatus::InProgress => write!(f, "Running"),
            ActionStatus::Completed => write!(f, "Done"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionConclusion {
    Success,
    Failure,
    Cancelled,
    Skipped,
    TimedOut,
}

impl std::fmt::Display for ActionConclusion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionConclusion::Success => write!(f, "✓"),
            ActionConclusion::Failure => write!(f, "✗"),
            ActionConclusion::Cancelled => write!(f, "⊘"),
            ActionConclusion::Skipped => write!(f, "⊘"),
            ActionConclusion::TimedOut => write!(f, "⏱"),
        }
    }
}

/// Review request - a PR where the current user is requested as reviewer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub repo_owner: String,
    pub repo_name: String,
    pub pr_number: u64,
    pub pr_title: String,
    pub author: String,
    pub updated_at: DateTime<Utc>,
}

/// Your open PR with CI status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyPr {
    pub repo_owner: String,
    pub repo_name: String,
    pub number: u64,
    pub title: String,
    pub state: PrState,
    pub checks_status: ChecksStatus,
    pub updated_at: DateTime<Utc>,
}

/// CI/CD checks status for a PR
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChecksStatus {
    Pending,
    Success,
    Failure,
    None,
}

impl std::fmt::Display for ChecksStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChecksStatus::Pending => write!(f, "⏳"),
            ChecksStatus::Success => write!(f, "✓"),
            ChecksStatus::Failure => write!(f, "✗"),
            ChecksStatus::None => write!(f, "-"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
    pub stars: u32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

impl std::fmt::Display for PrState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrState::Open => write!(f, "Open"),
            PrState::Closed => write!(f, "Closed"),
            PrState::Merged => write!(f, "Merged"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrSummary {
    pub number: u64,
    pub title: String,
    pub state: PrState,
    pub author: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrStats {
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub commits: u64,
    pub comments: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: PrState,
    pub author: String,
    pub head_branch: String,
    pub base_branch: String,
    pub stats: PrStats,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub merged_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
}
