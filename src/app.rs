use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use crate::action::{Action, ConfirmAction, EditorContext, RepoTab};
use crate::cache;
use crate::event::Event;
use crate::forge::Forge;
use crate::types::{
    ActionRun, Commit, CommitDetail, HomeData, Issue, MyPr, PrSummary, PullRequest, Repository,
    ReviewRequest,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Confirm,
    SelectPopup,
}

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub active: bool,
    pub match_indices: Vec<usize>,
    pub current_match: usize,
    /// For content views: (line_index, byte_start, byte_end)
    pub content_matches: Vec<(usize, usize, usize)>,
}

const PAGE_SIZE: usize = 50;
const PREFETCH_THRESHOLD: usize = 5;

#[derive(Debug, Clone)]
pub struct PaginationState {
    pub page: u32,
    pub has_more: bool,
    pub loading_more: bool,
}

impl Default for PaginationState {
    fn default() -> Self {
        Self {
            page: 1,
            has_more: false,
            loading_more: false,
        }
    }
}

pub struct App {
    pub screen: Screen,
    pub input_mode: InputMode,
    pub search: SearchState,

    // Popup state
    pub confirm_action: Option<ConfirmAction>,
    pub popup_items: Vec<String>,
    pub popup_index: usize,
    pub popup_title: String,

    // Flash message (transient success messages)
    pub flash_message: Option<(String, std::time::Instant)>,

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
    forge: Arc<dyn Forge>,
    action_tx: mpsc::UnboundedSender<Action>,
    load_id: u64,

    // Pagination state per list
    pub repos_pagination: PaginationState,
    pub prs_pagination: PaginationState,
    pub issues_pagination: PaginationState,
    pub commits_pagination: PaginationState,
    pub actions_pagination: PaginationState,
}

impl App {
    pub fn new(forge: Arc<dyn Forge>, action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self {
            screen: Screen::Home,
            input_mode: InputMode::Normal,
            search: SearchState::default(),

            // Popup
            confirm_action: None,
            popup_items: Vec::new(),
            popup_index: 0,
            popup_title: String::new(),

            // Flash
            flash_message: None,

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
            forge,
            action_tx,
            load_id: 0,

            // Pagination
            repos_pagination: PaginationState::default(),
            prs_pagination: PaginationState::default(),
            issues_pagination: PaginationState::default(),
            commits_pagination: PaginationState::default(),
            actions_pagination: PaginationState::default(),
        }
    }

    pub fn handle_event(&self, event: Event) -> Action {
        match event {
            Event::Key(key) => self.handle_key(key),
            _ => Action::None,
        }
    }

    fn handle_key(&self, key: KeyEvent) -> Action {
        match &self.input_mode {
            InputMode::Normal => self.handle_key_normal(key),
            InputMode::Search => self.handle_key_search(key),
            InputMode::Confirm => match key.code {
                KeyCode::Char('y') => Action::ConfirmYes,
                KeyCode::Char('n') | KeyCode::Esc => Action::ConfirmNo,
                _ => Action::None,
            },
            InputMode::SelectPopup => match key.code {
                KeyCode::Char('j') | KeyCode::Down => Action::PopupDown,
                KeyCode::Char('k') | KeyCode::Up => Action::PopupUp,
                KeyCode::Enter => Action::PopupSelect,
                KeyCode::Esc => Action::ConfirmNo,
                _ => Action::None,
            },
        }
    }

    fn handle_key_search(&self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => Action::ExitSearchMode,
            KeyCode::Enter => Action::SearchConfirm,
            KeyCode::Backspace => Action::SearchBackspace,
            KeyCode::Char(c) => Action::SearchInput(c),
            _ => Action::None,
        }
    }

    fn handle_key_normal(&self, key: KeyEvent) -> Action {
        use crossterm::event::KeyModifiers;

        match key.code {
            KeyCode::Char('q') => {
                if self.screen == Screen::Home {
                    Action::Quit
                } else {
                    Action::Back
                }
            }
            KeyCode::Esc => {
                if self.search.active {
                    Action::ClearSearch
                } else {
                    match self.screen {
                        Screen::Home => Action::Quit,
                        _ => Action::Back,
                    }
                }
            }

            // Search
            KeyCode::Char('/') => Action::EnterSearchMode,
            KeyCode::Char('n') if self.search.active => Action::SearchNext,
            KeyCode::Char('N') if self.search.active => Action::SearchPrev,

            // Vim navigation
            KeyCode::Char('j') | KeyCode::Down => Action::ScrollDown,
            KeyCode::Char('k') | KeyCode::Up => Action::ScrollUp,
            KeyCode::Char('g') | KeyCode::Home => Action::GoToTop,
            KeyCode::Char('G') | KeyCode::End => Action::GoToBottom,
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::PageDown,
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::PageUp,
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::PageDown,
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::PageUp,
            KeyCode::PageDown => Action::PageDown,
            KeyCode::PageUp => Action::PageUp,

            // Tab/section navigation
            KeyCode::Char('h') | KeyCode::Left => Action::PrevTab,
            KeyCode::Char('l') | KeyCode::Right => Action::NextTab,
            KeyCode::Tab => Action::NextTab,
            KeyCode::BackTab => Action::PrevTab,

            KeyCode::Enter => Action::Select,

            // Diff in pager
            KeyCode::Char('d')
                if matches!(self.screen, Screen::PrDetail | Screen::CommitDetail) =>
            {
                Action::ViewDiff
            }

            // Refresh
            KeyCode::Char('r') => Action::Refresh,

            // Open in browser / Yank URL
            KeyCode::Char('o') => Action::OpenInBrowser,
            KeyCode::Char('y') => Action::YankUrl,

            // PR mutations (PrDetail only)
            KeyCode::Char('m') if self.screen == Screen::PrDetail => Action::ShowMergeMethodSelect,
            KeyCode::Char('x')
                if matches!(self.screen, Screen::PrDetail)
                    || (self.screen == Screen::RepoView && self.repo_tab == RepoTab::Issues) =>
            {
                // Close PR or issue
                match self.screen {
                    Screen::PrDetail => {
                        if let Some(pr) = &self.current_pr {
                            Action::ShowConfirm(ConfirmAction::ClosePr(pr.number))
                        } else {
                            Action::None
                        }
                    }
                    Screen::RepoView => {
                        if let Some(issue) = self.issues.get(self.issue_index) {
                            Action::ShowConfirm(ConfirmAction::CloseIssue(issue.number))
                        } else {
                            Action::None
                        }
                    }
                    _ => Action::None,
                }
            }
            KeyCode::Char('C')
                if matches!(self.screen, Screen::PrDetail)
                    || (self.screen == Screen::RepoView && self.repo_tab == RepoTab::Issues) =>
            {
                if let Some((owner, repo)) = &self.current_repo {
                    match self.screen {
                        Screen::PrDetail => {
                            if let Some(pr) = &self.current_pr {
                                Action::SuspendForEditor(EditorContext::CommentOnPr {
                                    owner: owner.clone(),
                                    repo: repo.clone(),
                                    number: pr.number,
                                })
                            } else {
                                Action::None
                            }
                        }
                        Screen::RepoView => {
                            if let Some(issue) = self.issues.get(self.issue_index) {
                                Action::SuspendForEditor(EditorContext::CommentOnIssue {
                                    owner: owner.clone(),
                                    repo: repo.clone(),
                                    number: issue.number,
                                })
                            } else {
                                Action::None
                            }
                        }
                        _ => Action::None,
                    }
                } else {
                    Action::None
                }
            }
            KeyCode::Char('R') if self.screen == Screen::PrDetail => Action::ShowReviewSelect,

            // Repo view tab shortcuts
            KeyCode::Char('p') if self.screen == Screen::RepoView => {
                Action::SwitchRepoTab(RepoTab::PullRequests)
            }
            KeyCode::Char('i') if self.screen == Screen::RepoView => {
                Action::SwitchRepoTab(RepoTab::Issues)
            }
            KeyCode::Char('c') if self.screen == Screen::RepoView => {
                Action::SwitchRepoTab(RepoTab::Commits)
            }
            KeyCode::Char('a') if self.screen == Screen::RepoView => {
                Action::SwitchRepoTab(RepoTab::Actions)
            }
            _ => Action::None,
        }
    }

    pub fn update(&mut self, action: Action) {
        if self.error.is_some() && !matches!(action, Action::Quit | Action::Back) {
            self.error = None;
        }

        match action {
            Action::Quit => {
                self.should_quit = true;
            }
            Action::Back => match self.screen {
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
            },
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
            Action::ScrollDown => {
                match self.screen {
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
                            if !self.commits.is_empty()
                                && self.commit_index < self.commits.len() - 1
                            {
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
                        let max = self.max_scroll_offset();
                        if self.scroll_offset < max {
                            self.scroll_offset += 1;
                        }
                    }
                }
                self.check_pagination();
            }

            // Vim: go to top (gg, g, Home)
            Action::GoToTop => match self.screen {
                Screen::Home => match self.home_section {
                    HomeSection::ReviewRequests => self.review_index = 0,
                    HomeSection::MyPrs => self.my_pr_index = 0,
                },
                Screen::RepoList => self.repo_index = 0,
                Screen::RepoView => match self.repo_tab {
                    RepoTab::PullRequests => self.pr_index = 0,
                    RepoTab::Issues => self.issue_index = 0,
                    RepoTab::Commits => self.commit_index = 0,
                    RepoTab::Actions => self.action_index = 0,
                },
                Screen::PrDetail | Screen::CommitDetail => self.scroll_offset = 0,
            },

            // Vim: go to bottom (G, End)
            Action::GoToBottom => {
                match self.screen {
                    Screen::Home => match self.home_section {
                        HomeSection::ReviewRequests => {
                            if !self.review_requests.is_empty() {
                                self.review_index = self.review_requests.len() - 1;
                            }
                        }
                        HomeSection::MyPrs => {
                            if !self.my_prs.is_empty() {
                                self.my_pr_index = self.my_prs.len() - 1;
                            }
                        }
                    },
                    Screen::RepoList => {
                        if !self.repos.is_empty() {
                            self.repo_index = self.repos.len() - 1;
                        }
                    }
                    Screen::RepoView => match self.repo_tab {
                        RepoTab::PullRequests => {
                            if !self.prs.is_empty() {
                                self.pr_index = self.prs.len() - 1;
                            }
                        }
                        RepoTab::Issues => {
                            if !self.issues.is_empty() {
                                self.issue_index = self.issues.len() - 1;
                            }
                        }
                        RepoTab::Commits => {
                            if !self.commits.is_empty() {
                                self.commit_index = self.commits.len() - 1;
                            }
                        }
                        RepoTab::Actions => {
                            if !self.action_runs.is_empty() {
                                self.action_index = self.action_runs.len() - 1;
                            }
                        }
                    },
                    Screen::PrDetail | Screen::CommitDetail => {
                        self.scroll_offset = self.max_scroll_offset();
                    }
                }
                self.check_pagination();
            }

            // Vim: page up (Ctrl+u, Ctrl+b, PageUp)
            Action::PageUp => {
                let page_size = 10;
                match self.screen {
                    Screen::Home => match self.home_section {
                        HomeSection::ReviewRequests => {
                            self.review_index = self.review_index.saturating_sub(page_size);
                        }
                        HomeSection::MyPrs => {
                            self.my_pr_index = self.my_pr_index.saturating_sub(page_size);
                        }
                    },
                    Screen::RepoList => {
                        self.repo_index = self.repo_index.saturating_sub(page_size);
                    }
                    Screen::RepoView => match self.repo_tab {
                        RepoTab::PullRequests => {
                            self.pr_index = self.pr_index.saturating_sub(page_size);
                        }
                        RepoTab::Issues => {
                            self.issue_index = self.issue_index.saturating_sub(page_size);
                        }
                        RepoTab::Commits => {
                            self.commit_index = self.commit_index.saturating_sub(page_size);
                        }
                        RepoTab::Actions => {
                            self.action_index = self.action_index.saturating_sub(page_size);
                        }
                    },
                    Screen::PrDetail | Screen::CommitDetail => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
                    }
                }
            }

            // Vim: page down (Ctrl+d, Ctrl+f, PageDown)
            Action::PageDown => {
                let page_size = 10;
                match self.screen {
                    Screen::Home => match self.home_section {
                        HomeSection::ReviewRequests => {
                            let max = self.review_requests.len().saturating_sub(1);
                            self.review_index = (self.review_index + page_size).min(max);
                        }
                        HomeSection::MyPrs => {
                            let max = self.my_prs.len().saturating_sub(1);
                            self.my_pr_index = (self.my_pr_index + page_size).min(max);
                        }
                    },
                    Screen::RepoList => {
                        let max = self.repos.len().saturating_sub(1);
                        self.repo_index = (self.repo_index + page_size).min(max);
                    }
                    Screen::RepoView => match self.repo_tab {
                        RepoTab::PullRequests => {
                            let max = self.prs.len().saturating_sub(1);
                            self.pr_index = (self.pr_index + page_size).min(max);
                        }
                        RepoTab::Issues => {
                            let max = self.issues.len().saturating_sub(1);
                            self.issue_index = (self.issue_index + page_size).min(max);
                        }
                        RepoTab::Commits => {
                            let max = self.commits.len().saturating_sub(1);
                            self.commit_index = (self.commit_index + page_size).min(max);
                        }
                        RepoTab::Actions => {
                            let max = self.action_runs.len().saturating_sub(1);
                            self.action_index = (self.action_index + page_size).min(max);
                        }
                    },
                    Screen::PrDetail | Screen::CommitDetail => {
                        let max = self.max_scroll_offset();
                        self.scroll_offset = (self.scroll_offset + page_size).min(max);
                    }
                }
                self.check_pagination();
            }

            // Tab navigation (h/l, Tab/Shift+Tab, Left/Right)
            Action::NextTab => match self.screen {
                Screen::Home => {
                    self.home_section = match self.home_section {
                        HomeSection::ReviewRequests => HomeSection::MyPrs,
                        HomeSection::MyPrs => HomeSection::ReviewRequests,
                    };
                }
                Screen::RepoView => {
                    let next = match self.repo_tab {
                        RepoTab::PullRequests => RepoTab::Issues,
                        RepoTab::Issues => RepoTab::Commits,
                        RepoTab::Commits => RepoTab::Actions,
                        RepoTab::Actions => RepoTab::PullRequests,
                    };
                    self.repo_tab = next;
                    match next {
                        RepoTab::PullRequests => self.pr_index = 0,
                        RepoTab::Issues => self.issue_index = 0,
                        RepoTab::Commits => self.commit_index = 0,
                        RepoTab::Actions => self.action_index = 0,
                    }
                    self.load_id += 1;
                    if let Some((owner, repo)) = &self.current_repo {
                        self.loading = true;
                        match next {
                            RepoTab::PullRequests => {
                                self.spawn_load_prs(owner.clone(), repo.clone(), self.load_id)
                            }
                            RepoTab::Issues => {
                                self.spawn_load_issues(owner.clone(), repo.clone(), self.load_id)
                            }
                            RepoTab::Commits => {
                                self.spawn_load_commits(owner.clone(), repo.clone(), self.load_id)
                            }
                            RepoTab::Actions => self.spawn_load_action_runs(
                                owner.clone(),
                                repo.clone(),
                                self.load_id,
                            ),
                        }
                    }
                }
                _ => {}
            },
            Action::PrevTab => match self.screen {
                Screen::Home => {
                    self.home_section = match self.home_section {
                        HomeSection::ReviewRequests => HomeSection::MyPrs,
                        HomeSection::MyPrs => HomeSection::ReviewRequests,
                    };
                }
                Screen::RepoView => {
                    let prev = match self.repo_tab {
                        RepoTab::PullRequests => RepoTab::Actions,
                        RepoTab::Issues => RepoTab::PullRequests,
                        RepoTab::Commits => RepoTab::Issues,
                        RepoTab::Actions => RepoTab::Commits,
                    };
                    self.repo_tab = prev;
                    match prev {
                        RepoTab::PullRequests => self.pr_index = 0,
                        RepoTab::Issues => self.issue_index = 0,
                        RepoTab::Commits => self.commit_index = 0,
                        RepoTab::Actions => self.action_index = 0,
                    }
                    self.load_id += 1;
                    if let Some((owner, repo)) = &self.current_repo {
                        self.loading = true;
                        match prev {
                            RepoTab::PullRequests => {
                                self.spawn_load_prs(owner.clone(), repo.clone(), self.load_id)
                            }
                            RepoTab::Issues => {
                                self.spawn_load_issues(owner.clone(), repo.clone(), self.load_id)
                            }
                            RepoTab::Commits => {
                                self.spawn_load_commits(owner.clone(), repo.clone(), self.load_id)
                            }
                            RepoTab::Actions => self.spawn_load_action_runs(
                                owner.clone(),
                                repo.clone(),
                                self.load_id,
                            ),
                        }
                    }
                }
                _ => {}
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
                                self.load_id += 1;
                                self.spawn_load_pr_detail(owner, repo, number, self.load_id);
                            }
                        }
                        HomeSection::MyPrs => {
                            if let Some(pr) = self.my_prs.get(self.my_pr_index) {
                                let owner = pr.repo_owner.clone();
                                let repo = pr.repo_name.clone();
                                let number = pr.number;
                                self.current_repo = Some((owner.clone(), repo.clone()));
                                self.load_id += 1;
                                self.spawn_load_pr_detail(owner, repo, number, self.load_id);
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
                        self.pr_index = 0;
                        self.issue_index = 0;
                        self.commit_index = 0;
                        self.action_index = 0;
                        self.load_id += 1;
                        // Load PRs for this repo
                        self.spawn_load_prs(owner, name, self.load_id);
                    }
                }
                Screen::RepoView => {
                    // In RepoView, Enter drills into the selected item
                    match self.repo_tab {
                        RepoTab::PullRequests => {
                            if let Some(pr) = self.prs.get(self.pr_index) {
                                let number = pr.number;
                                if let Some((owner, repo)) = &self.current_repo {
                                    let owner = owner.clone();
                                    let repo = repo.clone();
                                    self.load_id += 1;
                                    self.spawn_load_pr_detail(owner, repo, number, self.load_id);
                                }
                            }
                        }
                        RepoTab::Issues => {
                            // TODO: Issue detail view
                        }
                        RepoTab::Commits => {
                            if let Some(commit) = self.commits.get(self.commit_index) {
                                let sha = commit.sha.clone();
                                if let Some((owner, repo)) = &self.current_repo {
                                    let owner = owner.clone();
                                    let repo = repo.clone();
                                    self.load_id += 1;
                                    self.spawn_load_commit_detail(owner, repo, sha, self.load_id);
                                }
                            }
                        }
                        RepoTab::Actions => {
                            // TODO: Action run detail view
                        }
                    }
                }
                Screen::PrDetail | Screen::CommitDetail => {}
            },

            // Home screen actions
            Action::LoadHome => {
                self.loading = true;
                self.load_id += 1;
                self.spawn_load_home(self.load_id);
            }
            Action::HomeLoaded {
                review_requests,
                my_prs,
                load_id,
            } => {
                if load_id == self.load_id {
                    self.loading = false;
                    self.review_requests = review_requests;
                    self.my_prs = my_prs;
                    self.review_index = self
                        .review_index
                        .min(self.review_requests.len().saturating_sub(1));
                    self.my_pr_index = self.my_pr_index.min(self.my_prs.len().saturating_sub(1));
                }
            }
            // Navigation actions
            Action::SwitchRepoTab(tab) => {
                self.repo_tab = tab;
                // Reset index for the new tab
                match tab {
                    RepoTab::PullRequests => self.pr_index = 0,
                    RepoTab::Issues => self.issue_index = 0,
                    RepoTab::Commits => self.commit_index = 0,
                    RepoTab::Actions => self.action_index = 0,
                }
                // Load content for the new tab if needed
                self.load_id += 1;
                if let Some((owner, repo)) = &self.current_repo {
                    self.loading = true;
                    match tab {
                        RepoTab::PullRequests => {
                            self.spawn_load_prs(owner.clone(), repo.clone(), self.load_id);
                        }
                        RepoTab::Issues => {
                            self.spawn_load_issues(owner.clone(), repo.clone(), self.load_id);
                        }
                        RepoTab::Commits => {
                            self.spawn_load_commits(owner.clone(), repo.clone(), self.load_id);
                        }
                        RepoTab::Actions => {
                            self.spawn_load_action_runs(owner.clone(), repo.clone(), self.load_id);
                        }
                    }
                }
            }

            // Repo list
            Action::ReposLoaded(repos, load_id) => {
                if load_id == self.load_id {
                    self.loading = false;
                    self.repos_pagination = PaginationState {
                        page: 1,
                        has_more: repos.len() == PAGE_SIZE,
                        loading_more: false,
                    };
                    self.repos = repos;
                    self.repo_index = self.repo_index.min(self.repos.len().saturating_sub(1));
                }
            }

            // PR operations
            Action::PrsLoaded(prs, load_id) => {
                if load_id == self.load_id {
                    self.loading = false;
                    self.prs_pagination = PaginationState {
                        page: 1,
                        has_more: prs.len() == PAGE_SIZE,
                        loading_more: false,
                    };
                    self.prs = prs;
                    self.pr_index = self.pr_index.min(self.prs.len().saturating_sub(1));
                }
            }
            Action::PrDetailLoaded(pr, load_id) => {
                if load_id == self.load_id {
                    self.loading = false;
                    self.current_pr = Some(*pr);
                    // Only transition screen on first load, not background refresh
                    if self.screen != Screen::PrDetail {
                        self.prev_screen = Some(self.screen);
                        self.scroll_offset = 0;
                        self.screen = Screen::PrDetail;
                    }
                }
            }

            // Issues
            Action::IssuesLoaded(issues, load_id) => {
                if load_id == self.load_id {
                    self.loading = false;
                    self.issues_pagination = PaginationState {
                        page: 1,
                        has_more: issues.len() == PAGE_SIZE,
                        loading_more: false,
                    };
                    self.issues = issues;
                    self.issue_index = self.issue_index.min(self.issues.len().saturating_sub(1));
                }
            }

            // Commits
            Action::CommitsLoaded(commits, load_id) => {
                if load_id == self.load_id {
                    self.loading = false;
                    self.commits_pagination = PaginationState {
                        page: 1,
                        has_more: commits.len() == PAGE_SIZE,
                        loading_more: false,
                    };
                    self.commits = commits;
                    self.commit_index = self.commit_index.min(self.commits.len().saturating_sub(1));
                }
            }
            Action::CommitDetailLoaded(commit, load_id) => {
                if load_id == self.load_id {
                    self.loading = false;
                    self.current_commit = Some(*commit);
                    // Only transition screen on first load, not background refresh
                    if self.screen != Screen::CommitDetail {
                        self.prev_screen = Some(self.screen);
                        self.scroll_offset = 0;
                        self.screen = Screen::CommitDetail;
                    }
                }
            }

            // Actions (workflow runs)
            Action::ActionRunsLoaded(runs, load_id) => {
                if load_id == self.load_id {
                    self.loading = false;
                    self.actions_pagination = PaginationState {
                        page: 1,
                        has_more: runs.len() == PAGE_SIZE,
                        loading_more: false,
                    };
                    self.action_runs = runs;
                    self.action_index = self
                        .action_index
                        .min(self.action_runs.len().saturating_sub(1));
                }
            }

            // Pagination append handlers
            Action::ReposAppended(new_repos, load_id) => {
                if load_id == self.load_id {
                    self.repos_pagination.loading_more = false;
                    self.repos_pagination.has_more = new_repos.len() == PAGE_SIZE;
                    self.repos.extend(new_repos);
                }
            }
            Action::PrsAppended(new_prs, load_id) => {
                if load_id == self.load_id {
                    self.prs_pagination.loading_more = false;
                    self.prs_pagination.has_more = new_prs.len() == PAGE_SIZE;
                    self.prs.extend(new_prs);
                }
            }
            Action::IssuesAppended(new_issues, load_id) => {
                if load_id == self.load_id {
                    self.issues_pagination.loading_more = false;
                    self.issues_pagination.has_more = new_issues.len() == PAGE_SIZE;
                    self.issues.extend(new_issues);
                }
            }
            Action::CommitsAppended(new_commits, load_id) => {
                if load_id == self.load_id {
                    self.commits_pagination.loading_more = false;
                    self.commits_pagination.has_more = new_commits.len() == PAGE_SIZE;
                    self.commits.extend(new_commits);
                }
            }
            Action::ActionRunsAppended(new_runs, load_id) => {
                if load_id == self.load_id {
                    self.actions_pagination.loading_more = false;
                    self.actions_pagination.has_more = new_runs.len() == PAGE_SIZE;
                    self.action_runs.extend(new_runs);
                }
            }

            // Search actions
            Action::EnterSearchMode => {
                self.input_mode = InputMode::Search;
                self.search.query.clear();
                self.search.match_indices.clear();
                self.search.content_matches.clear();
                self.search.current_match = 0;
            }
            Action::ExitSearchMode => {
                self.input_mode = InputMode::Normal;
                // Don't clear results - keep them active for n/N navigation
                if !self.search.query.is_empty() {
                    self.search.active = true;
                }
            }
            Action::SearchInput(c) => {
                self.search.query.push(c);
                self.recompute_search_matches();
            }
            Action::SearchBackspace => {
                self.search.query.pop();
                if self.search.query.is_empty() {
                    self.search.match_indices.clear();
                    self.search.content_matches.clear();
                    self.search.active = false;
                } else {
                    self.recompute_search_matches();
                }
            }
            Action::SearchConfirm => {
                self.input_mode = InputMode::Normal;
                if !self.search.query.is_empty() {
                    self.search.active = true;
                    self.jump_to_current_match();
                }
            }
            Action::SearchNext => {
                if !self.search.match_indices.is_empty() {
                    self.search.current_match =
                        (self.search.current_match + 1) % self.search.match_indices.len();
                    self.jump_to_current_match();
                } else if !self.search.content_matches.is_empty() {
                    self.search.current_match =
                        (self.search.current_match + 1) % self.search.content_matches.len();
                    self.jump_to_content_match();
                }
            }
            Action::SearchPrev => {
                if !self.search.match_indices.is_empty() {
                    self.search.current_match = if self.search.current_match == 0 {
                        self.search.match_indices.len() - 1
                    } else {
                        self.search.current_match - 1
                    };
                    self.jump_to_current_match();
                } else if !self.search.content_matches.is_empty() {
                    self.search.current_match = if self.search.current_match == 0 {
                        self.search.content_matches.len() - 1
                    } else {
                        self.search.current_match - 1
                    };
                    self.jump_to_content_match();
                }
            }
            Action::ClearSearch => {
                self.search = SearchState::default();
            }

            // Pager
            Action::ViewDiff => {
                if let Some((owner, repo)) = &self.current_repo {
                    match self.screen {
                        Screen::PrDetail => {
                            if let Some(pr) = &self.current_pr {
                                let number = pr.number;
                                self.spawn_load_pr_diff(owner.clone(), repo.clone(), number);
                            }
                        }
                        Screen::CommitDetail => {
                            if let Some(commit) = &self.current_commit {
                                let mut diff = String::new();
                                for file in &commit.files {
                                    if let Some(patch) = &file.patch {
                                        diff.push_str(&format!(
                                            "diff --git a/{f} b/{f}\n",
                                            f = file.filename
                                        ));
                                        diff.push_str(patch);
                                        diff.push('\n');
                                    }
                                }
                                let _ = self.action_tx.send(Action::SuspendForPager(diff));
                            }
                        }
                        _ => {}
                    }
                }
            }
            Action::SuspendForPager(_) => {
                // Handled in main loop
            }

            // Refresh
            Action::Refresh => {
                self.load_id += 1;
                match self.screen {
                    Screen::Home => {
                        // On Home, r navigates to repo browser
                        self.loading = true;
                        self.spawn_load_repos(self.load_id);
                        self.screen = Screen::RepoList;
                    }
                    Screen::RepoList => {
                        self.loading = true;
                        self.spawn_load_repos(self.load_id);
                    }
                    Screen::RepoView => {
                        if let Some((owner, repo)) = &self.current_repo {
                            self.loading = true;
                            match self.repo_tab {
                                RepoTab::PullRequests => {
                                    self.spawn_load_prs(owner.clone(), repo.clone(), self.load_id)
                                }
                                RepoTab::Issues => self.spawn_load_issues(
                                    owner.clone(),
                                    repo.clone(),
                                    self.load_id,
                                ),
                                RepoTab::Commits => self.spawn_load_commits(
                                    owner.clone(),
                                    repo.clone(),
                                    self.load_id,
                                ),
                                RepoTab::Actions => self.spawn_load_action_runs(
                                    owner.clone(),
                                    repo.clone(),
                                    self.load_id,
                                ),
                            }
                        }
                    }
                    Screen::PrDetail => {
                        if let Some((owner, repo)) = &self.current_repo {
                            if let Some(pr) = &self.current_pr {
                                self.spawn_load_pr_detail(
                                    owner.clone(),
                                    repo.clone(),
                                    pr.number,
                                    self.load_id,
                                );
                            }
                        }
                    }
                    Screen::CommitDetail => {
                        if let Some((owner, repo)) = &self.current_repo {
                            if let Some(commit) = &self.current_commit {
                                self.spawn_load_commit_detail(
                                    owner.clone(),
                                    repo.clone(),
                                    commit.sha.clone(),
                                    self.load_id,
                                );
                            }
                        }
                    }
                }
            }

            // Open in browser
            Action::OpenInBrowser => {
                if let Some(url) = self.current_item_url() {
                    let _ = open::that(&url);
                }
            }

            // Yank URL
            Action::YankUrl => {
                if let Some(url) = self.current_item_url() {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if clipboard.set_text(&url).is_ok() {
                            self.flash_message =
                                Some(("URL copied!".to_string(), std::time::Instant::now()));
                        }
                    }
                }
            }

            // Popup: merge method select
            Action::ShowMergeMethodSelect => {
                self.input_mode = InputMode::SelectPopup;
                self.popup_title = "Merge Method".to_string();
                self.popup_items = vec![
                    "Merge commit".to_string(),
                    "Squash and merge".to_string(),
                    "Rebase and merge".to_string(),
                ];
                self.popup_index = 0;
            }

            // Popup: review select
            Action::ShowReviewSelect => {
                self.input_mode = InputMode::SelectPopup;
                self.popup_title = "Submit Review".to_string();
                self.popup_items = vec![
                    "Approve".to_string(),
                    "Request changes".to_string(),
                    "Comment".to_string(),
                ];
                self.popup_index = 0;
            }

            // Confirm dialog
            Action::ShowConfirm(confirm_action) => {
                self.confirm_action = Some(confirm_action);
                self.input_mode = InputMode::Confirm;
            }

            Action::ConfirmYes => {
                if let Some(confirm) = self.confirm_action.take() {
                    self.input_mode = InputMode::Normal;
                    match confirm {
                        ConfirmAction::ClosePr(number) => {
                            if let Some((owner, repo)) = &self.current_repo {
                                self.spawn_close_pr(owner.clone(), repo.clone(), number);
                            }
                        }
                        ConfirmAction::MergePr { number, method } => {
                            if let Some((owner, repo)) = &self.current_repo {
                                self.spawn_merge_pr(owner.clone(), repo.clone(), number, method);
                            }
                        }
                        ConfirmAction::CloseIssue(number) => {
                            if let Some((owner, repo)) = &self.current_repo {
                                self.spawn_close_issue(owner.clone(), repo.clone(), number);
                            }
                        }
                    }
                }
            }

            Action::ConfirmNo => {
                self.confirm_action = None;
                self.input_mode = InputMode::Normal;
            }

            // Popup navigation
            Action::PopupUp => {
                if self.popup_index > 0 {
                    self.popup_index -= 1;
                }
            }
            Action::PopupDown => {
                if self.popup_index < self.popup_items.len().saturating_sub(1) {
                    self.popup_index += 1;
                }
            }
            Action::PopupSelect => {
                self.input_mode = InputMode::Normal;
                // Determine what the popup was for based on title
                if self.popup_title == "Merge Method" {
                    if let Some(pr) = &self.current_pr {
                        let method = match self.popup_index {
                            0 => crate::types::MergeMethod::Merge,
                            1 => crate::types::MergeMethod::Squash,
                            _ => crate::types::MergeMethod::Rebase,
                        };
                        let _ = self
                            .action_tx
                            .send(Action::ShowConfirm(ConfirmAction::MergePr {
                                number: pr.number,
                                method,
                            }));
                    }
                } else if self.popup_title == "Submit Review" {
                    let event = match self.popup_index {
                        0 => crate::types::ReviewEvent::Approve,
                        1 => crate::types::ReviewEvent::RequestChanges,
                        _ => crate::types::ReviewEvent::Comment,
                    };
                    if let Some((owner, repo)) = &self.current_repo {
                        if let Some(pr) = &self.current_pr {
                            let _ = self.action_tx.send(Action::SuspendForEditor(
                                EditorContext::ReviewPr {
                                    owner: owner.clone(),
                                    repo: repo.clone(),
                                    number: pr.number,
                                    event,
                                },
                            ));
                        }
                    }
                }
            }

            // Mutation results
            Action::PrMerged => {
                self.flash_message = Some(("PR merged!".to_string(), std::time::Instant::now()));
                let _ = self.action_tx.send(Action::Back);
            }
            Action::PrClosed => {
                self.flash_message = Some(("PR closed.".to_string(), std::time::Instant::now()));
                let _ = self.action_tx.send(Action::Back);
            }
            Action::IssueClosed => {
                self.flash_message = Some(("Issue closed.".to_string(), std::time::Instant::now()));
                let _ = self.action_tx.send(Action::Refresh);
            }
            Action::CommentPosted => {
                self.flash_message =
                    Some(("Comment posted.".to_string(), std::time::Instant::now()));
            }
            Action::ReviewSubmitted => {
                self.flash_message =
                    Some(("Review submitted.".to_string(), std::time::Instant::now()));
            }

            // Editor suspend - handled in main loop
            Action::SuspendForEditor(_) => {}

            Action::Error(msg) => {
                self.loading = false;
                self.error = Some(msg);
            }
            Action::None => {}
        }

        // Clear flash messages after 3 seconds
        if let Some((_, instant)) = &self.flash_message {
            if instant.elapsed() > std::time::Duration::from_secs(3) {
                self.flash_message = None;
            }
        }
    }

    fn recompute_search_matches(&mut self) {
        let query = self.search.query.to_lowercase();
        if query.is_empty() {
            self.search.match_indices.clear();
            self.search.content_matches.clear();
            return;
        }

        match self.screen {
            Screen::Home => match self.home_section {
                HomeSection::ReviewRequests => {
                    self.search.match_indices = self
                        .review_requests
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| {
                            r.pr_title.to_lowercase().contains(&query)
                                || r.repo_name.to_lowercase().contains(&query)
                                || r.author.to_lowercase().contains(&query)
                        })
                        .map(|(i, _)| i)
                        .collect();
                }
                HomeSection::MyPrs => {
                    self.search.match_indices = self
                        .my_prs
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            p.title.to_lowercase().contains(&query)
                                || p.repo_name.to_lowercase().contains(&query)
                        })
                        .map(|(i, _)| i)
                        .collect();
                }
            },
            Screen::RepoList => {
                self.search.match_indices = self
                    .repos
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| {
                        r.name.to_lowercase().contains(&query)
                            || r.owner.to_lowercase().contains(&query)
                            || r.description
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&query)
                    })
                    .map(|(i, _)| i)
                    .collect();
            }
            Screen::RepoView => match self.repo_tab {
                RepoTab::PullRequests => {
                    self.search.match_indices = self
                        .prs
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            p.title.to_lowercase().contains(&query)
                                || p.author.to_lowercase().contains(&query)
                                || p.number.to_string().contains(&query)
                        })
                        .map(|(i, _)| i)
                        .collect();
                }
                RepoTab::Issues => {
                    self.search.match_indices = self
                        .issues
                        .iter()
                        .enumerate()
                        .filter(|(_, issue)| {
                            issue.title.to_lowercase().contains(&query)
                                || issue.author.to_lowercase().contains(&query)
                                || issue.number.to_string().contains(&query)
                        })
                        .map(|(i, _)| i)
                        .collect();
                }
                RepoTab::Commits => {
                    self.search.match_indices = self
                        .commits
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| {
                            c.message.to_lowercase().contains(&query)
                                || c.author.to_lowercase().contains(&query)
                                || c.sha.to_lowercase().contains(&query)
                        })
                        .map(|(i, _)| i)
                        .collect();
                }
                RepoTab::Actions => {
                    self.search.match_indices = self
                        .action_runs
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| {
                            r.name.to_lowercase().contains(&query)
                                || r.branch.to_lowercase().contains(&query)
                        })
                        .map(|(i, _)| i)
                        .collect();
                }
            },
            Screen::PrDetail => {
                self.search.content_matches.clear();
                if let Some(pr) = &self.current_pr {
                    let body = pr.body.as_deref().unwrap_or("");
                    for (line_idx, line) in body.lines().enumerate() {
                        let lower = line.to_lowercase();
                        let mut start = 0;
                        while let Some(pos) = lower[start..].find(&query) {
                            let byte_start = start + pos;
                            let byte_end = byte_start + query.len();
                            self.search
                                .content_matches
                                .push((line_idx, byte_start, byte_end));
                            start = byte_end;
                        }
                    }
                }
            }
            Screen::CommitDetail => {
                self.search.content_matches.clear();
                if let Some(commit) = &self.current_commit {
                    let mut line_idx = 0;
                    // Skip header lines (same structure as render)
                    line_idx += 5; // header, blank, stats, blank, "Message:"
                    for msg_line in commit.message.lines() {
                        let lower = msg_line.to_lowercase();
                        let mut start = 0;
                        while let Some(pos) = lower[start..].find(&query) {
                            let byte_start = start + pos;
                            let byte_end = byte_start + query.len();
                            self.search
                                .content_matches
                                .push((line_idx, byte_start, byte_end));
                            start = byte_end;
                        }
                        line_idx += 1;
                    }
                    line_idx += 1; // blank after message
                    for file in &commit.files {
                        line_idx += 1; // file header
                        if let Some(patch) = &file.patch {
                            for patch_line in patch.lines() {
                                let lower = patch_line.to_lowercase();
                                let mut start = 0;
                                while let Some(pos) = lower[start..].find(&query) {
                                    let byte_start = start + pos;
                                    let byte_end = byte_start + query.len();
                                    self.search
                                        .content_matches
                                        .push((line_idx, byte_start, byte_end));
                                    start = byte_end;
                                }
                                line_idx += 1;
                            }
                        }
                        line_idx += 1; // blank after file
                    }
                }
            }
        }

        self.search.current_match = 0;
    }

    fn jump_to_current_match(&mut self) {
        if let Some(&idx) = self.search.match_indices.get(self.search.current_match) {
            match self.screen {
                Screen::Home => match self.home_section {
                    HomeSection::ReviewRequests => self.review_index = idx,
                    HomeSection::MyPrs => self.my_pr_index = idx,
                },
                Screen::RepoList => self.repo_index = idx,
                Screen::RepoView => match self.repo_tab {
                    RepoTab::PullRequests => self.pr_index = idx,
                    RepoTab::Issues => self.issue_index = idx,
                    RepoTab::Commits => self.commit_index = idx,
                    RepoTab::Actions => self.action_index = idx,
                },
                _ => {}
            }
        }
    }

    fn jump_to_content_match(&mut self) {
        if let Some(&(line_idx, _, _)) = self.search.content_matches.get(self.search.current_match)
        {
            self.scroll_offset = line_idx.saturating_sub(5);
        }
    }

    fn spawn_load_home(&self, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);

        // Serve from cache immediately
        if let Some(cached) = cache::read::<HomeData>("home") {
            tx.send(Action::HomeLoaded {
                review_requests: cached.review_requests,
                my_prs: cached.my_prs,
                load_id,
            })
            .ok();
        }

        // Background refresh
        tokio::spawn(async move {
            let username = match forge.get_current_user().await {
                Ok(u) => u,
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                    return;
                }
            };

            let (review_result, my_prs_result) = tokio::join!(
                forge.list_review_requests(&username),
                forge.list_my_prs(&username)
            );

            match (review_result, my_prs_result) {
                (Ok(review_requests), Ok(my_prs)) => {
                    cache::write(
                        "home",
                        &HomeData {
                            review_requests: review_requests.clone(),
                            my_prs: my_prs.clone(),
                        },
                    );
                    tx.send(Action::HomeLoaded {
                        review_requests,
                        my_prs,
                        load_id,
                    })
                    .ok();
                }
                (Err(e), _) | (_, Err(e)) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_repos(&self, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);

        if let Some(cached) = cache::read::<Vec<Repository>>("repos") {
            tx.send(Action::ReposLoaded(cached, load_id)).ok();
        }

        tokio::spawn(async move {
            match forge.list_repos(1).await {
                Ok(repos) => {
                    cache::write("repos", &repos);
                    tx.send(Action::ReposLoaded(repos, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_prs(&self, owner: String, repo: String, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        let key = format!("prs_{}", cache::repo_key(&owner, &repo));

        if let Some(cached) = cache::read::<Vec<PrSummary>>(&key) {
            tx.send(Action::PrsLoaded(cached, load_id)).ok();
        }

        tokio::spawn(async move {
            match forge.list_prs(&owner, &repo, 1).await {
                Ok(prs) => {
                    cache::write(&key, &prs);
                    tx.send(Action::PrsLoaded(prs, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_pr_detail(&self, owner: String, repo: String, number: u64, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        let key = format!("pr_{}_{}", cache::repo_key(&owner, &repo), number);

        if let Some(cached) = cache::read::<PullRequest>(&key) {
            tx.send(Action::PrDetailLoaded(Box::new(cached), load_id))
                .ok();
        }

        tokio::spawn(async move {
            match forge.get_pr(&owner, &repo, number).await {
                Ok(pr) => {
                    cache::write(&key, &pr);
                    tx.send(Action::PrDetailLoaded(Box::new(pr), load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_issues(&self, owner: String, repo: String, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        let key = format!("issues_{}", cache::repo_key(&owner, &repo));

        if let Some(cached) = cache::read::<Vec<Issue>>(&key) {
            tx.send(Action::IssuesLoaded(cached, load_id)).ok();
        }

        tokio::spawn(async move {
            match forge.list_issues(&owner, &repo, 1).await {
                Ok(issues) => {
                    cache::write(&key, &issues);
                    tx.send(Action::IssuesLoaded(issues, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_commits(&self, owner: String, repo: String, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        let key = format!("commits_{}", cache::repo_key(&owner, &repo));

        if let Some(cached) = cache::read::<Vec<Commit>>(&key) {
            tx.send(Action::CommitsLoaded(cached, load_id)).ok();
        }

        tokio::spawn(async move {
            match forge.list_commits(&owner, &repo, 1).await {
                Ok(commits) => {
                    cache::write(&key, &commits);
                    tx.send(Action::CommitsLoaded(commits, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_action_runs(&self, owner: String, repo: String, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        let key = format!("actions_{}", cache::repo_key(&owner, &repo));

        if let Some(cached) = cache::read::<Vec<ActionRun>>(&key) {
            tx.send(Action::ActionRunsLoaded(cached, load_id)).ok();
        }

        tokio::spawn(async move {
            match forge.list_action_runs(&owner, &repo, 1).await {
                Ok(runs) => {
                    cache::write(&key, &runs);
                    tx.send(Action::ActionRunsLoaded(runs, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    // Pagination: spawn methods for loading next pages (no cache)

    fn spawn_load_repos_page(&self, page: u32, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.list_repos(page).await {
                Ok(repos) => {
                    tx.send(Action::ReposAppended(repos, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_prs_page(&self, owner: String, repo: String, page: u32, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.list_prs(&owner, &repo, page).await {
                Ok(prs) => {
                    tx.send(Action::PrsAppended(prs, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_issues_page(&self, owner: String, repo: String, page: u32, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.list_issues(&owner, &repo, page).await {
                Ok(issues) => {
                    tx.send(Action::IssuesAppended(issues, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_commits_page(&self, owner: String, repo: String, page: u32, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.list_commits(&owner, &repo, page).await {
                Ok(commits) => {
                    tx.send(Action::CommitsAppended(commits, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_action_runs_page(&self, owner: String, repo: String, page: u32, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.list_action_runs(&owner, &repo, page).await {
                Ok(runs) => {
                    tx.send(Action::ActionRunsAppended(runs, load_id)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    /// Check if we need to fetch the next page and trigger if so
    fn check_pagination(&mut self) {
        match self.screen {
            Screen::RepoList => {
                if self.repo_index >= self.repos.len().saturating_sub(PREFETCH_THRESHOLD)
                    && self.repos_pagination.has_more
                    && !self.repos_pagination.loading_more
                {
                    self.repos_pagination.loading_more = true;
                    self.repos_pagination.page += 1;
                    self.spawn_load_repos_page(self.repos_pagination.page, self.load_id);
                }
            }
            Screen::RepoView => match self.repo_tab {
                RepoTab::PullRequests => {
                    if self.pr_index >= self.prs.len().saturating_sub(PREFETCH_THRESHOLD)
                        && self.prs_pagination.has_more
                        && !self.prs_pagination.loading_more
                    {
                        self.prs_pagination.loading_more = true;
                        self.prs_pagination.page += 1;
                        if let Some((owner, repo)) = &self.current_repo {
                            self.spawn_load_prs_page(
                                owner.clone(),
                                repo.clone(),
                                self.prs_pagination.page,
                                self.load_id,
                            );
                        }
                    }
                }
                RepoTab::Issues => {
                    if self.issue_index >= self.issues.len().saturating_sub(PREFETCH_THRESHOLD)
                        && self.issues_pagination.has_more
                        && !self.issues_pagination.loading_more
                    {
                        self.issues_pagination.loading_more = true;
                        self.issues_pagination.page += 1;
                        if let Some((owner, repo)) = &self.current_repo {
                            self.spawn_load_issues_page(
                                owner.clone(),
                                repo.clone(),
                                self.issues_pagination.page,
                                self.load_id,
                            );
                        }
                    }
                }
                RepoTab::Commits => {
                    if self.commit_index >= self.commits.len().saturating_sub(PREFETCH_THRESHOLD)
                        && self.commits_pagination.has_more
                        && !self.commits_pagination.loading_more
                    {
                        self.commits_pagination.loading_more = true;
                        self.commits_pagination.page += 1;
                        if let Some((owner, repo)) = &self.current_repo {
                            self.spawn_load_commits_page(
                                owner.clone(),
                                repo.clone(),
                                self.commits_pagination.page,
                                self.load_id,
                            );
                        }
                    }
                }
                RepoTab::Actions => {
                    if self.action_index
                        >= self.action_runs.len().saturating_sub(PREFETCH_THRESHOLD)
                        && self.actions_pagination.has_more
                        && !self.actions_pagination.loading_more
                    {
                        self.actions_pagination.loading_more = true;
                        self.actions_pagination.page += 1;
                        if let Some((owner, repo)) = &self.current_repo {
                            self.spawn_load_action_runs_page(
                                owner.clone(),
                                repo.clone(),
                                self.actions_pagination.page,
                                self.load_id,
                            );
                        }
                    }
                }
            },
            _ => {}
        }
    }

    /// Calculate max scroll offset for current detail view
    fn max_scroll_offset(&self) -> usize {
        match self.screen {
            Screen::PrDetail => {
                if let Some(pr) = &self.current_pr {
                    pr.body
                        .as_deref()
                        .unwrap_or("")
                        .lines()
                        .count()
                        .saturating_sub(1)
                } else {
                    0
                }
            }
            Screen::CommitDetail => {
                if let Some(commit) = &self.current_commit {
                    // Header lines (4) + message lines + blank + file entries
                    let mut lines = 5; // header, blank, stats, blank, "Message:"
                    lines += commit.message.lines().count();
                    lines += 1; // blank after message
                    for file in &commit.files {
                        lines += 1; // file header
                        if let Some(patch) = &file.patch {
                            lines += patch.lines().count();
                        }
                        lines += 1; // blank after file
                    }
                    lines.saturating_sub(1)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn spawn_load_commit_detail(&self, owner: String, repo: String, sha: String, load_id: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        let key = format!(
            "commit_{}_{}",
            cache::repo_key(&owner, &repo),
            &sha[..7.min(sha.len())]
        );

        if let Some(cached) = cache::read::<CommitDetail>(&key) {
            tx.send(Action::CommitDetailLoaded(Box::new(cached), load_id))
                .ok();
        }

        tokio::spawn(async move {
            match forge.get_commit(&owner, &repo, &sha).await {
                Ok(commit) => {
                    cache::write(&key, &commit);
                    tx.send(Action::CommitDetailLoaded(Box::new(commit), load_id))
                        .ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_load_pr_diff(&self, owner: String, repo: String, number: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.get_pr_diff(&owner, &repo, number).await {
                Ok(diff) => {
                    tx.send(Action::SuspendForPager(diff)).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_close_pr(&self, owner: String, repo: String, number: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.close_pr(&owner, &repo, number).await {
                Ok(()) => {
                    tx.send(Action::PrClosed).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_merge_pr(
        &self,
        owner: String,
        repo: String,
        number: u64,
        method: crate::types::MergeMethod,
    ) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge
                .merge_pr(&owner, &repo, number, method.as_api_str())
                .await
            {
                Ok(()) => {
                    tx.send(Action::PrMerged).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    fn spawn_close_issue(&self, owner: String, repo: String, number: u64) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.close_issue(&owner, &repo, number).await {
                Ok(()) => {
                    tx.send(Action::IssueClosed).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    pub fn spawn_comment(&self, owner: String, repo: String, number: u64, body: String) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge.comment(&owner, &repo, number, &body).await {
                Ok(()) => {
                    tx.send(Action::CommentPosted).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    pub fn spawn_submit_review(
        &self,
        owner: String,
        repo: String,
        number: u64,
        event: crate::types::ReviewEvent,
        body: String,
    ) {
        let tx = self.action_tx.clone();
        let forge = Arc::clone(&self.forge);
        tokio::spawn(async move {
            match forge
                .submit_review(&owner, &repo, number, event.as_api_str(), &body)
                .await
            {
                Ok(()) => {
                    tx.send(Action::ReviewSubmitted).ok();
                }
                Err(e) => {
                    tx.send(Action::Error(e.to_string())).ok();
                }
            }
        });
    }

    /// Construct GitHub URL for the current item
    pub(crate) fn current_item_url(&self) -> Option<String> {
        match self.screen {
            Screen::Home => match self.home_section {
                HomeSection::ReviewRequests => {
                    let req = self.review_requests.get(self.review_index)?;
                    Some(self.forge.web_url(
                        &req.repo_owner,
                        &req.repo_name,
                        "pr",
                        &req.pr_number.to_string(),
                    ))
                }
                HomeSection::MyPrs => {
                    let pr = self.my_prs.get(self.my_pr_index)?;
                    Some(self.forge.web_url(
                        &pr.repo_owner,
                        &pr.repo_name,
                        "pr",
                        &pr.number.to_string(),
                    ))
                }
            },
            Screen::RepoList => {
                let repo = self.repos.get(self.repo_index)?;
                Some(self.forge.web_url(&repo.owner, &repo.name, "repo", ""))
            }
            Screen::RepoView => {
                let (owner, repo) = self.current_repo.as_ref()?;
                match self.repo_tab {
                    RepoTab::PullRequests => {
                        let pr = self.prs.get(self.pr_index)?;
                        Some(
                            self.forge
                                .web_url(owner, repo, "pr", &pr.number.to_string()),
                        )
                    }
                    RepoTab::Issues => {
                        let issue = self.issues.get(self.issue_index)?;
                        Some(
                            self.forge
                                .web_url(owner, repo, "issue", &issue.number.to_string()),
                        )
                    }
                    RepoTab::Commits => {
                        let commit = self.commits.get(self.commit_index)?;
                        Some(self.forge.web_url(owner, repo, "commit", &commit.sha))
                    }
                    RepoTab::Actions => {
                        let run = self.action_runs.get(self.action_index)?;
                        Some(
                            self.forge
                                .web_url(owner, repo, "action_run", &run.id.to_string()),
                        )
                    }
                }
            }
            Screen::PrDetail => {
                let (owner, repo) = self.current_repo.as_ref()?;
                let pr = self.current_pr.as_ref()?;
                Some(
                    self.forge
                        .web_url(owner, repo, "pr", &pr.number.to_string()),
                )
            }
            Screen::CommitDetail => {
                let (owner, repo) = self.current_repo.as_ref()?;
                let commit = self.current_commit.as_ref()?;
                Some(self.forge.web_url(owner, repo, "commit", &commit.sha))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::GitHub;
    use crate::types::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    // ── Test helpers ──

    fn test_app() -> (App, mpsc::UnboundedReceiver<Action>) {
        let github = GitHub::new("dummy_token".to_string()).unwrap();
        let forge: Arc<dyn Forge> = Arc::new(github);
        let (tx, rx) = mpsc::unbounded_channel();
        (App::new(forge, tx), rx)
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn key_ctrl(c: char) -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    fn make_repo(name: &str) -> Repository {
        Repository {
            owner: "testowner".to_string(),
            name: name.to_string(),
            description: Some("A test repo".to_string()),
            url: format!("https://github.com/testowner/{}", name),
            stars: 42,
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_pr_summary(number: u64, title: &str) -> PrSummary {
        PrSummary {
            number,
            title: title.to_string(),
            state: PrState::Open,
            author: "testauthor".to_string(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_issue(number: u64, title: &str) -> Issue {
        Issue {
            number,
            title: title.to_string(),
            state: IssueState::Open,
            author: "testauthor".to_string(),
            labels: vec![],
            comments: 0,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_commit(sha: &str, message: &str) -> Commit {
        Commit {
            sha: sha.to_string(),
            message: message.to_string(),
            author: "testauthor".to_string(),
            date: chrono::Utc::now(),
        }
    }

    fn make_review_request(owner: &str, repo: &str, number: u64) -> ReviewRequest {
        ReviewRequest {
            repo_owner: owner.to_string(),
            repo_name: repo.to_string(),
            pr_number: number,
            pr_title: format!("PR #{}", number),
            author: "someone".to_string(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_my_pr(owner: &str, repo: &str, number: u64) -> MyPr {
        MyPr {
            repo_owner: owner.to_string(),
            repo_name: repo.to_string(),
            number,
            title: format!("My PR #{}", number),
            state: PrState::Open,
            checks_status: ChecksStatus::Success,
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_pull_request(number: u64, body: &str) -> PullRequest {
        PullRequest {
            number,
            title: format!("PR #{}", number),
            body: Some(body.to_string()),
            state: PrState::Open,
            author: "testauthor".to_string(),
            head_branch: "feature".to_string(),
            base_branch: "main".to_string(),
            stats: PrStats {
                additions: 10,
                deletions: 5,
                changed_files: 3,
                commits: 2,
                comments: 1,
            },
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            merged_at: None,
            closed_at: None,
        }
    }

    fn make_commit_detail(sha: &str, message: &str, files: Vec<CommitFile>) -> CommitDetail {
        CommitDetail {
            sha: sha.to_string(),
            message: message.to_string(),
            author: "testauthor".to_string(),
            date: chrono::Utc::now(),
            stats: CommitStats {
                additions: 10,
                deletions: 5,
                total: 15,
            },
            files,
        }
    }

    fn make_action_run(id: u64, name: &str) -> ActionRun {
        ActionRun {
            id,
            name: name.to_string(),
            status: ActionStatus::Completed,
            conclusion: Some(ActionConclusion::Success),
            branch: "main".to_string(),
            event: "push".to_string(),
            created_at: chrono::Utc::now(),
        }
    }

    // ── Key handling tests ──

    mod key_handling {
        use super::*;

        // Normal mode

        #[tokio::test]
        async fn q_on_home_quits() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('q')));
            assert!(matches!(action, Action::Quit));
        }

        #[tokio::test]
        async fn q_on_repo_list_goes_back() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            let action = app.handle_event(key(KeyCode::Char('q')));
            assert!(matches!(action, Action::Back));
        }

        #[tokio::test]
        async fn esc_on_home_quits() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Esc));
            assert!(matches!(action, Action::Quit));
        }

        #[tokio::test]
        async fn esc_on_repo_list_goes_back() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            let action = app.handle_event(key(KeyCode::Esc));
            assert!(matches!(action, Action::Back));
        }

        #[tokio::test]
        async fn esc_with_active_search_clears() {
            let (mut app, _rx) = test_app();
            app.search.active = true;
            let action = app.handle_event(key(KeyCode::Esc));
            assert!(matches!(action, Action::ClearSearch));
        }

        #[tokio::test]
        async fn j_scrolls_down() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('j')));
            assert!(matches!(action, Action::ScrollDown));
        }

        #[tokio::test]
        async fn down_scrolls_down() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Down));
            assert!(matches!(action, Action::ScrollDown));
        }

        #[tokio::test]
        async fn k_scrolls_up() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('k')));
            assert!(matches!(action, Action::ScrollUp));
        }

        #[tokio::test]
        async fn up_scrolls_up() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Up));
            assert!(matches!(action, Action::ScrollUp));
        }

        #[tokio::test]
        async fn g_goes_to_top() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('g')));
            assert!(matches!(action, Action::GoToTop));
        }

        #[tokio::test]
        async fn home_goes_to_top() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Home));
            assert!(matches!(action, Action::GoToTop));
        }

        #[tokio::test]
        async fn big_g_goes_to_bottom() {
            let (app, _rx) = test_app();
            // G is uppercase, which crossterm sends as Char('G') with SHIFT
            let action = app.handle_event(key(KeyCode::Char('G')));
            assert!(matches!(action, Action::GoToBottom));
        }

        #[tokio::test]
        async fn end_goes_to_bottom() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::End));
            assert!(matches!(action, Action::GoToBottom));
        }

        #[tokio::test]
        async fn ctrl_d_pages_down() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key_ctrl('d'));
            assert!(matches!(action, Action::PageDown));
        }

        #[tokio::test]
        async fn ctrl_u_pages_up() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key_ctrl('u'));
            assert!(matches!(action, Action::PageUp));
        }

        #[tokio::test]
        async fn slash_enters_search() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('/')));
            assert!(matches!(action, Action::EnterSearchMode));
        }

        #[tokio::test]
        async fn n_with_active_search_next() {
            let (mut app, _rx) = test_app();
            app.search.active = true;
            let action = app.handle_event(key(KeyCode::Char('n')));
            assert!(matches!(action, Action::SearchNext));
        }

        #[tokio::test]
        async fn big_n_with_active_search_prev() {
            let (mut app, _rx) = test_app();
            app.search.active = true;
            let action = app.handle_event(key(KeyCode::Char('N')));
            assert!(matches!(action, Action::SearchPrev));
        }

        #[tokio::test]
        async fn n_without_search_is_not_search_next() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('n')));
            assert!(!matches!(action, Action::SearchNext));
        }

        #[tokio::test]
        async fn enter_selects() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Enter));
            assert!(matches!(action, Action::Select));
        }

        #[tokio::test]
        async fn d_on_pr_detail_views_diff() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::PrDetail;
            let action = app.handle_event(key(KeyCode::Char('d')));
            assert!(matches!(action, Action::ViewDiff));
        }

        #[tokio::test]
        async fn d_on_commit_detail_views_diff() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::CommitDetail;
            let action = app.handle_event(key(KeyCode::Char('d')));
            assert!(matches!(action, Action::ViewDiff));
        }

        #[tokio::test]
        async fn d_on_repo_list_not_view_diff() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            let action = app.handle_event(key(KeyCode::Char('d')));
            assert!(!matches!(action, Action::ViewDiff));
        }

        #[tokio::test]
        async fn r_refreshes() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('r')));
            assert!(matches!(action, Action::Refresh));
        }

        #[tokio::test]
        async fn o_opens_in_browser() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('o')));
            assert!(matches!(action, Action::OpenInBrowser));
        }

        #[tokio::test]
        async fn y_yanks_url() {
            let (app, _rx) = test_app();
            let action = app.handle_event(key(KeyCode::Char('y')));
            assert!(matches!(action, Action::YankUrl));
        }

        #[tokio::test]
        async fn m_on_pr_detail_shows_merge() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::PrDetail;
            let action = app.handle_event(key(KeyCode::Char('m')));
            assert!(matches!(action, Action::ShowMergeMethodSelect));
        }

        #[tokio::test]
        async fn m_on_repo_list_is_none() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            let action = app.handle_event(key(KeyCode::Char('m')));
            assert!(matches!(action, Action::None));
        }

        #[tokio::test]
        async fn big_r_on_pr_detail_shows_review() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::PrDetail;
            let action = app.handle_event(key(KeyCode::Char('R')));
            assert!(matches!(action, Action::ShowReviewSelect));
        }

        #[tokio::test]
        async fn p_on_repo_view_switches_tab() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            let action = app.handle_event(key(KeyCode::Char('p')));
            assert!(matches!(
                action,
                Action::SwitchRepoTab(RepoTab::PullRequests)
            ));
        }

        #[tokio::test]
        async fn i_on_repo_view_switches_tab() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            let action = app.handle_event(key(KeyCode::Char('i')));
            assert!(matches!(action, Action::SwitchRepoTab(RepoTab::Issues)));
        }

        #[tokio::test]
        async fn c_on_repo_view_switches_tab() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            let action = app.handle_event(key(KeyCode::Char('c')));
            assert!(matches!(action, Action::SwitchRepoTab(RepoTab::Commits)));
        }

        #[tokio::test]
        async fn a_on_repo_view_switches_tab() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            let action = app.handle_event(key(KeyCode::Char('a')));
            assert!(matches!(action, Action::SwitchRepoTab(RepoTab::Actions)));
        }

        // Search mode

        #[tokio::test]
        async fn search_esc_exits() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            let action = app.handle_event(key(KeyCode::Esc));
            assert!(matches!(action, Action::ExitSearchMode));
        }

        #[tokio::test]
        async fn search_enter_confirms() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            let action = app.handle_event(key(KeyCode::Enter));
            assert!(matches!(action, Action::SearchConfirm));
        }

        #[tokio::test]
        async fn search_backspace() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            let action = app.handle_event(key(KeyCode::Backspace));
            assert!(matches!(action, Action::SearchBackspace));
        }

        #[tokio::test]
        async fn search_char_input() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            let action = app.handle_event(key(KeyCode::Char('f')));
            assert!(matches!(action, Action::SearchInput('f')));
        }

        #[tokio::test]
        async fn search_other_key_none() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            let action = app.handle_event(key(KeyCode::Tab));
            assert!(matches!(action, Action::None));
        }

        // Confirm mode

        #[tokio::test]
        async fn confirm_y_yes() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Confirm;
            let action = app.handle_event(key(KeyCode::Char('y')));
            assert!(matches!(action, Action::ConfirmYes));
        }

        #[tokio::test]
        async fn confirm_n_no() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Confirm;
            let action = app.handle_event(key(KeyCode::Char('n')));
            assert!(matches!(action, Action::ConfirmNo));
        }

        #[tokio::test]
        async fn confirm_esc_no() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Confirm;
            let action = app.handle_event(key(KeyCode::Esc));
            assert!(matches!(action, Action::ConfirmNo));
        }

        // SelectPopup mode

        #[tokio::test]
        async fn popup_j_down() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::SelectPopup;
            let action = app.handle_event(key(KeyCode::Char('j')));
            assert!(matches!(action, Action::PopupDown));
        }

        #[tokio::test]
        async fn popup_k_up() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::SelectPopup;
            let action = app.handle_event(key(KeyCode::Char('k')));
            assert!(matches!(action, Action::PopupUp));
        }

        #[tokio::test]
        async fn popup_enter_select() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::SelectPopup;
            let action = app.handle_event(key(KeyCode::Enter));
            assert!(matches!(action, Action::PopupSelect));
        }

        #[tokio::test]
        async fn popup_esc_cancels() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::SelectPopup;
            let action = app.handle_event(key(KeyCode::Esc));
            assert!(matches!(action, Action::ConfirmNo));
        }
    }

    // ── State transition tests ──

    mod state_transitions {
        use super::*;

        // Navigation

        #[tokio::test]
        async fn quit_sets_should_quit() {
            let (mut app, _rx) = test_app();
            app.update(Action::Quit);
            assert!(app.should_quit);
        }

        #[tokio::test]
        async fn back_from_home_quits() {
            let (mut app, _rx) = test_app();
            app.update(Action::Back);
            assert!(app.should_quit);
        }

        #[tokio::test]
        async fn back_from_repo_list_to_home() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.update(Action::Back);
            assert_eq!(app.screen, Screen::Home);
        }

        #[tokio::test]
        async fn back_from_repo_view_to_repo_list() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            app.repo_tab = RepoTab::Issues;
            app.prs = vec![make_pr_summary(1, "test")];
            app.issues = vec![make_issue(1, "test")];
            app.commits = vec![make_commit("abc123", "test")];
            app.update(Action::Back);
            assert_eq!(app.screen, Screen::RepoList);
            assert_eq!(app.repo_tab, RepoTab::PullRequests);
            assert!(app.prs.is_empty());
            assert!(app.issues.is_empty());
            assert!(app.commits.is_empty());
        }

        #[tokio::test]
        async fn back_from_pr_detail() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::PrDetail;
            app.prev_screen = Some(Screen::RepoView);
            app.current_pr = Some(make_pull_request(1, "body"));
            app.scroll_offset = 5;
            app.update(Action::Back);
            assert_eq!(app.screen, Screen::RepoView);
            assert!(app.current_pr.is_none());
            assert_eq!(app.scroll_offset, 0);
        }

        #[tokio::test]
        async fn back_from_commit_detail() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::CommitDetail;
            app.prev_screen = Some(Screen::RepoView);
            app.current_commit = Some(make_commit_detail("abc123", "msg", vec![]));
            app.update(Action::Back);
            assert_eq!(app.screen, Screen::RepoView);
            assert!(app.current_commit.is_none());
            assert_eq!(app.scroll_offset, 0);
        }

        #[tokio::test]
        async fn next_tab_on_home_toggles_section() {
            let (mut app, _rx) = test_app();
            assert_eq!(app.home_section, HomeSection::ReviewRequests);
            app.update(Action::NextTab);
            assert_eq!(app.home_section, HomeSection::MyPrs);
            app.update(Action::NextTab);
            assert_eq!(app.home_section, HomeSection::ReviewRequests);
        }

        #[tokio::test]
        async fn next_tab_on_repo_view_cycles() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            assert_eq!(app.repo_tab, RepoTab::PullRequests);
            app.update(Action::NextTab);
            assert_eq!(app.repo_tab, RepoTab::Issues);
            app.update(Action::NextTab);
            assert_eq!(app.repo_tab, RepoTab::Commits);
            app.update(Action::NextTab);
            assert_eq!(app.repo_tab, RepoTab::Actions);
            app.update(Action::NextTab);
            assert_eq!(app.repo_tab, RepoTab::PullRequests);
        }

        #[tokio::test]
        async fn prev_tab_on_repo_view_cycles_backward() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            assert_eq!(app.repo_tab, RepoTab::PullRequests);
            app.update(Action::PrevTab);
            assert_eq!(app.repo_tab, RepoTab::Actions);
            app.update(Action::PrevTab);
            assert_eq!(app.repo_tab, RepoTab::Commits);
            app.update(Action::PrevTab);
            assert_eq!(app.repo_tab, RepoTab::Issues);
            app.update(Action::PrevTab);
            assert_eq!(app.repo_tab, RepoTab::PullRequests);
        }

        #[tokio::test]
        async fn switch_repo_tab_sets_tab_and_resets_index() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            app.issue_index = 5;
            app.update(Action::SwitchRepoTab(RepoTab::Issues));
            assert_eq!(app.repo_tab, RepoTab::Issues);
            assert_eq!(app.issue_index, 0);
        }

        #[tokio::test]
        async fn refresh_on_home_goes_to_repo_list() {
            let (mut app, _rx) = test_app();
            app.update(Action::Refresh);
            assert_eq!(app.screen, Screen::RepoList);
            assert!(app.loading);
        }

        #[tokio::test]
        async fn refresh_on_repo_list_sets_loading() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.update(Action::Refresh);
            assert_eq!(app.screen, Screen::RepoList);
            assert!(app.loading);
        }

        // Scroll/Index

        #[tokio::test]
        async fn scroll_down_increments_repo_index() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = vec![make_repo("a"), make_repo("b"), make_repo("c")];
            app.repo_index = 0;
            app.update(Action::ScrollDown);
            assert_eq!(app.repo_index, 1);
        }

        #[tokio::test]
        async fn scroll_down_at_end_no_overflow() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = vec![make_repo("a"), make_repo("b")];
            app.repo_index = 1;
            app.update(Action::ScrollDown);
            assert_eq!(app.repo_index, 1);
        }

        #[tokio::test]
        async fn scroll_down_empty_list_noop() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.update(Action::ScrollDown);
            assert_eq!(app.repo_index, 0);
        }

        #[tokio::test]
        async fn scroll_up_decrements_repo_index() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = vec![make_repo("a"), make_repo("b")];
            app.repo_index = 1;
            app.update(Action::ScrollUp);
            assert_eq!(app.repo_index, 0);
        }

        #[tokio::test]
        async fn scroll_up_at_zero_stays() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = vec![make_repo("a")];
            app.repo_index = 0;
            app.update(Action::ScrollUp);
            assert_eq!(app.repo_index, 0);
        }

        #[tokio::test]
        async fn go_to_top_resets_index() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = vec![make_repo("a"), make_repo("b"), make_repo("c")];
            app.repo_index = 2;
            app.update(Action::GoToTop);
            assert_eq!(app.repo_index, 0);
        }

        #[tokio::test]
        async fn go_to_bottom_sets_last_index() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = vec![make_repo("a"), make_repo("b"), make_repo("c")];
            app.update(Action::GoToBottom);
            assert_eq!(app.repo_index, 2);
        }

        #[tokio::test]
        async fn go_to_bottom_empty_list_noop() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.update(Action::GoToBottom);
            assert_eq!(app.repo_index, 0);
        }

        #[tokio::test]
        async fn page_down_advances_by_10_clamped() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            // Create 5 repos — page down should clamp to index 4
            app.repos = (0..5).map(|i| make_repo(&format!("r{}", i))).collect();
            app.repo_index = 0;
            app.update(Action::PageDown);
            assert_eq!(app.repo_index, 4);
        }

        #[tokio::test]
        async fn page_up_decrements_by_10_saturating() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = (0..20).map(|i| make_repo(&format!("r{}", i))).collect();
            app.repo_index = 5;
            app.update(Action::PageUp);
            assert_eq!(app.repo_index, 0);
        }

        // Data loading (load_id)

        #[tokio::test]
        async fn home_loaded_matching_id_updates_data() {
            let (mut app, _rx) = test_app();
            app.load_id = 1;
            let rrs = vec![make_review_request("o", "r", 1)];
            let prs = vec![make_my_pr("o", "r", 2)];
            app.update(Action::HomeLoaded {
                review_requests: rrs.clone(),
                my_prs: prs.clone(),
                load_id: 1,
            });
            assert_eq!(app.review_requests.len(), 1);
            assert_eq!(app.my_prs.len(), 1);
            assert!(!app.loading);
        }

        #[tokio::test]
        async fn home_loaded_stale_id_ignored() {
            let (mut app, _rx) = test_app();
            app.load_id = 2;
            app.update(Action::HomeLoaded {
                review_requests: vec![make_review_request("o", "r", 1)],
                my_prs: vec![],
                load_id: 1,
            });
            assert!(app.review_requests.is_empty());
        }

        #[tokio::test]
        async fn repos_loaded_matching_id_updates() {
            let (mut app, _rx) = test_app();
            app.load_id = 3;
            app.repo_index = 10;
            let repos = vec![make_repo("a"), make_repo("b")];
            app.update(Action::ReposLoaded(repos, 3));
            assert_eq!(app.repos.len(), 2);
            assert_eq!(app.repo_index, 1); // clamped
        }

        #[tokio::test]
        async fn repos_loaded_stale_id_ignored() {
            let (mut app, _rx) = test_app();
            app.load_id = 5;
            app.update(Action::ReposLoaded(vec![make_repo("a")], 3));
            assert!(app.repos.is_empty());
        }

        #[tokio::test]
        async fn pr_detail_loaded_first_time_transitions_screen() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            app.load_id = 1;
            let pr = make_pull_request(42, "test body");
            app.update(Action::PrDetailLoaded(Box::new(pr), 1));
            assert_eq!(app.screen, Screen::PrDetail);
            assert!(app.current_pr.is_some());
            assert_eq!(app.scroll_offset, 0);
        }

        #[tokio::test]
        async fn pr_detail_loaded_already_on_pr_detail_no_scroll_reset() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::PrDetail;
            app.load_id = 2;
            app.scroll_offset = 10;
            let pr = make_pull_request(42, "updated body");
            app.update(Action::PrDetailLoaded(Box::new(pr), 2));
            assert_eq!(app.screen, Screen::PrDetail);
            assert!(app.current_pr.is_some());
            assert_eq!(app.scroll_offset, 10); // not reset
        }

        // Popup & confirm

        #[tokio::test]
        async fn show_merge_method_select() {
            let (mut app, _rx) = test_app();
            app.update(Action::ShowMergeMethodSelect);
            assert_eq!(app.input_mode, InputMode::SelectPopup);
            assert_eq!(app.popup_items.len(), 3);
        }

        #[tokio::test]
        async fn show_review_select() {
            let (mut app, _rx) = test_app();
            app.update(Action::ShowReviewSelect);
            assert_eq!(app.input_mode, InputMode::SelectPopup);
            assert_eq!(app.popup_items.len(), 3);
        }

        #[tokio::test]
        async fn popup_down_increments() {
            let (mut app, _rx) = test_app();
            app.popup_items = vec!["a".into(), "b".into(), "c".into()];
            app.popup_index = 0;
            app.update(Action::PopupDown);
            assert_eq!(app.popup_index, 1);
        }

        #[tokio::test]
        async fn popup_up_decrements() {
            let (mut app, _rx) = test_app();
            app.popup_items = vec!["a".into(), "b".into(), "c".into()];
            app.popup_index = 2;
            app.update(Action::PopupUp);
            assert_eq!(app.popup_index, 1);
        }

        #[tokio::test]
        async fn show_confirm_sets_state() {
            let (mut app, _rx) = test_app();
            app.update(Action::ShowConfirm(ConfirmAction::ClosePr(42)));
            assert_eq!(app.input_mode, InputMode::Confirm);
            assert!(matches!(
                app.confirm_action,
                Some(ConfirmAction::ClosePr(42))
            ));
        }

        #[tokio::test]
        async fn confirm_no_resets() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Confirm;
            app.confirm_action = Some(ConfirmAction::ClosePr(42));
            app.update(Action::ConfirmNo);
            assert_eq!(app.input_mode, InputMode::Normal);
            assert!(app.confirm_action.is_none());
        }

        // Search state machine

        #[tokio::test]
        async fn enter_search_mode() {
            let (mut app, _rx) = test_app();
            app.search.query = "old".to_string();
            app.update(Action::EnterSearchMode);
            assert_eq!(app.input_mode, InputMode::Search);
            assert!(app.search.query.is_empty());
        }

        #[tokio::test]
        async fn search_input_appends() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            app.update(Action::SearchInput('a'));
            assert_eq!(app.search.query, "a");
            app.update(Action::SearchInput('b'));
            assert_eq!(app.search.query, "ab");
        }

        #[tokio::test]
        async fn search_backspace_pops() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            app.search.query = "ab".to_string();
            app.update(Action::SearchBackspace);
            assert_eq!(app.search.query, "a");
        }

        #[tokio::test]
        async fn search_backspace_empty_deactivates() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            app.search.query = "a".to_string();
            app.search.active = true;
            app.update(Action::SearchBackspace);
            // query is now empty
            assert!(app.search.query.is_empty());
            assert!(!app.search.active);
        }

        #[tokio::test]
        async fn exit_search_mode_keeps_active() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            app.search.query = "foo".to_string();
            app.update(Action::ExitSearchMode);
            assert_eq!(app.input_mode, InputMode::Normal);
            assert!(app.search.active);
        }

        #[tokio::test]
        async fn search_confirm_activates() {
            let (mut app, _rx) = test_app();
            app.input_mode = InputMode::Search;
            app.search.query = "bar".to_string();
            app.update(Action::SearchConfirm);
            assert_eq!(app.input_mode, InputMode::Normal);
            assert!(app.search.active);
        }

        #[tokio::test]
        async fn clear_search_resets() {
            let (mut app, _rx) = test_app();
            app.search.query = "foo".to_string();
            app.search.active = true;
            app.search.match_indices = vec![0, 1, 2];
            app.update(Action::ClearSearch);
            assert!(app.search.query.is_empty());
            assert!(!app.search.active);
            assert!(app.search.match_indices.is_empty());
        }

        #[tokio::test]
        async fn search_on_repo_list_computes_matches() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = vec![make_repo("foo-bar"), make_repo("baz"), make_repo("foo-qux")];
            app.update(Action::SearchInput('f'));
            app.update(Action::SearchInput('o'));
            app.update(Action::SearchInput('o'));
            assert_eq!(app.search.match_indices, vec![0, 2]);
        }

        // Mutation results

        #[tokio::test]
        async fn pr_merged_sets_flash() {
            let (mut app, _rx) = test_app();
            app.update(Action::PrMerged);
            assert!(app.flash_message.is_some());
            assert_eq!(app.flash_message.as_ref().unwrap().0, "PR merged!");
        }

        #[tokio::test]
        async fn pr_closed_sets_flash() {
            let (mut app, _rx) = test_app();
            app.update(Action::PrClosed);
            assert!(app.flash_message.is_some());
            assert_eq!(app.flash_message.as_ref().unwrap().0, "PR closed.");
        }

        #[tokio::test]
        async fn issue_closed_sets_flash() {
            let (mut app, _rx) = test_app();
            app.update(Action::IssueClosed);
            assert!(app.flash_message.is_some());
            assert_eq!(app.flash_message.as_ref().unwrap().0, "Issue closed.");
        }

        #[tokio::test]
        async fn comment_posted_sets_flash() {
            let (mut app, _rx) = test_app();
            app.update(Action::CommentPosted);
            assert!(app.flash_message.is_some());
            assert_eq!(app.flash_message.as_ref().unwrap().0, "Comment posted.");
        }

        #[tokio::test]
        async fn error_sets_error_clears_loading() {
            let (mut app, _rx) = test_app();
            app.loading = true;
            app.update(Action::Error("something failed".to_string()));
            assert_eq!(app.error, Some("something failed".to_string()));
            assert!(!app.loading);
        }
    }

    // ── Pagination tests ──

    mod pagination {
        use super::*;

        #[tokio::test]
        async fn repos_loaded_sets_has_more_when_full_page() {
            let (mut app, _rx) = test_app();
            app.load_id = 1;
            let repos: Vec<Repository> = (0..PAGE_SIZE)
                .map(|i| make_repo(&format!("r{}", i)))
                .collect();
            app.update(Action::ReposLoaded(repos, 1));
            assert!(app.repos_pagination.has_more);
            assert_eq!(app.repos_pagination.page, 1);
        }

        #[tokio::test]
        async fn repos_loaded_clears_has_more_when_partial_page() {
            let (mut app, _rx) = test_app();
            app.load_id = 1;
            let repos = vec![make_repo("a"), make_repo("b")];
            app.update(Action::ReposLoaded(repos, 1));
            assert!(!app.repos_pagination.has_more);
        }

        #[tokio::test]
        async fn repos_appended_extends_list() {
            let (mut app, _rx) = test_app();
            app.load_id = 1;
            app.repos = vec![make_repo("a")];
            let new_repos = vec![make_repo("b"), make_repo("c")];
            app.repos_pagination.loading_more = true;
            app.update(Action::ReposAppended(new_repos, 1));
            assert_eq!(app.repos.len(), 3);
            assert!(!app.repos_pagination.loading_more);
            assert!(!app.repos_pagination.has_more); // 2 < PAGE_SIZE
        }

        #[tokio::test]
        async fn repos_appended_stale_id_ignored() {
            let (mut app, _rx) = test_app();
            app.load_id = 5;
            app.repos = vec![make_repo("a")];
            app.repos_pagination.loading_more = true;
            app.update(Action::ReposAppended(vec![make_repo("b")], 3));
            assert_eq!(app.repos.len(), 1); // not extended
            assert!(app.repos_pagination.loading_more); // not cleared
        }

        #[tokio::test]
        async fn scroll_down_near_end_triggers_pagination() {
            let (mut app, mut rx) = test_app();
            app.screen = Screen::RepoList;
            // Create a list of 50 repos (full page)
            app.repos = (0..PAGE_SIZE)
                .map(|i| make_repo(&format!("r{}", i)))
                .collect();
            app.repos_pagination.has_more = true;
            app.repos_pagination.loading_more = false;
            // Set index near end (within PREFETCH_THRESHOLD)
            app.repo_index = PAGE_SIZE - 2; // second to last
            app.update(Action::ScrollDown);
            // Index should advance
            assert_eq!(app.repo_index, PAGE_SIZE - 1);
            // Pagination should be triggered
            assert!(app.repos_pagination.loading_more);
            assert_eq!(app.repos_pagination.page, 2);
            // Drain the channel to verify no errors
            rx.try_recv().ok();
        }

        #[tokio::test]
        async fn scroll_down_near_end_no_trigger_when_no_more() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = (0..10).map(|i| make_repo(&format!("r{}", i))).collect();
            app.repos_pagination.has_more = false;
            app.repo_index = 8;
            app.update(Action::ScrollDown);
            assert_eq!(app.repo_index, 9);
            assert!(!app.repos_pagination.loading_more);
            assert_eq!(app.repos_pagination.page, 1);
        }

        #[tokio::test]
        async fn go_to_bottom_triggers_pagination() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = (0..PAGE_SIZE)
                .map(|i| make_repo(&format!("r{}", i)))
                .collect();
            app.repos_pagination.has_more = true;
            app.update(Action::GoToBottom);
            assert_eq!(app.repo_index, PAGE_SIZE - 1);
            assert!(app.repos_pagination.loading_more);
        }

        #[tokio::test]
        async fn page_down_triggers_pagination() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = (0..PAGE_SIZE)
                .map(|i| make_repo(&format!("r{}", i)))
                .collect();
            app.repos_pagination.has_more = true;
            app.repo_index = PAGE_SIZE - 11;
            app.update(Action::PageDown);
            assert_eq!(app.repo_index, PAGE_SIZE - 1);
            assert!(app.repos_pagination.loading_more);
        }

        #[tokio::test]
        async fn prs_appended_extends_list() {
            let (mut app, _rx) = test_app();
            app.load_id = 1;
            app.prs = vec![make_pr_summary(1, "first")];
            app.prs_pagination.loading_more = true;
            app.update(Action::PrsAppended(vec![make_pr_summary(2, "second")], 1));
            assert_eq!(app.prs.len(), 2);
            assert!(!app.prs_pagination.loading_more);
        }

        #[tokio::test]
        async fn repos_loaded_resets_pagination() {
            let (mut app, _rx) = test_app();
            app.load_id = 1;
            app.repos_pagination.page = 3;
            app.repos_pagination.has_more = true;
            app.repos_pagination.loading_more = true;
            app.update(Action::ReposLoaded(vec![make_repo("a")], 1));
            assert_eq!(app.repos_pagination.page, 1);
            assert!(!app.repos_pagination.has_more);
            assert!(!app.repos_pagination.loading_more);
        }
    }

    // ── URL construction tests ──

    mod url_construction {
        use super::*;

        #[tokio::test]
        async fn home_review_requests_url() {
            let (mut app, _rx) = test_app();
            app.review_requests = vec![make_review_request("octo", "repo", 42)];
            app.home_section = HomeSection::ReviewRequests;
            app.review_index = 0;
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/octo/repo/pull/42".to_string())
            );
        }

        #[tokio::test]
        async fn home_my_prs_url() {
            let (mut app, _rx) = test_app();
            app.my_prs = vec![make_my_pr("octo", "repo", 7)];
            app.home_section = HomeSection::MyPrs;
            app.my_pr_index = 0;
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/octo/repo/pull/7".to_string())
            );
        }

        #[tokio::test]
        async fn repo_list_url() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoList;
            app.repos = vec![make_repo("myrepo")];
            app.repo_index = 0;
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/testowner/myrepo".to_string())
            );
        }

        #[tokio::test]
        async fn repo_view_pull_requests_url() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            app.current_repo = Some(("owner".to_string(), "repo".to_string()));
            app.repo_tab = RepoTab::PullRequests;
            app.prs = vec![make_pr_summary(99, "test")];
            app.pr_index = 0;
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/owner/repo/pull/99".to_string())
            );
        }

        #[tokio::test]
        async fn repo_view_issues_url() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            app.current_repo = Some(("owner".to_string(), "repo".to_string()));
            app.repo_tab = RepoTab::Issues;
            app.issues = vec![make_issue(15, "test")];
            app.issue_index = 0;
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/owner/repo/issues/15".to_string())
            );
        }

        #[tokio::test]
        async fn repo_view_commits_url() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            app.current_repo = Some(("owner".to_string(), "repo".to_string()));
            app.repo_tab = RepoTab::Commits;
            app.commits = vec![make_commit("abc123def456", "msg")];
            app.commit_index = 0;
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/owner/repo/commit/abc123def456".to_string())
            );
        }

        #[tokio::test]
        async fn repo_view_actions_url() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::RepoView;
            app.current_repo = Some(("owner".to_string(), "repo".to_string()));
            app.repo_tab = RepoTab::Actions;
            app.action_runs = vec![make_action_run(12345, "CI")];
            app.action_index = 0;
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/owner/repo/actions/runs/12345".to_string())
            );
        }

        #[tokio::test]
        async fn pr_detail_url() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::PrDetail;
            app.current_repo = Some(("owner".to_string(), "repo".to_string()));
            app.current_pr = Some(make_pull_request(55, "body"));
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/owner/repo/pull/55".to_string())
            );
        }

        #[tokio::test]
        async fn commit_detail_url() {
            let (mut app, _rx) = test_app();
            app.screen = Screen::CommitDetail;
            app.current_repo = Some(("owner".to_string(), "repo".to_string()));
            app.current_commit = Some(make_commit_detail("deadbeef", "msg", vec![]));
            assert_eq!(
                app.current_item_url(),
                Some("https://github.com/owner/repo/commit/deadbeef".to_string())
            );
        }

        #[tokio::test]
        async fn empty_state_returns_none() {
            let (app, _rx) = test_app();
            // Home screen with no review requests
            assert_eq!(app.current_item_url(), None);
        }
    }
}
