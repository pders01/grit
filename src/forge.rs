use async_trait::async_trait;

use crate::error::{GritError, Result};
use crate::types::{
    ActionRun, ChecksStatus, Commit, CommitDetail, Issue, MyPr, PagedResult, PrSummary,
    PullRequest, Repository, ReviewRequest,
};

#[async_trait]
#[allow(dead_code)]
pub trait Forge: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn web_url(&self, owner: &str, repo: &str, kind: &str, id: &str) -> String;

    // Core (required)
    async fn get_current_user(&self) -> Result<String>;
    async fn list_repos(&self, page: u32) -> Result<PagedResult<Repository>>;
    async fn list_prs(&self, owner: &str, repo: &str, page: u32) -> Result<PagedResult<PrSummary>>;
    async fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest>;
    async fn list_issues(&self, owner: &str, repo: &str, page: u32) -> Result<PagedResult<Issue>>;
    async fn list_commits(&self, owner: &str, repo: &str, page: u32)
        -> Result<PagedResult<Commit>>;
    async fn get_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<CommitDetail>;
    async fn get_pr_diff(&self, owner: &str, repo: &str, number: u64) -> Result<String>;
    async fn merge_pr(&self, owner: &str, repo: &str, number: u64, method: &str) -> Result<()>;
    async fn close_pr(&self, owner: &str, repo: &str, number: u64) -> Result<()>;
    async fn close_issue(&self, owner: &str, repo: &str, number: u64) -> Result<()>;
    async fn comment(&self, owner: &str, repo: &str, number: u64, body: &str) -> Result<()>;

    // Optional (default impls for forge-specific features)
    async fn list_review_requests(&self, _username: &str) -> Result<Vec<ReviewRequest>> {
        Ok(vec![])
    }
    async fn list_my_prs(&self, _username: &str) -> Result<Vec<MyPr>> {
        Ok(vec![])
    }
    async fn list_action_runs(
        &self,
        _owner: &str,
        _repo: &str,
        _page: u32,
    ) -> Result<PagedResult<ActionRun>> {
        Ok(PagedResult {
            items: vec![],
            total_count: None,
        })
    }
    async fn get_check_status(
        &self,
        _owner: &str,
        _repo: &str,
        _pr_number: u64,
    ) -> Result<ChecksStatus> {
        Ok(ChecksStatus::None)
    }
    async fn submit_review(
        &self,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _event: &str,
        _body: &str,
    ) -> Result<()> {
        Err(GritError::Api("Reviews not supported by this forge".into()))
    }
}
