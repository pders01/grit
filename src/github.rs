use octocrab::models::IssueState as OctoIssueState;
use octocrab::Octocrab;

use crate::error::{GritError, Result};
use crate::types::{
    ActionConclusion, ActionRun, ActionStatus, ChecksStatus, Commit, Issue, IssueState, MyPr,
    PrState, PrStats, PrSummary, PullRequest, Repository, ReviewRequest,
};

pub struct GitHub {
    client: Octocrab,
}

impl GitHub {
    pub fn new(token: String) -> Result<Self> {
        let client = Octocrab::builder()
            .personal_token(token)
            .build()
            .map_err(|e| GritError::Auth(e.to_string()))?;

        Ok(Self { client })
    }

    pub async fn list_repos(&self) -> Result<Vec<Repository>> {
        let repos = self
            .client
            .current()
            .list_repos_for_authenticated_user()
            .sort("updated")
            .direction("desc")
            .per_page(50)
            .send()
            .await?;

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

        Ok(repositories)
    }

    pub async fn list_prs(&self, owner: &str, repo: &str) -> Result<Vec<PrSummary>> {
        let prs = self
            .client
            .pulls(owner, repo)
            .list()
            .state(octocrab::params::State::Open)
            .sort(octocrab::params::pulls::Sort::Updated)
            .direction(octocrab::params::Direction::Descending)
            .per_page(50)
            .send()
            .await?;

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

        Ok(summaries)
    }

    pub async fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
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

    /// Get the current authenticated user's login
    pub async fn get_current_user(&self) -> Result<String> {
        let user = self.client.current().user().await?;
        Ok(user.login)
    }

    /// List PRs where the current user is requested as a reviewer
    pub async fn list_review_requests(&self, username: &str) -> Result<Vec<ReviewRequest>> {
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
                // Parse repo info from the repository_url
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

    /// List open PRs authored by the current user
    pub async fn list_my_prs(&self, username: &str) -> Result<Vec<MyPr>> {
        let query = format!("is:pr is:open author:{}", username);

        let results = self
            .client
            .search()
            .issues_and_pull_requests(&query)
            .per_page(50)
            .send()
            .await?;

        // First, collect basic PR info
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

                Some((repo_owner, repo_name, issue.number, issue.title, state, issue.updated_at))
            })
            .collect();

        // Fetch check status for each PR concurrently
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

    /// List issues for a repository
    pub async fn list_issues(&self, owner: &str, repo: &str) -> Result<Vec<Issue>> {
        let issues = self
            .client
            .issues(owner, repo)
            .list()
            .state(octocrab::params::State::Open)
            .sort(octocrab::params::issues::Sort::Updated)
            .direction(octocrab::params::Direction::Descending)
            .per_page(50)
            .send()
            .await?;

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

        Ok(result)
    }

    /// List commits for a repository (default branch)
    pub async fn list_commits(&self, owner: &str, repo: &str) -> Result<Vec<Commit>> {
        let commits = self
            .client
            .repos(owner, repo)
            .list_commits()
            .per_page(50)
            .send()
            .await?;

        let result = commits
            .items
            .into_iter()
            .map(|c| {
                let message = c
                    .commit
                    .message
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
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
                    sha: c.sha[..7].to_string(),
                    message,
                    author,
                    date,
                }
            })
            .collect();

        Ok(result)
    }

    /// List GitHub Actions workflow runs for a repository
    pub async fn list_action_runs(&self, owner: &str, repo: &str) -> Result<Vec<ActionRun>> {
        let url = format!("/repos/{}/{}/actions/runs?per_page=30", owner, repo);
        let response: serde_json::Value = self.client.get(&url, None::<&()>).await?;

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

        Ok(runs)
    }

    /// Get check status for a PR
    pub async fn get_check_status(&self, owner: &str, repo: &str, pr_number: u64) -> Result<ChecksStatus> {
        // Get the PR to find the head SHA
        let pr = self.client.pulls(owner, repo).get(pr_number).await?;
        let sha = pr.head.sha;

        // Get combined status
        let url = format!("/repos/{}/{}/commits/{}/check-runs", owner, repo, sha);
        let response: serde_json::Value = self.client.get(&url, None::<&()>).await?;

        let check_runs = response
            .get("check_runs")
            .and_then(|r| r.as_array());

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
}
