use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use crate::error::{GritError, Result};
use crate::forge::Forge;
use crate::types::{
    ActionConclusion, ActionRun, ActionStatus, ChecksStatus, Commit, CommitDetail, CommitFile,
    CommitStats, Issue, IssueState, PrState, PrStats, PrSummary, PullRequest, Repository,
};

pub struct GitLab {
    client: Client,
    host: String,
    token: String,
}

impl GitLab {
    pub fn new(host: String, token: String) -> Self {
        Self {
            client: Client::new(),
            host,
            token,
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("https://{}/api/v4{}", self.host, path)
    }

    /// URL-encode owner/repo as a project path for GitLab API
    fn project_path(owner: &str, repo: &str) -> String {
        urlencoding::encode(&format!("{}/{}", owner, repo)).into_owned()
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(GritError::Api(format!("GitLab API {}: {}", status, text)));
        }

        response
            .json()
            .await
            .map_err(|e| GritError::Api(e.to_string()))
    }
}

// GitLab API response types

#[derive(Deserialize)]
struct GlProject {
    path_with_namespace: String,
    name: String,
    description: Option<String>,
    web_url: String,
    star_count: Option<u32>,
    last_activity_at: Option<String>,
}

#[derive(Deserialize)]
struct GlUser {
    username: String,
}

#[derive(Deserialize)]
struct GlMergeRequest {
    iid: u64,
    title: String,
    state: String,
    description: Option<String>,
    author: GlMrAuthor,
    source_branch: Option<String>,
    target_branch: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    merged_at: Option<String>,
    closed_at: Option<String>,
    user_notes_count: Option<u64>,
    changes_count: Option<String>,
}

#[derive(Deserialize)]
struct GlMrAuthor {
    username: String,
}

#[derive(Deserialize)]
struct GlIssue {
    iid: u64,
    title: String,
    state: String,
    author: GlMrAuthor,
    labels: Vec<String>,
    user_notes_count: Option<u32>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct GlCommit {
    id: String,
    title: Option<String>,
    message: Option<String>,
    author_name: Option<String>,
    created_at: Option<String>,
}

#[derive(Deserialize)]
struct GlCommitDetail {
    id: String,
    message: Option<String>,
    author_name: Option<String>,
    created_at: Option<String>,
    stats: Option<GlCommitStats>,
}

#[derive(Deserialize)]
struct GlCommitStats {
    additions: Option<u64>,
    deletions: Option<u64>,
    total: Option<u64>,
}

#[derive(Deserialize)]
struct GlDiff {
    new_path: String,
    new_file: bool,
    renamed_file: bool,
    deleted_file: bool,
    diff: Option<String>,
}

#[derive(Deserialize)]
struct GlPipeline {
    id: u64,
    status: String,
    #[serde(rename = "ref")]
    ref_field: Option<String>,
    source: Option<String>,
    created_at: Option<String>,
    #[allow(dead_code)]
    web_url: Option<String>,
}

fn parse_datetime(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn parse_optional_datetime(s: Option<&str>) -> chrono::DateTime<chrono::Utc> {
    s.map(parse_datetime).unwrap_or_else(chrono::Utc::now)
}

#[async_trait]
impl Forge for GitLab {
    fn name(&self) -> &str {
        "GitLab"
    }

    fn web_url(&self, owner: &str, repo: &str, kind: &str, id: &str) -> String {
        match kind {
            "repo" => format!("https://{}/{}/{}", self.host, owner, repo),
            "pr" => format!(
                "https://{}/{}/{}/-/merge_requests/{}",
                self.host, owner, repo, id
            ),
            "issue" => format!("https://{}/{}/{}/-/issues/{}", self.host, owner, repo, id),
            "commit" => format!("https://{}/{}/{}/-/commit/{}", self.host, owner, repo, id),
            "action_run" => format!(
                "https://{}/{}/{}/-/pipelines/{}",
                self.host, owner, repo, id
            ),
            _ => format!("https://{}/{}/{}", self.host, owner, repo),
        }
    }

    async fn get_current_user(&self) -> Result<String> {
        let url = self.api_url("/user");
        let user: GlUser = self.get_json(&url).await?;
        Ok(user.username)
    }

    async fn list_repos(&self, page: u32) -> Result<Vec<Repository>> {
        let url = self.api_url(&format!(
            "/projects?membership=true&order_by=last_activity_at&sort=desc&per_page=50&page={}",
            page
        ));
        let projects: Vec<GlProject> = self.get_json(&url).await?;

        let repos = projects
            .into_iter()
            .map(|p| {
                let parts: Vec<&str> = p.path_with_namespace.splitn(2, '/').collect();
                let owner = parts.first().unwrap_or(&"unknown").to_string();
                let name = if parts.len() > 1 {
                    parts[1].to_string()
                } else {
                    p.name
                };

                Repository {
                    owner,
                    name,
                    description: p.description.filter(|d| !d.is_empty()),
                    url: p.web_url,
                    stars: p.star_count.unwrap_or(0),
                    updated_at: parse_optional_datetime(p.last_activity_at.as_deref()),
                }
            })
            .collect();

        Ok(repos)
    }

    async fn list_prs(&self, owner: &str, repo: &str, page: u32) -> Result<Vec<PrSummary>> {
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!(
            "/projects/{}/merge_requests?state=opened&order_by=updated_at&sort=desc&per_page=50&page={}",
            project, page
        ));
        let mrs: Vec<GlMergeRequest> = self.get_json(&url).await?;

        let summaries = mrs
            .into_iter()
            .map(|mr| PrSummary {
                number: mr.iid,
                title: mr.title,
                state: gl_mr_state(&mr.state),
                author: mr.author.username,
                updated_at: parse_optional_datetime(mr.updated_at.as_deref()),
            })
            .collect();

        Ok(summaries)
    }

    async fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!("/projects/{}/merge_requests/{}", project, number));
        let mr: GlMergeRequest = self.get_json(&url).await?;

        let changes_count = mr
            .changes_count
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        Ok(PullRequest {
            number: mr.iid,
            title: mr.title,
            body: mr.description,
            state: gl_mr_state(&mr.state),
            author: mr.author.username,
            head_branch: mr.source_branch.unwrap_or_default(),
            base_branch: mr.target_branch.unwrap_or_default(),
            stats: PrStats {
                additions: 0,
                deletions: 0,
                changed_files: changes_count,
                commits: 0,
                comments: mr.user_notes_count.unwrap_or(0),
            },
            created_at: parse_optional_datetime(mr.created_at.as_deref()),
            updated_at: parse_optional_datetime(mr.updated_at.as_deref()),
            merged_at: mr.merged_at.as_deref().map(parse_datetime),
            closed_at: mr.closed_at.as_deref().map(parse_datetime),
        })
    }

    async fn list_issues(&self, owner: &str, repo: &str, page: u32) -> Result<Vec<Issue>> {
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!(
            "/projects/{}/issues?state=opened&order_by=updated_at&sort=desc&per_page=50&page={}",
            project, page
        ));
        let issues: Vec<GlIssue> = self.get_json(&url).await?;

        let result = issues
            .into_iter()
            .map(|i| Issue {
                number: i.iid,
                title: i.title,
                state: if i.state == "closed" {
                    IssueState::Closed
                } else {
                    IssueState::Open
                },
                author: i.author.username,
                labels: i.labels,
                comments: i.user_notes_count.unwrap_or(0),
                created_at: parse_optional_datetime(i.created_at.as_deref()),
                updated_at: parse_optional_datetime(i.updated_at.as_deref()),
            })
            .collect();

        Ok(result)
    }

    async fn list_commits(&self, owner: &str, repo: &str, page: u32) -> Result<Vec<Commit>> {
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!(
            "/projects/{}/repository/commits?per_page=50&page={}",
            project, page
        ));
        let commits: Vec<GlCommit> = self.get_json(&url).await?;

        let result = commits
            .into_iter()
            .map(|c| {
                let message = c
                    .title
                    .or(c
                        .message
                        .as_ref()
                        .map(|m| m.lines().next().unwrap_or("").to_string()))
                    .unwrap_or_default();

                Commit {
                    sha: c.id,
                    message,
                    author: c.author_name.unwrap_or_else(|| "unknown".to_string()),
                    date: parse_optional_datetime(c.created_at.as_deref()),
                }
            })
            .collect();

        Ok(result)
    }

    async fn get_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<CommitDetail> {
        let project = Self::project_path(owner, repo);

        // Fetch commit detail and diff in parallel
        let detail_url = self.api_url(&format!("/projects/{}/repository/commits/{}", project, sha));
        let diff_url = self.api_url(&format!(
            "/projects/{}/repository/commits/{}/diff",
            project, sha
        ));

        let (detail, diffs): (GlCommitDetail, Vec<GlDiff>) =
            tokio::try_join!(self.get_json(&detail_url), self.get_json(&diff_url),)?;

        let stats = detail.stats.as_ref().map_or(
            CommitStats {
                additions: 0,
                deletions: 0,
                total: 0,
            },
            |s| CommitStats {
                additions: s.additions.unwrap_or(0),
                deletions: s.deletions.unwrap_or(0),
                total: s.total.unwrap_or(0),
            },
        );

        let files = diffs
            .into_iter()
            .map(|d| {
                let status = if d.new_file {
                    "added"
                } else if d.deleted_file {
                    "removed"
                } else if d.renamed_file {
                    "renamed"
                } else {
                    "modified"
                };

                // Count additions/deletions from diff text
                let (additions, deletions) = d
                    .diff
                    .as_deref()
                    .map(|text| {
                        let mut adds: u64 = 0;
                        let mut dels: u64 = 0;
                        for line in text.lines() {
                            if line.starts_with('+') && !line.starts_with("+++") {
                                adds += 1;
                            } else if line.starts_with('-') && !line.starts_with("---") {
                                dels += 1;
                            }
                        }
                        (adds, dels)
                    })
                    .unwrap_or((0, 0));

                CommitFile {
                    filename: d.new_path,
                    status: status.to_string(),
                    additions,
                    deletions,
                    patch: d.diff,
                }
            })
            .collect();

        Ok(CommitDetail {
            sha: detail.id,
            message: detail.message.unwrap_or_default(),
            author: detail.author_name.unwrap_or_else(|| "unknown".to_string()),
            date: parse_optional_datetime(detail.created_at.as_deref()),
            stats,
            files,
        })
    }

    async fn get_pr_diff(&self, owner: &str, repo: &str, number: u64) -> Result<String> {
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!(
            "/projects/{}/merge_requests/{}/changes",
            project, number
        ));
        let mr: serde_json::Value = self.get_json(&url).await?;

        // Build a unified diff from the structured changes
        let mut diff = String::new();
        if let Some(changes) = mr.get("changes").and_then(|c| c.as_array()) {
            for change in changes {
                let old_path = change
                    .get("old_path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown");
                let new_path = change
                    .get("new_path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown");
                let diff_text = change.get("diff").and_then(|d| d.as_str()).unwrap_or("");

                diff.push_str(&format!("diff --git a/{} b/{}\n", old_path, new_path));
                diff.push_str(&format!("--- a/{}\n", old_path));
                diff.push_str(&format!("+++ b/{}\n", new_path));
                diff.push_str(diff_text);
                if !diff_text.ends_with('\n') {
                    diff.push('\n');
                }
            }
        }

        Ok(diff)
    }

    async fn merge_pr(&self, owner: &str, repo: &str, number: u64, method: &str) -> Result<()> {
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!(
            "/projects/{}/merge_requests/{}/merge",
            project, number
        ));

        let merge_method = match method {
            "squash" => "squash_merge",
            "rebase" => "rebase_merge",
            _ => "merge",
        };

        let body = serde_json::json!({ "merge_method": merge_method });
        let response = self
            .client
            .put(&url)
            .header("PRIVATE-TOKEN", &self.token)
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
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!("/projects/{}/merge_requests/{}", project, number));
        let body = serde_json::json!({ "state_event": "close" });
        let response = self
            .client
            .put(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(GritError::Api(format!("Close MR failed: {}", text)));
        }
        Ok(())
    }

    async fn close_issue(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!("/projects/{}/issues/{}", project, number));
        let body = serde_json::json!({ "state_event": "close" });
        let response = self
            .client
            .put(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&body)
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(GritError::Api(format!("Close issue failed: {}", text)));
        }
        Ok(())
    }

    async fn comment(&self, owner: &str, repo: &str, number: u64, body: &str) -> Result<()> {
        let project = Self::project_path(owner, repo);
        // GitLab uses "notes" for comments on merge requests
        let url = self.api_url(&format!(
            "/projects/{}/merge_requests/{}/notes",
            project, number
        ));
        let payload = serde_json::json!({ "body": body });
        let response = self
            .client
            .post(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(GritError::Api(format!("Comment failed: {}", text)));
        }
        Ok(())
    }

    async fn list_action_runs(&self, owner: &str, repo: &str, page: u32) -> Result<Vec<ActionRun>> {
        let project = Self::project_path(owner, repo);
        let url = self.api_url(&format!(
            "/projects/{}/pipelines?per_page=50&page={}",
            project, page
        ));
        let pipelines: Vec<GlPipeline> = self.get_json(&url).await?;

        let runs = pipelines
            .into_iter()
            .map(|p| {
                let (status, conclusion) = gl_pipeline_status(&p.status);
                ActionRun {
                    id: p.id,
                    name: format!("Pipeline #{}", p.id),
                    status,
                    conclusion,
                    branch: p.ref_field.unwrap_or_else(|| "unknown".to_string()),
                    event: p.source.unwrap_or_else(|| "push".to_string()),
                    created_at: parse_optional_datetime(p.created_at.as_deref()),
                }
            })
            .collect();

        Ok(runs)
    }

    async fn get_check_status(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<ChecksStatus> {
        let project = Self::project_path(owner, repo);
        // Get pipelines for the MR
        let url = self.api_url(&format!(
            "/projects/{}/merge_requests/{}/pipelines",
            project, pr_number
        ));
        let pipelines: Vec<GlPipeline> = self.get_json(&url).await?;

        let Some(latest) = pipelines.first() else {
            return Ok(ChecksStatus::None);
        };

        match latest.status.as_str() {
            "success" => Ok(ChecksStatus::Success),
            "failed" => Ok(ChecksStatus::Failure),
            "running" | "pending" | "created" | "waiting_for_resource" | "preparing" => {
                Ok(ChecksStatus::Pending)
            }
            "canceled" | "skipped" => Ok(ChecksStatus::Failure),
            _ => Ok(ChecksStatus::None),
        }
    }
}

fn gl_mr_state(state: &str) -> PrState {
    match state {
        "merged" => PrState::Merged,
        "closed" => PrState::Closed,
        _ => PrState::Open,
    }
}

fn gl_pipeline_status(status: &str) -> (ActionStatus, Option<ActionConclusion>) {
    match status {
        "created" | "waiting_for_resource" | "preparing" | "pending" => {
            (ActionStatus::Queued, None)
        }
        "running" => (ActionStatus::InProgress, None),
        "success" => (ActionStatus::Completed, Some(ActionConclusion::Success)),
        "failed" => (ActionStatus::Completed, Some(ActionConclusion::Failure)),
        "canceled" => (ActionStatus::Completed, Some(ActionConclusion::Cancelled)),
        "skipped" => (ActionStatus::Completed, Some(ActionConclusion::Skipped)),
        _ => (ActionStatus::Completed, Some(ActionConclusion::Failure)),
    }
}
