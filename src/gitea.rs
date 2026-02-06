use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

use crate::error::{GritError, Result};
use crate::forge::Forge;
use crate::types::{
    Commit, CommitDetail, CommitFile, CommitStats, Issue, IssueState, PrState, PrStats, PrSummary,
    PullRequest, Repository,
};

pub struct Gitea {
    client: Client,
    host: String,
    token: String,
}

impl Gitea {
    pub fn new(host: String, token: String) -> Self {
        Self {
            client: Client::new(),
            host,
            token,
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("https://{}/api/v1{}", self.host, path)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let response = self
            .client
            .get(url)
            .header("Authorization", format!("token {}", self.token))
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(GritError::Api(format!("Gitea API {}: {}", status, text)));
        }

        response
            .json()
            .await
            .map_err(|e| GritError::Api(e.to_string()))
    }
}

// Gitea API response types

#[derive(Deserialize)]
struct GtRepo {
    owner: Option<GtUser>,
    name: String,
    description: Option<String>,
    html_url: Option<String>,
    stars_count: Option<u32>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct GtUser {
    login: String,
}

#[derive(Deserialize)]
struct GtPullRequest {
    number: u64,
    title: String,
    state: String,
    body: Option<String>,
    user: Option<GtUser>,
    head: Option<GtPrRef>,
    base: Option<GtPrRef>,
    additions: Option<u64>,
    deletions: Option<u64>,
    changed_files: Option<u64>,
    comments: Option<u64>,
    merged: Option<bool>,
    created_at: Option<String>,
    updated_at: Option<String>,
    merged_at: Option<String>,
    closed_at: Option<String>,
}

#[derive(Deserialize)]
struct GtPrRef {
    #[serde(rename = "ref")]
    ref_field: Option<String>,
}

#[derive(Deserialize)]
struct GtIssue {
    number: u64,
    title: String,
    state: String,
    user: Option<GtUser>,
    labels: Option<Vec<GtLabel>>,
    comments: Option<u32>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct GtLabel {
    name: String,
}

#[derive(Deserialize)]
struct GtCommit {
    sha: Option<String>,
    commit: Option<GtCommitInner>,
    #[allow(dead_code)]
    html_url: Option<String>,
}

#[derive(Deserialize)]
struct GtCommitInner {
    message: Option<String>,
    author: Option<GtCommitAuthor>,
}

#[derive(Deserialize)]
struct GtCommitAuthor {
    name: Option<String>,
    date: Option<String>,
}

#[derive(Deserialize)]
struct GtCommitDetail {
    sha: Option<String>,
    commit: Option<GtCommitInner>,
    stats: Option<GtCommitStats>,
    files: Option<Vec<GtCommitFile>>,
}

#[derive(Deserialize)]
struct GtCommitStats {
    additions: Option<u64>,
    deletions: Option<u64>,
    total: Option<u64>,
}

#[derive(Deserialize)]
struct GtCommitFile {
    filename: Option<String>,
    status: Option<String>,
    additions: Option<u64>,
    deletions: Option<u64>,
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
impl Forge for Gitea {
    fn name(&self) -> &str {
        "Gitea"
    }

    fn web_url(&self, owner: &str, repo: &str, kind: &str, id: &str) -> String {
        match kind {
            "repo" => format!("https://{}/{}/{}", self.host, owner, repo),
            "pr" => format!("https://{}/{}/{}/pulls/{}", self.host, owner, repo, id),
            "issue" => format!("https://{}/{}/{}/issues/{}", self.host, owner, repo, id),
            "commit" => format!("https://{}/{}/{}/commit/{}", self.host, owner, repo, id),
            _ => format!("https://{}/{}/{}", self.host, owner, repo),
        }
    }

    async fn get_current_user(&self) -> Result<String> {
        let url = self.api_url("/user");
        let user: GtUser = self.get_json(&url).await?;
        Ok(user.login)
    }

    async fn list_repos(&self, page: u32) -> Result<Vec<Repository>> {
        let url = self.api_url(&format!("/user/repos?sort=updated&limit=50&page={}", page));
        let repos: Vec<GtRepo> = self.get_json(&url).await?;

        let result = repos
            .into_iter()
            .map(|r| Repository {
                owner: r
                    .owner
                    .map(|o| o.login)
                    .unwrap_or_else(|| "unknown".to_string()),
                name: r.name,
                description: r.description.filter(|d| !d.is_empty()),
                url: r.html_url.unwrap_or_default(),
                stars: r.stars_count.unwrap_or(0),
                updated_at: parse_optional_datetime(r.updated_at.as_deref()),
            })
            .collect();

        Ok(result)
    }

    async fn list_prs(&self, owner: &str, repo: &str, page: u32) -> Result<Vec<PrSummary>> {
        let url = self.api_url(&format!(
            "/repos/{}/{}/pulls?state=open&sort=updated&limit=50&page={}",
            owner, repo, page
        ));
        let prs: Vec<GtPullRequest> = self.get_json(&url).await?;

        let summaries = prs
            .into_iter()
            .map(|pr| PrSummary {
                number: pr.number,
                title: pr.title,
                state: gt_pr_state(&pr.state, pr.merged),
                author: pr
                    .user
                    .map(|u| u.login)
                    .unwrap_or_else(|| "unknown".to_string()),
                updated_at: parse_optional_datetime(pr.updated_at.as_deref()),
            })
            .collect();

        Ok(summaries)
    }

    async fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest> {
        let url = self.api_url(&format!("/repos/{}/{}/pulls/{}", owner, repo, number));
        let pr: GtPullRequest = self.get_json(&url).await?;

        Ok(PullRequest {
            number: pr.number,
            title: pr.title,
            body: pr.body,
            state: gt_pr_state(&pr.state, pr.merged),
            author: pr
                .user
                .map(|u| u.login)
                .unwrap_or_else(|| "unknown".to_string()),
            head_branch: pr.head.and_then(|h| h.ref_field).unwrap_or_default(),
            base_branch: pr.base.and_then(|b| b.ref_field).unwrap_or_default(),
            stats: PrStats {
                additions: pr.additions.unwrap_or(0),
                deletions: pr.deletions.unwrap_or(0),
                changed_files: pr.changed_files.unwrap_or(0),
                commits: 0,
                comments: pr.comments.unwrap_or(0),
            },
            created_at: parse_optional_datetime(pr.created_at.as_deref()),
            updated_at: parse_optional_datetime(pr.updated_at.as_deref()),
            merged_at: pr.merged_at.as_deref().map(parse_datetime),
            closed_at: pr.closed_at.as_deref().map(parse_datetime),
        })
    }

    async fn list_issues(&self, owner: &str, repo: &str, page: u32) -> Result<Vec<Issue>> {
        let url = self.api_url(&format!(
            "/repos/{}/{}/issues?type=issues&state=open&sort=updated&limit=50&page={}",
            owner, repo, page
        ));
        let issues: Vec<GtIssue> = self.get_json(&url).await?;

        let result = issues
            .into_iter()
            .map(|i| Issue {
                number: i.number,
                title: i.title,
                state: if i.state == "closed" {
                    IssueState::Closed
                } else {
                    IssueState::Open
                },
                author: i
                    .user
                    .map(|u| u.login)
                    .unwrap_or_else(|| "unknown".to_string()),
                labels: i
                    .labels
                    .unwrap_or_default()
                    .into_iter()
                    .map(|l| l.name)
                    .collect(),
                comments: i.comments.unwrap_or(0),
                created_at: parse_optional_datetime(i.created_at.as_deref()),
                updated_at: parse_optional_datetime(i.updated_at.as_deref()),
            })
            .collect();

        Ok(result)
    }

    async fn list_commits(&self, owner: &str, repo: &str, page: u32) -> Result<Vec<Commit>> {
        let url = self.api_url(&format!(
            "/repos/{}/{}/commits?limit=50&page={}",
            owner, repo, page
        ));
        let commits: Vec<GtCommit> = self.get_json(&url).await?;

        let result = commits
            .into_iter()
            .map(|c| {
                let inner = c.commit.as_ref();
                let message = inner
                    .and_then(|i| i.message.as_deref())
                    .and_then(|m| m.lines().next())
                    .unwrap_or("")
                    .to_string();
                let author = inner
                    .and_then(|i| i.author.as_ref())
                    .and_then(|a| a.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                let date = inner
                    .and_then(|i| i.author.as_ref())
                    .and_then(|a| a.date.as_deref())
                    .map(parse_datetime)
                    .unwrap_or_else(chrono::Utc::now);

                Commit {
                    sha: c.sha.unwrap_or_default(),
                    message,
                    author,
                    date,
                }
            })
            .collect();

        Ok(result)
    }

    async fn get_commit(&self, owner: &str, repo: &str, sha: &str) -> Result<CommitDetail> {
        let url = self.api_url(&format!("/repos/{}/{}/git/commits/{}", owner, repo, sha));
        let detail: GtCommitDetail = self.get_json(&url).await?;

        let inner = detail.commit.as_ref();
        let message = inner.and_then(|i| i.message.clone()).unwrap_or_default();
        let author = inner
            .and_then(|i| i.author.as_ref())
            .and_then(|a| a.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let date = inner
            .and_then(|i| i.author.as_ref())
            .and_then(|a| a.date.as_deref())
            .map(parse_datetime)
            .unwrap_or_else(chrono::Utc::now);

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

        let files = detail
            .files
            .unwrap_or_default()
            .into_iter()
            .filter_map(|f| {
                Some(CommitFile {
                    filename: f.filename?,
                    status: f.status.unwrap_or_else(|| "modified".to_string()),
                    additions: f.additions.unwrap_or(0),
                    deletions: f.deletions.unwrap_or(0),
                    patch: None,
                })
            })
            .collect();

        Ok(CommitDetail {
            sha: detail.sha.unwrap_or_else(|| sha.to_string()),
            message,
            author,
            date,
            stats,
            files,
        })
    }

    async fn get_pr_diff(&self, owner: &str, repo: &str, number: u64) -> Result<String> {
        let url = format!(
            "https://{}/api/v1/repos/{}/{}/pulls/{}.diff",
            self.host, owner, repo, number
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("token {}", self.token))
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
        let url = self.api_url(&format!("/repos/{}/{}/pulls/{}/merge", owner, repo, number));

        let do_method = match method {
            "squash" => "squash",
            "rebase" => "rebase",
            _ => "merge",
        };

        let body = serde_json::json!({ "Do": do_method });
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("token {}", self.token))
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
        let url = self.api_url(&format!("/repos/{}/{}/pulls/{}", owner, repo, number));
        let body = serde_json::json!({ "state": "closed" });
        let response = self
            .client
            .patch(&url)
            .header("Authorization", format!("token {}", self.token))
            .json(&body)
            .send()
            .await
            .map_err(|e| GritError::Api(e.to_string()))?;

        if !response.status().is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(GritError::Api(format!("Close PR failed: {}", text)));
        }
        Ok(())
    }

    async fn close_issue(&self, owner: &str, repo: &str, number: u64) -> Result<()> {
        let url = self.api_url(&format!("/repos/{}/{}/issues/{}", owner, repo, number));
        let body = serde_json::json!({ "state": "closed" });
        let response = self
            .client
            .patch(&url)
            .header("Authorization", format!("token {}", self.token))
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
        // In Gitea, PRs are issues — comment via issues API
        let url = self.api_url(&format!(
            "/repos/{}/{}/issues/{}/comments",
            owner, repo, number
        ));
        let payload = serde_json::json!({ "body": body });
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("token {}", self.token))
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
}

fn gt_pr_state(state: &str, merged: Option<bool>) -> PrState {
    if merged == Some(true) {
        PrState::Merged
    } else if state == "closed" {
        PrState::Closed
    } else {
        PrState::Open
    }
}
