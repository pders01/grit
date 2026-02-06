use async_trait::async_trait;
use octocrab::models::IssueState as OctoIssueState;
use octocrab::Octocrab;

use crate::error::{GritError, Result};
use crate::forge::Forge;
use crate::types::{
    ActionConclusion, ActionRun, ActionStatus, ChecksStatus, Commit, CommitDetail, CommitFile,
    CommitStats, Issue, IssueState, MyPr, PagedResult, PrState, PrStats, PrSummary, PullRequest,
    Repository, ReviewRequest,
};

pub struct GitHub {
    client: Octocrab,
    token: String,
}

impl std::fmt::Debug for GitHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitHub").finish_non_exhaustive()
    }
}

impl From<octocrab::Error> for GritError {
    fn from(err: octocrab::Error) -> Self {
        GritError::Api(err.to_string())
    }
}

impl GitHub {
    pub fn new(token: String) -> Result<Self> {
        let client = Octocrab::builder()
            .personal_token(token.clone())
            .build()
            .map_err(|e| GritError::Auth(e.to_string()))?;

        Ok(Self { client, token })
    }
}

#[async_trait]
impl Forge for GitHub {
    fn name(&self) -> &str {
        "GitHub"
    }

    fn web_url(&self, owner: &str, repo: &str, kind: &str, id: &str) -> String {
        match kind {
            "repo" => format!("https://github.com/{}/{}", owner, repo),
            "pr" => format!("https://github.com/{}/{}/pull/{}", owner, repo, id),
            "issue" => format!("https://github.com/{}/{}/issues/{}", owner, repo, id),
            "commit" => format!("https://github.com/{}/{}/commit/{}", owner, repo, id),
            "action_run" => {
                format!("https://github.com/{}/{}/actions/runs/{}", owner, repo, id)
            }
            _ => format!("https://github.com/{}/{}", owner, repo),
        }
    }

    async fn get_current_user(&self) -> Result<String> {
        let user = self.client.current().user().await?;
        Ok(user.login)
    }

    async fn list_repos(&self, page: u32) -> Result<PagedResult<Repository>> {
        let repos = self
            .client
            .current()
            .list_repos_for_authenticated_user()
            .sort("updated")
            .direction("desc")
            .per_page(50)
            .page(page as u8)
            .send()
            .await?;

        let total = repos
            .total_count
            .or_else(|| repos.number_of_pages().map(|n| n as u64 * 50));

        let repositories = repos
            .items
            .into_iter()
            .map(|repo| Repository {
                owner: repo
                    .owner
                    .map(|o| o.login)
                    .unwrap_or_else(|| "unknown".to_string()),
                name: repo.name,
                description: repo.description,
                url: repo.html_url.map(|u| u.to_string()).unwrap_or_default(),
                stars: repo.stargazers_count.unwrap_or(0),
                updated_at: repo.updated_at.unwrap_or_else(chrono::Utc::now),
            })
            .collect();

        Ok(PagedResult {
            items: repositories,
            total_count: total,
        })
    }

    async fn list_prs(&self, owner: &str, repo: &str, page: u32) -> Result<PagedResult<PrSummary>> {
        let prs = self
            .client
            .pulls(owner, repo)
            .list()
            .state(octocrab::params::State::Open)
            .sort(octocrab::params::pulls::Sort::Updated)
            .direction(octocrab::params::Direction::Descending)
            .per_page(50)
            .page(page)
            .send()
            .await?;

        let total = prs
            .total_count
            .or_else(|| prs.number_of_pages().map(|n| n as u64 * 50));

        let summaries = prs
            .items
            .into_iter()
            .map(|pr| PrSummary {
                number: pr.number,
                title: pr.title.unwrap_or_default(),
                state: match pr.merged_at {
                    Some(_) => PrState::Merged,
                    None => match pr.state {
                        Some(OctoIssueState::Closed) => PrState::Closed,
                        _ => PrState::Open,
                    },
                },
                author: pr
                    .user
                    .map(|u| u.login)
                    .unwrap_or_else(|| "unknown".to_string()),
                updated_at: pr.updated_at.unwrap_or_else(chrono::Utc::now),
            })
            .collect();

        Ok(PagedResult {
            items: summaries,
            total_count: total,
        })
    }

    async fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
        let pr = self.client.pulls(owner, repo).get(number).await?;

        let state = match pr.merged_at {
            Some(_) => PrState::Merged,
            None => match pr.state {
                Some(OctoIssueState::Closed) => PrState::Closed,
                _ => PrState::Open,
            },
        };

        Ok(PullRequest {
            number: pr.number,
            title: pr.title.unwrap_or_default(),
            body: pr.body,
            state,
            author: pr
                .user
                .map(|u| u.login)
                .unwrap_or_else(|| "unknown".to_string()),
            head_branch: pr.head.ref_field,
            base_branch: pr.base.ref_field,
            stats: PrStats {
                additions: pr.additions.unwrap_or(0),
                deletions: pr.deletions.unwrap_or(0),
                changed_files: pr.changed_files.unwrap_or(0),
                commits: pr.commits.unwrap_or(0),
                comments: pr.comments.unwrap_or(0),
            },
            created_at: pr.created_at.unwrap_or_else(chrono::Utc::now),
            updated_at: pr.updated_at.unwrap_or_else(chrono::Utc::now),
            merged_at: pr.merged_at,
            closed_at: pr.closed_at,
        })
    }

    async fn list_issues(&self, owner: &str, repo: &str, page: u32) -> Result<PagedResult<Issue>> {
        let issues = self
            .client
            .issues(owner, repo)
            .list()
            .state(octocrab::params::State::Open)
            .sort(octocrab::params::issues::Sort::Updated)
            .direction(octocrab::params::Direction::Descending)
            .per_page(50)
            .page(page)
            .send()
            .await?;

        let total = issues
            .total_count
            .or_else(|| issues.number_of_pages().map(|n| n as u64 * 50));

        let result = issues
            .items
            .into_iter()
            .filter(|i| i.pull_request.is_none()) // Filter out PRs
            .map(|issue| Issue {
                number: issue.number,
                title: issue.title,
                state: match issue.state {
                    OctoIssueState::Closed => IssueState::Closed,
                    _ => IssueState::Open,
                },
                author: issue.user.login,
                labels: issue.labels.into_iter().map(|l| l.name).collect(),
                comments: issue.comments,
                created_at: issue.created_at,
                updated_at: issue.updated_at,
            })
            .collect();

        Ok(PagedResult {
            items: result,
            total_count: total,
        })
    }

    async fn list_commits(
        &self,
        owner: &str,
        repo: &str,
        page: u32,
    ) -> Result<PagedResult<Commit>> {
        let commits = self
            .client
            .repos(owner, repo)
            .list_commits()
            .per_page(50)
            .page(page)
            .send()
            .await?;

        let total = commits
            .total_count
            .or_else(|| commits.number_of_pages().map(|n| n as u64 * 50));

        let result = commits
            .items
            .into_iter()
            .map(|c| {
                let message = c.commit.message.lines().next().unwrap_or("").to_string();
                let author = c
                    .author
                    .map(|a| a.login)
                    .or_else(|| c.commit.author.as_ref().map(|a| a.name.clone()))
                    .unwrap_or_else(|| "unknown".to_string());
                let date = c
                    .commit
                    .author
                    .and_then(|a| a.date)
                    .unwrap_or_else(chrono::Utc::now);

                Commit {
                    sha: c.sha,
                    message,
                    author,
                    date,
                }
            })
            .collect();

        Ok(PagedResult {
            items: result,
            total_count: total,
        })
    }

    async fn get_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<CommitDetail> {
        let url = format!("/repos/{}/{}/commits/{}", owner, repo, sha);
        let response: serde_json::Value = self.client.get(&url, None::<&()>).await?;

        let message = response
            .get("commit")
            .and_then(|c| c.get("message"))
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        let author = response
            .get("author")
            .and_then(|a| a.get("login"))
            .and_then(|l| l.as_str())
            .or_else(|| {
                response
                    .get("commit")
                    .and_then(|c| c.get("author"))
                    .and_then(|a| a.get("name"))
                    .and_then(|n| n.as_str())
            })
            .unwrap_or("unknown")
            .to_string();

        let date = response
            .get("commit")
            .and_then(|c| c.get("author"))
            .and_then(|a| a.get("date"))
            .and_then(|d| d.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);

        let stats = response
            .get("stats")
            .map(|s| CommitStats {
                additions: s.get("additions").and_then(|a| a.as_u64()).unwrap_or(0),
                deletions: s.get("deletions").and_then(|d| d.as_u64()).unwrap_or(0),
                total: s.get("total").and_then(|t| t.as_u64()).unwrap_or(0),
            })
            .unwrap_or(CommitStats {
                additions: 0,
                deletions: 0,
                total: 0,
            });

        let files = response
            .get("files")
            .and_then(|f| f.as_array())
            .map(|files| {
                files
                    .iter()
                    .filter_map(|f| {
                        Some(CommitFile {
                            filename: f.get("filename")?.as_str()?.to_string(),
                            status: f.get("status")?.as_str()?.to_string(),
                            additions: f.get("additions").and_then(|a| a.as_u64()).unwrap_or(0),
                            deletions: f.get("deletions").and_then(|d| d.as_u64()).unwrap_or(0),
                            patch: f
                                .get("patch")
                                .and_then(|p| p.as_str())
                                .map(|s| s.to_string()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(CommitDetail {
            sha: sha.to_string(),
            message,
            author,
            date,
            stats,
            files,
        })
    }

    async fn get_pr_diff(&self, owner: &str, repo: &str, number: u64) -> Result<String> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}",
            owner, repo, number
        );
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.diff")
            .header("User-Agent", "grit")
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            return Err(GritError::Api(format!(
                "Failed to fetch diff: {}",
                response.status()
            )));
        }

        response
            .text()
            .await
            .map_err(|e| GritError::Api(e.to_string()))
    }

    async fn merge_pr(&self, owner: &str, repo: &str, number: u64, method: &str) -> Result<()> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/merge",
            owner, repo, number
        );
        let client = reqwest::Client::new();
        let body = serde_json::json!({ "merge_method": method });
        let response = client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "grit")
            .json(&body)
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(GritError::Api(format!("Merge failed: {}", text)));
        }
        Ok(())
    }

    async fn close_pr(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        self.client
            .pulls(owner, repo)
            .update(number)
            .state(octocrab::params::pulls::State::Closed)
            .send()
            .await?;
        Ok(())
    }

    async fn close_issue(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        self.client
            .issues(owner, repo)
            .update(number)
            .state(OctoIssueState::Closed)
            .send()
            .await?;
        Ok(())
    }

    async fn comment(&self, owner: &str, repo: &str, number: u64, body: &str) -> Result<()> {
        self.client
            .issues(owner, repo)
            .create_comment(number, body)
            .await?;
        Ok(())
    }

    async fn list_review_requests(&self, username: &str) -> Result<Vec<ReviewRequest>> {
        let query = format!("is:pr is:open review-requested:{}", username);

        let results = self
            .client
            .search()
            .issues_and_pull_requests(&query)
            .per_page(50)
            .send()
            .await?;

        let review_requests = results
            .items
            .into_iter()
            .filter_map(|issue| {
                let repo_url = issue.repository_url.as_str();
                let parts: Vec<&str> = repo_url.split('/').collect();
                if parts.len() < 2 {
                    return None;
                }
                let repo_name = parts[parts.len() - 1].to_string();
                let repo_owner = parts[parts.len() - 2].to_string();

                Some(ReviewRequest {
                    repo_owner,
                    repo_name,
                    pr_number: issue.number,
                    pr_title: issue.title,
                    author: issue.user.login,
                    updated_at: issue.updated_at,
                })
            })
            .collect();

        Ok(review_requests)
    }

    async fn list_my_prs(&self, username: &str) -> Result<Vec<MyPr>> {
        let query = format!("is:pr is:open author:{}", username);

        let results = self
            .client
            .search()
            .issues_and_pull_requests(&query)
            .per_page(50)
            .send()
            .await?;

        let prs_without_status: Vec<_> = results
            .items
            .into_iter()
            .filter_map(|issue| {
                let repo_url = issue.repository_url.as_str();
                let parts: Vec<&str> = repo_url.split('/').collect();
                if parts.len() < 2 {
                    return None;
                }
                let repo_name = parts[parts.len() - 1].to_string();
                let repo_owner = parts[parts.len() - 2].to_string();

                let state = match issue.state {
                    OctoIssueState::Closed => PrState::Closed,
                    _ => PrState::Open,
                };

                Some((
                    repo_owner,
                    repo_name,
                    issue.number,
                    issue.title,
                    state,
                    issue.updated_at,
                ))
            })
            .collect();

        let mut my_prs = Vec::with_capacity(prs_without_status.len());
        for (repo_owner, repo_name, number, title, state, updated_at) in prs_without_status {
            let checks_status = self
                .get_check_status(&repo_owner, &repo_name, number)
                .await
                .unwrap_or(ChecksStatus::None);

            my_prs.push(MyPr {
                repo_owner,
                repo_name,
                number,
                title,
                state,
                checks_status,
                updated_at,
            });
        }

        Ok(my_prs)
    }

    async fn list_action_runs(
        &self,
        owner: &str,
        repo: &str,
        page: u32,
    ) -> Result<PagedResult<ActionRun>> {
        let url = format!(
            "/repos/{}/{}/actions/runs?per_page=50&page={}",
            owner, repo, page
        );
        let response: serde_json::Value = self.client.get(&url, None::<&()>).await?;

        let total = response.get("total_count").and_then(|v| v.as_u64());

        let runs = response
            .get("workflow_runs")
            .and_then(|r| r.as_array())
            .map(|runs| {
                runs.iter()
                    .filter_map(|run| {
                        Some(ActionRun {
                            id: run.get("id")?.as_u64()?,
                            name: run.get("name")?.as_str()?.to_string(),
                            status: match run.get("status")?.as_str()? {
                                "queued" => ActionStatus::Queued,
                                "in_progress" => ActionStatus::InProgress,
                                _ => ActionStatus::Completed,
                            },
                            conclusion: run.get("conclusion").and_then(|c| {
                                c.as_str().map(|s| match s {
                                    "success" => ActionConclusion::Success,
                                    "failure" => ActionConclusion::Failure,
                                    "cancelled" => ActionConclusion::Cancelled,
                                    "skipped" => ActionConclusion::Skipped,
                                    "timed_out" => ActionConclusion::TimedOut,
                                    _ => ActionConclusion::Failure,
                                })
                            }),
                            branch: run
                                .get("head_branch")
                                .and_then(|b| b.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            event: run
                                .get("event")
                                .and_then(|e| e.as_str())
                                .unwrap_or("unknown")
                                .to_string(),
                            created_at: run
                                .get("created_at")
                                .and_then(|d| d.as_str())
                                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                                .map(|d| d.with_timezone(&chrono::Utc))
                                .unwrap_or_else(chrono::Utc::now),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(PagedResult {
            items: runs,
            total_count: total,
        })
    }

    async fn get_check_status(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<ChecksStatus> {
        let pr = self.client.pulls(owner, repo).get(pr_number).await?;
        let sha = pr.head.sha;

        let url = format!("/repos/{}/{}/commits/{}/check-runs", owner, repo, sha);
        let response: serde_json::Value = self.client.get(&url, None::<&()>).await?;

        let check_runs = response.get("check_runs").and_then(|r| r.as_array());

        let Some(runs) = check_runs else {
            return Ok(ChecksStatus::None);
        };

        if runs.is_empty() {
            return Ok(ChecksStatus::None);
        }

        let mut has_pending = false;
        let mut has_failure = false;

        for run in runs {
            let status = run.get("status").and_then(|s| s.as_str());
            let conclusion = run.get("conclusion").and_then(|c| c.as_str());

            match status {
                Some("completed") => match conclusion {
                    Some("success") | Some("skipped") => {}
                    Some("failure") | Some("cancelled") | Some("timed_out") => {
                        has_failure = true;
                    }
                    _ => {}
                },
                Some("queued") | Some("in_progress") => {
                    has_pending = true;
                }
                _ => {}
            }
        }

        if has_failure {
            Ok(ChecksStatus::Failure)
        } else if has_pending {
            Ok(ChecksStatus::Pending)
        } else {
            Ok(ChecksStatus::Success)
        }
    }

    async fn submit_review(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        event: &str,
        body: &str,
    ) -> Result<()> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/reviews",
            owner, repo, number
        );
        let client = reqwest::Client::new();
        let payload = serde_json::json!({
            "event": event,
            "body": body,
        });
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "grit")
            .json(&payload)
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(GritError::Api(format!("Review failed: {}", text)));
        }
        Ok(())
    }
}
