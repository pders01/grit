use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl MergeMethod {
    pub fn as_api_str(&self) -> &'static str {
        match self {
            MergeMethod::Merge => "merge",
            MergeMethod::Squash => "squash",
            MergeMethod::Rebase => "rebase",
        }
    }
}

impl fmt::Display for MergeMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeMethod::Merge => write!(f, "Merge commit"),
            MergeMethod::Squash => write!(f, "Squash and merge"),
            MergeMethod::Rebase => write!(f, "Rebase and merge"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEvent {
    Approve,
    RequestChanges,
    Comment,
}

impl ReviewEvent {
    pub fn as_api_str(&self) -> &'static str {
        match self {
            ReviewEvent::Approve => "APPROVE",
            ReviewEvent::RequestChanges => "REQUEST_CHANGES",
            ReviewEvent::Comment => "COMMENT",
        }
    }
}

impl fmt::Display for ReviewEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReviewEvent::Approve => write!(f, "Approve"),
            ReviewEvent::RequestChanges => write!(f, "Request changes"),
            ReviewEvent::Comment => write!(f, "Comment"),
        }
    }
}

/// Cached home screen data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeData {
    pub review_requests: Vec<ReviewRequest>,
    pub my_prs: Vec<MyPr>,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    // MergeMethod::as_api_str
    #[test]
    fn merge_method_api_str_merge() {
        assert_eq!(MergeMethod::Merge.as_api_str(), "merge");
    }

    #[test]
    fn merge_method_api_str_squash() {
        assert_eq!(MergeMethod::Squash.as_api_str(), "squash");
    }

    #[test]
    fn merge_method_api_str_rebase() {
        assert_eq!(MergeMethod::Rebase.as_api_str(), "rebase");
    }

    // MergeMethod::Display
    #[test]
    fn merge_method_display_merge() {
        assert_eq!(MergeMethod::Merge.to_string(), "Merge commit");
    }

    #[test]
    fn merge_method_display_squash() {
        assert_eq!(MergeMethod::Squash.to_string(), "Squash and merge");
    }

    #[test]
    fn merge_method_display_rebase() {
        assert_eq!(MergeMethod::Rebase.to_string(), "Rebase and merge");
    }

    // ReviewEvent::as_api_str
    #[test]
    fn review_event_api_str_approve() {
        assert_eq!(ReviewEvent::Approve.as_api_str(), "APPROVE");
    }

    #[test]
    fn review_event_api_str_request_changes() {
        assert_eq!(ReviewEvent::RequestChanges.as_api_str(), "REQUEST_CHANGES");
    }

    #[test]
    fn review_event_api_str_comment() {
        assert_eq!(ReviewEvent::Comment.as_api_str(), "COMMENT");
    }

    // ReviewEvent::Display
    #[test]
    fn review_event_display_approve() {
        assert_eq!(ReviewEvent::Approve.to_string(), "Approve");
    }

    #[test]
    fn review_event_display_request_changes() {
        assert_eq!(ReviewEvent::RequestChanges.to_string(), "Request changes");
    }

    #[test]
    fn review_event_display_comment() {
        assert_eq!(ReviewEvent::Comment.to_string(), "Comment");
    }

    // PrState::Display
    #[test]
    fn pr_state_display() {
        assert_eq!(PrState::Open.to_string(), "Open");
        assert_eq!(PrState::Closed.to_string(), "Closed");
        assert_eq!(PrState::Merged.to_string(), "Merged");
    }

    // IssueState::Display
    #[test]
    fn issue_state_display() {
        assert_eq!(IssueState::Open.to_string(), "Open");
        assert_eq!(IssueState::Closed.to_string(), "Closed");
    }

    // ActionStatus::Display
    #[test]
    fn action_status_display() {
        assert_eq!(ActionStatus::Queued.to_string(), "Queued");
        assert_eq!(ActionStatus::InProgress.to_string(), "Running");
        assert_eq!(ActionStatus::Completed.to_string(), "Done");
    }

    // ChecksStatus::Display
    #[test]
    fn checks_status_display() {
        assert_eq!(ChecksStatus::Pending.to_string(), "⏳");
        assert_eq!(ChecksStatus::Success.to_string(), "✓");
        assert_eq!(ChecksStatus::Failure.to_string(), "✗");
        assert_eq!(ChecksStatus::None.to_string(), "-");
    }
}
