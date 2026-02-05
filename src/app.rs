use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use crate::action::{Action, RepoTab};
use crate::event::Event;
use crate::github::GitHub;
use crate::types::{ActionRun, Commit, CommitDetail, Issue, MyPr, PrSummary, PullRequest, Repository, ReviewRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,         // Dashboard with review requests + your PRs
    RepoList,     // Repository browser
    RepoView,     // Repo view with tabs (PRs, Issues, Commits, Actions)
    PrDetail,     // PR detail view
    CommitDetail, // Commit detail view
}

/// Section of the home screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeSection {
    #[default]
    ReviewRequests,
    MyPrs,
}

pub struct App {
    pub screen: Screen,

    // Home screen data
    pub review_requests: Vec<ReviewRequest>,
    pub my_prs: Vec<MyPr>,
    pub home_section: HomeSection,
    pub review_index: usize,
    pub my_pr_index: usize,

    // Repo view
    pub repo_tab: RepoTab,

    // Repo tab data
    pub issues: Vec<Issue>,
    pub commits: Vec<Commit>,
    pub action_runs: Vec<ActionRun>,
    pub issue_index: usize,
    pub commit_index: usize,
    pub action_index: usize,

    // Existing state
    pub repos: Vec<Repository>,
    pub prs: Vec<PrSummary>,
    pub current_pr: Option<PullRequest>,
    pub current_commit: Option<CommitDetail>,
    pub repo_index: usize,
    pub pr_index: usize,
    pub scroll_offset: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub should_quit: bool,
    pub current_repo: Option<(String, String)>,
    prev_screen: Option<Screen>,
    github: Arc<GitHub>,
    action_tx: mpsc::UnboundedSender<Action>,
}

impl App {
    pub fn new(github: GitHub, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            screen: Screen::Home,

            // Home screen
            review_requests: Vec::new(),
            my_prs: Vec::new(),
            home_section: HomeSection::default(),
            review_index: 0,
            my_pr_index: 0,

            // Repo view
            repo_tab: RepoTab::default(),

            // Repo tab data
            issues: Vec::new(),
            commits: Vec::new(),
            action_runs: Vec::new(),
            issue_index: 0,
            commit_index: 0,
            action_index: 0,

            // Existing
            repos: Vec::new(),
            prs: Vec::new(),
            current_pr: None,
            current_commit: None,
            repo_index: 0,
            pr_index: 0,
            scroll_offset: 0,
            loading: false,
            error: None,
            should_quit: false,
            current_repo: None,
            prev_screen: None,
            github: Arc::new(github),
            action_tx,
        }
    }

    pub fn handle_event(&self, event: Event) -> Action {
        match event {
            Event::Init => Action::LoadHome,
            Event::Key(key) => self.handle_key(key),
            _ => Action::None,
        }
    }

    fn handle_key(&self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') => {
                if self.screen == Screen::Home {
                    Action::Quit
                } else {
                    Action::Back
                }
            }
            KeyCode::Esc => match self.screen {
                Screen::Home => Action::Quit,
                _ => Action::Back,
            },
            KeyCode::Char('j') | KeyCode::Down => Action::ScrollDown,
            KeyCode::Char('k') | KeyCode::Up => Action::ScrollUp,
            KeyCode::Enter => Action::Select,
            KeyCode::Tab => {
                if self.screen == Screen::Home {
                    Action::SwitchHomeSection
                } else if self.screen == Screen::RepoView {
                    // Cycle through tabs
                    let next = match self.repo_tab {
                        RepoTab::PullRequests => RepoTab::Issues,
                        RepoTab::Issues => RepoTab::Commits,
                        RepoTab::Commits => RepoTab::Actions,
                        RepoTab::Actions => RepoTab::PullRequests,
                    };
                    Action::SwitchRepoTab(next)
                } else {
                    Action::None
                }
            }
            KeyCode::Char('r') => {
                if self.screen == Screen::Home {
                    Action::LoadRepos
                } else {
                    Action::None
                }
            }
            // Repo view tab shortcuts
            KeyCode::Char('p') => {
                if self.screen == Screen::RepoView {
                    Action::SwitchRepoTab(RepoTab::PullRequests)
                } else {
                    Action::None
                }
            }
            KeyCode::Char('i') => {
                if self.screen == Screen::RepoView {
                    Action::SwitchRepoTab(RepoTab::Issues)
                } else {
                    Action::None
                }
            }
            KeyCode::Char('c') => {
                if self.screen == Screen::RepoView {
                    Action::SwitchRepoTab(RepoTab::Commits)
                } else {
                    Action::None
                }
            }
            KeyCode::Char('a') => {
                if self.screen == Screen::RepoView {
                    Action::SwitchRepoTab(RepoTab::Actions)
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }

    pub fn update(&mut self, action: Action) {
        if self.error.is_some() {
            if !matches!(action, Action::Quit | Action::Back) {
                self.error = None;
            }
        }

        match action {
            Action::Quit => {
                self.should_quit = true;
            }
            Action::Back => {
                match self.screen {
                    Screen::Home => {
                        self.should_quit = true;
                    }
                    Screen::RepoList => {
                        self.screen = Screen::Home;
                    }
                    Screen::RepoView => {
                        self.screen = Screen::RepoList;
                        self.repo_tab = RepoTab::default();
                        self.prs.clear();
                        self.issues.clear();
                        self.commits.clear();
                        self.action_runs.clear();
                    }
                    Screen::PrDetail => {
                        self.screen = self.prev_screen.unwrap_or(Screen::Home);
                        self.current_pr = None;
                        self.scroll_offset = 0;
                        self.prev_screen = None;
                    }
                    Screen::CommitDetail => {
                        self.screen = self.prev_screen.unwrap_or(Screen::RepoView);
                        self.current_commit = None;
                        self.scroll_offset = 0;
                        self.prev_screen = None;
                    }
                }
            }
            Action::ScrollUp => match self.screen {
                Screen::Home => match self.home_section {
                    HomeSection::ReviewRequests => {
                        if self.review_index > 0 {
                            self.review_index -= 1;
                        }
                    }
                    HomeSection::MyPrs => {
                        if self.my_pr_index > 0 {
                            self.my_pr_index -= 1;
                        }
                    }
                },
                Screen::RepoList => {
                    if self.repo_index > 0 {
                        self.repo_index -= 1;
                    }
                }
                Screen::RepoView => match self.repo_tab {
                    RepoTab::PullRequests => {
                        if self.pr_index > 0 {
                            self.pr_index -= 1;
                        }
                    }
                    RepoTab::Issues => {
                        if self.issue_index > 0 {
                            self.issue_index -= 1;
                        }
                    }
                    RepoTab::Commits => {
                        if self.commit_index > 0 {
                            self.commit_index -= 1;
                        }
                    }
                    RepoTab::Actions => {
                        if self.action_index > 0 {
                            self.action_index -= 1;
                        }
                    }
                },
                Screen::PrDetail | Screen::CommitDetail => {
                    if self.scroll_offset > 0 {
                        self.scroll_offset -= 1;
                    }
                }
            },
            Action::ScrollDown => match self.screen {
                Screen::Home => match self.home_section {
                    HomeSection::ReviewRequests => {
                        if !self.review_requests.is_empty()
                            && self.review_index < self.review_requests.len() - 1
                        {
                            self.review_index += 1;
                        }
                    }
                    HomeSection::MyPrs => {
                        if !self.my_prs.is_empty() && self.my_pr_index < self.my_prs.len() - 1 {
                            self.my_pr_index += 1;
                        }
                    }
                },
                Screen::RepoList => {
                    if !self.repos.is_empty() && self.repo_index < self.repos.len() - 1 {
                        self.repo_index += 1;
                    }
                }
                Screen::RepoView => match self.repo_tab {
                    RepoTab::PullRequests => {
                        if !self.prs.is_empty() && self.pr_index < self.prs.len() - 1 {
                            self.pr_index += 1;
                        }
                    }
                    RepoTab::Issues => {
                        if !self.issues.is_empty() && self.issue_index < self.issues.len() - 1 {
                            self.issue_index += 1;
                        }
                    }
                    RepoTab::Commits => {
                        if !self.commits.is_empty() && self.commit_index < self.commits.len() - 1 {
                            self.commit_index += 1;
                        }
                    }
                    RepoTab::Actions => {
                        if !self.action_runs.is_empty()
                            && self.action_index < self.action_runs.len() - 1
                        {
                            self.action_index += 1;
                        }
                    }
                },
                Screen::PrDetail | Screen::CommitDetail => {
                    self.scroll_offset += 1;
                }
            },
            Action::Select => match self.screen {
                Screen::Home => {
                    // Select a review request or my PR -> load PR detail
                    match self.home_section {
                        HomeSection::ReviewRequests => {
                            if let Some(req) = self.review_requests.get(self.review_index) {
                                let owner = req.repo_owner.clone();
                                let repo = req.repo_name.clone();
                                let number = req.pr_number;
                                self.current_repo = Some((owner.clone(), repo.clone()));
                                self.spawn_load_pr_detail(owner, repo, number);
                            }
                        }
                        HomeSection::MyPrs => {
                            if let Some(pr) = self.my_prs.get(self.my_pr_index) {
                                let owner = pr.repo_owner.clone();
                                let repo = pr.repo_name.clone();
                                let number = pr.number;
                                self.current_repo = Some((owner.clone(), repo.clone()));
                                self.spawn_load_pr_detail(owner, repo, number);
                            }
                        }
                    }
                }
                Screen::RepoList => {
                    if let Some(repo) = self.repos.get(self.repo_index) {
                        let owner = repo.owner.clone();
                        let name = repo.name.clone();
                        self.current_repo = Some((owner.clone(), name.clone()));
                        self.screen = Screen::RepoView;
                        self.repo_tab = RepoTab::PullRequests;
                        // Load PRs for this repo
                        self.spawn_load_prs(owner, name);
                    }
                }
                Screen::RepoView => {
                    // In RepoView, Enter drills into the selected item
                    if let Some((owner, repo)) = &self.current_repo {
                        match self.repo_tab {
                            RepoTab::PullRequests => {
                                if let Some(pr) = self.prs.get(self.pr_index) {
                                    self.spawn_load_pr_detail(owner.clone(), repo.clone(), pr.number);
                                }
                            }
                            RepoTab::Issues => {
                                // TODO: Issue detail view
                            }
                            RepoTab::Commits => {
                                if let Some(commit) = self.commits.get(self.commit_index) {
                                    self.spawn_load_commit_detail(
                                        owner.clone(),
                                        repo.clone(),
                                        commit.sha.clone(),
                                    );
                                }
                            }
                            RepoTab::Actions => {
                                // TODO: Action run detail view
                            }
                        }
                    }
                }
                Screen::PrDetail | Screen::CommitDetail => {}
            },

            // Home screen actions
            Action::LoadHome => {
                self.loading = true;
                self.spawn_load_home();
            }
            Action::HomeLoaded {
                review_requests,
                my_prs,
            } => {
                self.loading = false;
                self.review_requests = review_requests;
                self.my_prs = my_prs;
                self.review_index = 0;
                self.my_pr_index = 0;
                self.screen = Screen::Home;
            }
            Action::SwitchHomeSection => {
                self.home_section = match self.home_section {
                    HomeSection::ReviewRequests => HomeSection::MyPrs,
                    HomeSection::MyPrs => HomeSection::ReviewRequests,
                };
            }

            // Navigation actions
            Action::SwitchRepoTab(tab) => {
                self.repo_tab = tab;
                // Load content for the new tab if needed
                if let Some((owner, repo)) = &self.current_repo {
                    self.loading = true;
                    match tab {
                        RepoTab::PullRequests => {
                            self.spawn_load_prs(owner.clone(), repo.clone());
                        }
                        RepoTab::Issues => {
                            self.spawn_load_issues(owner.clone(), repo.clone());
                        }
                        RepoTab::Commits => {
                            self.spawn_load_commits(owner.clone(), repo.clone());
                        }
                        RepoTab::Actions => {
                            self.spawn_load_action_runs(owner.clone(), repo.clone());
                        }
                    }
                }
            }

            // Repo list
            Action::LoadRepos => {
                self.loading = true;
                self.spawn_load_repos();
                self.screen = Screen::RepoList;
            }
            Action::ReposLoaded(repos) => {
                self.loading = false;
                self.repos = repos;
                self.repo_index = 0;
            }

            // PR operations
            Action::PrsLoaded(prs) => {
                self.loading = false;
                self.prs = prs;
                self.pr_index = 0;
            }
            Action::PrDetailLoaded(pr) => {
                self.loading = false;
                self.prev_screen = Some(self.screen);
                self.current_pr = Some(*pr);
                self.scroll_offset = 0;
                self.screen = Screen::PrDetail;
            }

            // Issues
            Action::IssuesLoaded(issues) => {
                self.loading = false;
                self.issues = issues;
                self.issue_index = 0;
            }

            // Commits
            Action::CommitsLoaded(commits) => {
                self.loading = false;
                self.commits = commits;
                self.commit_index = 0;
            }
            Action::CommitDetailLoaded(commit) => {
                self.loading = false;
                self.prev_screen = Some(self.screen);
                self.current_commit = Some(*commit);
                self.scroll_offset = 0;
                self.screen = Screen::CommitDetail;
            }

            // Actions (workflow runs)
            Action::ActionRunsLoaded(runs) => {
                self.loading = false;
                self.action_runs = runs;
                self.action_index = 0;
            }

            Action::Error(msg) => {
                self.loading = false;
                self.error = Some(msg);
            }
            Action::None => {}
        }
    }

    fn spawn_load_home(&self) {
        let tx = self.action_tx.clone();
        let github = Arc::clone(&self.github);
        tokio::spawn(async move {
            // First get the current user
            let username = match github.get_current_user().await {
                Ok(u) => u,
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                    return;
                }
            };

            // Fetch review requests and my PRs in parallel
            let (review_result, my_prs_result) = tokio::join!(
                github.list_review_requests(&username),
                github.list_my_prs(&username)
            );

            match (review_result, my_prs_result) {
                (Ok(review_requests), Ok(my_prs)) => {
                    tx.send(Action::HomeLoaded {
                        review_requests,
                        my_prs,
                    })
                    .ok();
                }
                (Err(e), _) | (_, Err(e)) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_repos(&self) {
        let tx = self.action_tx.clone();
        let github = Arc::clone(&self.github);
        tokio::spawn(async move {
            match github.list_repos().await {
                Ok(repos) => {
                    tx.send(Action::ReposLoaded(repos)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_prs(&self, owner: String, repo: String) {
        let tx = self.action_tx.clone();
        let github = Arc::clone(&self.github);
        tokio::spawn(async move {
            match github.list_prs(&owner, &repo).await {
                Ok(prs) => {
                    tx.send(Action::PrsLoaded(prs)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_pr_detail(&self, owner: String, repo: String, number: u64) {
        let tx = self.action_tx.clone();
        let github = Arc::clone(&self.github);
        tokio::spawn(async move {
            match github.get_pr(&owner, &repo, number).await {
                Ok(pr) => {
                    tx.send(Action::PrDetailLoaded(Box::new(pr))).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_issues(&self, owner: String, repo: String) {
        let tx = self.action_tx.clone();
        let github = Arc::clone(&self.github);
        tokio::spawn(async move {
            match github.list_issues(&owner, &repo).await {
                Ok(issues) => {
                    tx.send(Action::IssuesLoaded(issues)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_commits(&self, owner: String, repo: String) {
        let tx = self.action_tx.clone();
        let github = Arc::clone(&self.github);
        tokio::spawn(async move {
            match github.list_commits(&owner, &repo).await {
                Ok(commits) => {
                    tx.send(Action::CommitsLoaded(commits)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_action_runs(&self, owner: String, repo: String) {
        let tx = self.action_tx.clone();
        let github = Arc::clone(&self.github);
        tokio::spawn(async move {
            match github.list_action_runs(&owner, &repo).await {
                Ok(runs) => {
                    tx.send(Action::ActionRunsLoaded(runs)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_commit_detail(&self, owner: String, repo: String, sha: String) {
        let tx = self.action_tx.clone();
        let github = Arc::clone(&self.github);
        tokio::spawn(async move {
            match github.get_commit(&owner, &repo, &sha).await {
                Ok(commit) => {
                    tx.send(Action::CommitDetailLoaded(Box::new(commit))).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }
}
