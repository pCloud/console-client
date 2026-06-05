use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use secrecy::{ExposeSecret, SecretString};

use crate::daemon::{DaemonClient, DaemonCommand, DaemonResponse};
use crate::ffi::types::{
    PSTATUS_BAD_LOGIN_DATA, PSTATUS_BAD_LOGIN_TOKEN, PSTATUS_LOGIN_REQUIRED, PSTATUS_PAUSED,
    PSTATUS_READY, PSTATUS_STOPPED,
};
use crate::security::zeroize_string;
use crate::wrapper::{CryptoState, DashboardSnapshot};

use super::state::{
    AboutFocus, CryptoAction, InputMode, Screen, StatusMessageKind, SyncEngineState, TuiState,
};

/// The main TUI application.
///
/// The TUI is a pure IPC client: it owns no pclsync engine. All state is
/// fetched from, and all actions are sent to, the daemon over the Unix socket
/// via [`DaemonClient`].
pub struct App {
    pub state: TuiState,
    daemon: DaemonClient,
    /// Highest activity-log sequence id already pulled from the daemon.
    activity_cursor: u64,
}

impl App {
    pub fn new(daemon: DaemonClient) -> Self {
        Self {
            state: TuiState::new(),
            daemon,
            activity_cursor: 0,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.state.should_quit
    }

    /// Send a command to the daemon, tracking connectivity.
    ///
    /// Returns `None` (and flags the daemon as unavailable) on an IPC failure,
    /// so callers degrade gracefully instead of panicking. Connectivity is
    /// re-established automatically on the next successful command.
    fn send(&mut self, command: DaemonCommand) -> Option<DaemonResponse> {
        match self.daemon.send_command(command) {
            Ok(resp) => {
                self.state.daemon_unavailable = false;
                Some(resp)
            }
            Err(_) => {
                self.state.daemon_unavailable = true;
                None
            }
        }
    }

    /// Send a fire-and-report action whose response is `Ok` / `OkWithMessage` /
    /// `Error`, surfacing the outcome as a transient status message.
    fn run_action(&mut self, command: DaemonCommand, success_default: &str) {
        match self.send(command) {
            Some(DaemonResponse::Ok) => self
                .state
                .set_status_message(success_default.into(), StatusMessageKind::Success),
            Some(DaemonResponse::OkWithMessage(msg)) => self
                .state
                .set_status_message(msg, StatusMessageKind::Success),
            Some(DaemonResponse::Error(e)) => {
                self.state.set_status_message(e, StatusMessageKind::Error)
            }
            Some(_) => self.state.set_status_message(
                "Unexpected daemon response".into(),
                StatusMessageKind::Error,
            ),
            None => {}
        }
    }

    /// Apply a dashboard snapshot fetched from the daemon to the view state,
    /// driving auth-screen transitions as the engine's login state changes.
    fn apply_snapshot(&mut self, snap: DashboardSnapshot) {
        let needs_auth = matches!(
            snap.status.status,
            PSTATUS_LOGIN_REQUIRED | PSTATUS_BAD_LOGIN_DATA | PSTATUS_BAD_LOGIN_TOKEN
        );

        self.state.status = snap.status;
        self.state.auth_state = snap.auth_state;
        self.state.crypto_state = snap.crypto_state;
        self.state.fs_mounted = snap.fs_mounted;
        self.state.mountpoint = snap.mountpoint;
        self.state.account_email = snap.account_email;
        self.state.quota_used = snap.quota_used;
        self.state.quota_total = snap.quota_total;
        self.state.account_location = snap.account_location;
        self.state.crypto_folder_path = snap.crypto_folder_path;

        if needs_auth && self.state.input_mode == InputMode::Normal {
            self.state.input_mode = InputMode::AuthMenu;
        } else if !needs_auth
            && matches!(
                self.state.input_mode,
                InputMode::AuthMenu | InputMode::AuthToken | InputMode::AuthWebWaiting(_)
            )
        {
            self.state.needs_clear = true;
            self.state.input_mode = InputMode::Normal;
            self.state.set_status_message(
                "Authentication successful!".into(),
                StatusMessageKind::Success,
            );
        }
    }

    /// Handle a key event.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match &self.state.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::AuthMenu => self.handle_auth_menu_key(key),
            InputMode::AuthToken => self.handle_auth_token_key(key),
            InputMode::AuthWebWaiting(_) => self.handle_auth_waiting_key(key),
            InputMode::PasswordPrompt(_) => self.handle_password_key(key),
            InputMode::HintPrompt => self.handle_hint_key(key),
            InputMode::UnlinkConfirm => self.handle_unlink_confirm_key(key),
            InputMode::BackupAdd => self.handle_backup_add_key(key),
            InputMode::BackupRemoveConfirm => self.handle_backup_remove_confirm_key(key),
            InputMode::BackupStopDeviceConfirm => self.handle_backup_stop_confirm_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        // Global keys (work on any screen)
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.state.should_quit = true;
                return;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
                return;
            }
            KeyCode::Char('1') => {
                self.state.active_screen = Screen::Dashboard;
                self.state.about_focus = None;
                return;
            }
            KeyCode::Char('2') => {
                self.state.active_screen = Screen::Backups;
                self.state.about_focus = None;
                self.refresh_backups();
                return;
            }
            KeyCode::Char('3') => {
                self.state.active_screen = Screen::Help;
                self.state.about_focus = None;
                return;
            }
            KeyCode::Char('4') => {
                self.state.active_screen = Screen::About;
                self.state.about_focus = Some(AboutFocus::ClientBuild);
                return;
            }
            _ => {}
        }

        // About screen keys
        if self.state.active_screen == Screen::About {
            match key.code {
                KeyCode::Tab => {
                    self.state.about_focus = Some(
                        self.state
                            .about_focus
                            .as_ref()
                            .map(|f| f.next())
                            .unwrap_or(AboutFocus::ClientBuild),
                    );
                }
                KeyCode::Enter => {
                    if let Some(ref focus) = self.state.about_focus {
                        let client_commit = option_env!("PCLOUD_GIT_COMMIT").unwrap_or("main");
                        let pclsync_commit = option_env!("PCLSYNC_GIT_COMMIT").unwrap_or("main");
                        let url = match focus {
                            AboutFocus::ClientBuild => format!(
                                "https://github.com/pCloud/console-client/commit/{}",
                                client_commit
                            ),
                            AboutFocus::PclsyncBuild => format!(
                                "https://github.com/pCloud/pclsync/commit/{}",
                                pclsync_commit
                            ),
                            AboutFocus::LicenseLink => format!(
                                "https://github.com/pCloud/console-client/blob/{}/LICENSE",
                                client_commit
                            ),
                        };
                        let _ = crate::utils::browser::open_url(&url, true);
                    }
                }
                _ => {}
            }
            return;
        }

        // Backups screen keys
        if self.state.active_screen == Screen::Backups {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.scroll_backup_up(),
                KeyCode::Down | KeyCode::Char('j') => self.scroll_backup_down(),
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.state.input_mode = InputMode::BackupAdd;
                    self.state.input_buffer.clear();
                }
                KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                    if self.state.selected_backup().is_some() {
                        self.state.input_mode = InputMode::BackupRemoveConfirm;
                    } else {
                        self.state.set_status_message(
                            "No backup selected".into(),
                            StatusMessageKind::Error,
                        );
                    }
                }
                KeyCode::Char('S') => {
                    self.state.input_mode = InputMode::BackupStopDeviceConfirm;
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.refresh_backups();
                }
                _ => {}
            }
            return;
        }

        // Dashboard-only keys
        if self.state.active_screen != Screen::Dashboard {
            return;
        }

        match key.code {
            KeyCode::Tab => {
                self.state.active_panel = self.state.active_panel.next();
            }
            KeyCode::BackTab => {
                self.state.active_panel = self.state.active_panel.prev();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_log_up();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_log_down();
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if !self.state.activity_log.is_empty() {
                    self.state.log_state.select(Some(0));
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                let len = self.state.activity_log.len();
                if len > 0 {
                    self.state.log_state.select(Some(len - 1));
                }
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_crypto_action();
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_pause_action();
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.handle_stop_action();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.input_mode = InputMode::UnlinkConfirm;
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.restart_daemon();
            }
            _ => {}
        }
    }

    fn handle_unlink_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // Unlink shuts the daemon down; the TUI exits afterwards.
                let _ = self.send(DaemonCommand::Unlink);
                self.state.set_status_message(
                    "Account unlinked. Exiting...".into(),
                    StatusMessageKind::Success,
                );
                self.state.input_mode = InputMode::Normal;
                self.state.should_quit = true;
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.state.input_mode = InputMode::Normal;
            }
            _ => {}
        }
    }

    // ===== Backup operations =====

    fn handle_backup_add_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let path = self.state.input_buffer.trim().to_string();
                self.state.input_buffer.clear();
                self.state.input_mode = InputMode::Normal;
                if path.is_empty() {
                    self.state
                        .set_status_message("Path is empty".into(), StatusMessageKind::Error);
                } else {
                    self.do_backup_add(path);
                }
            }
            KeyCode::Esc => {
                self.state.input_buffer.clear();
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
            }
            _ => {}
        }
    }

    fn handle_backup_remove_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let id = self.state.selected_backup().map(|b| b.sync_id);
                self.state.input_mode = InputMode::Normal;
                if let Some(id) = id {
                    self.do_backup_remove(id);
                }
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.state.input_mode = InputMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_backup_stop_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.state.input_mode = InputMode::Normal;
                self.do_backup_stop_device();
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.state.input_mode = InputMode::Normal;
            }
            _ => {}
        }
    }

    fn do_backup_add(&mut self, path: String) {
        match self.send(DaemonCommand::BackupCreate { path }) {
            Some(DaemonResponse::BackupCreated { sync_id }) => self.state.set_status_message(
                format!("Backup created (id {})", sync_id),
                StatusMessageKind::Success,
            ),
            // Path-eligibility errors (e.g. inside an existing sync folder) are
            // raised by the C layer and surfaced here verbatim.
            Some(DaemonResponse::Error(e)) => {
                self.state.set_status_message(e, StatusMessageKind::Error)
            }
            _ => {}
        }
        self.refresh_backups();
    }

    fn do_backup_remove(&mut self, sync_id: u32) {
        self.run_action(
            DaemonCommand::BackupRemove { sync_id },
            &format!("Backup {} removed", sync_id),
        );
        self.refresh_backups();
    }

    fn do_backup_stop_device(&mut self) {
        self.run_action(DaemonCommand::BackupStopDevice, "Device backups stopped");
        self.refresh_backups();
    }

    /// Reload the backups list and device root name from the daemon.
    fn refresh_backups(&mut self) {
        match self.send(DaemonCommand::BackupList) {
            Some(DaemonResponse::BackupList(list)) => {
                self.state.backups = list;
                self.state.backup_error = None;
            }
            Some(DaemonResponse::Error(e)) => {
                self.state.backups.clear();
                self.state.backup_error = Some(e);
            }
            _ => {}
        }
        self.state.backup_root_name = match self.send(DaemonCommand::BackupRootName) {
            Some(DaemonResponse::BackupRootName(name)) => Some(name),
            _ => None,
        };
        self.state.clamp_backup_selection();
    }

    fn scroll_backup_up(&mut self) {
        if self.state.backups.is_empty() {
            return;
        }
        let i = self
            .state
            .backup_list_state
            .selected()
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0);
        self.state.backup_list_state.select(Some(i));
    }

    fn scroll_backup_down(&mut self) {
        let len = self.state.backups.len();
        if len == 0 {
            return;
        }
        let i = match self.state.backup_list_state.selected() {
            Some(i) if i + 1 < len => i + 1,
            Some(i) => i,
            None => 0,
        };
        self.state.backup_list_state.select(Some(i));
    }

    fn handle_auth_menu_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('1') => {
                self.initiate_web_login();
            }
            KeyCode::Char('2') => {
                self.state.input_mode = InputMode::AuthToken;
                self.state.input_buffer.clear();
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.state.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
            }
            _ => {}
        }
    }

    fn handle_auth_token_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let token = self.state.input_buffer.clone();
                self.state.input_buffer.clear();
                if !token.is_empty() {
                    self.submit_auth_token(token);
                }
            }
            KeyCode::Esc => {
                self.state.input_buffer.clear();
                self.state.input_mode = InputMode::AuthMenu;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
            }
            _ => {}
        }
    }

    fn handle_auth_waiting_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.state.scroll_offset = 0;
                self.state.input_mode = InputMode::AuthMenu;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.state.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
            }
            KeyCode::Char('o') | KeyCode::Char('O') => {
                if let InputMode::AuthWebWaiting(url) = &self.state.input_mode {
                    let url = url.clone();
                    match crate::utils::browser::open_url(&url, true) {
                        Ok(true) => self.state.set_status_message(
                            "Opened login URL in browser".into(),
                            StatusMessageKind::Success,
                        ),
                        Ok(false) | Err(_) => self.state.set_status_message(
                            "Could not open a browser. Copy the URL or scan the QR code.".into(),
                            StatusMessageKind::Error,
                        ),
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.scroll_offset = self.state.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.scroll_offset = self.state.scroll_offset.saturating_add(1);
            }
            _ => {}
        }
    }

    fn handle_password_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let action = if let InputMode::PasswordPrompt(action) = &self.state.input_mode {
                    action.clone()
                } else {
                    return;
                };
                let password = self.state.input_buffer.clone();
                zeroize_string(&mut self.state.input_buffer);

                match action {
                    CryptoAction::Unlock => {
                        self.do_crypto_unlock(password);
                    }
                    CryptoAction::Setup => {
                        // Stash the password, ask for hint
                        self.state.password_stash = Some(SecretString::from(password));
                        self.state.input_mode = InputMode::HintPrompt;
                        self.state.input_buffer.clear();
                    }
                }
            }
            KeyCode::Esc => {
                zeroize_string(&mut self.state.input_buffer);
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                self.state.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            _ => {}
        }
    }

    fn handle_hint_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                let hint = self.state.input_buffer.clone();
                self.state.input_buffer.clear();
                let password = self.state.password_stash.take();
                if let Some(pwd) = password {
                    self.do_crypto_setup(pwd, hint);
                }
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.state.input_buffer.clear();
                self.state.password_stash = None;
                self.state.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) => {
                self.state.input_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.state.input_buffer.pop();
            }
            _ => {}
        }
    }

    // ===== Crypto operations =====

    fn handle_crypto_action(&mut self) {
        match &self.state.crypto_state {
            CryptoState::Started => self.lock_crypto(),
            CryptoState::SetupComplete | CryptoState::Stopped => self.start_crypto_unlock(),
            CryptoState::NotSetup | CryptoState::Failed(_) => self.start_crypto_setup(),
        }
    }

    fn start_crypto_unlock(&mut self) {
        match self.state.crypto_state {
            CryptoState::Started => {
                self.state.set_status_message(
                    "Crypto already unlocked".into(),
                    StatusMessageKind::Success,
                );
                return;
            }
            CryptoState::NotSetup => {
                self.state.set_status_message(
                    "Crypto not set up. Press Ctrl+L to setup".into(),
                    StatusMessageKind::Error,
                );
                return;
            }
            _ => {}
        }
        self.state.input_mode = InputMode::PasswordPrompt(CryptoAction::Unlock);
        self.state.input_buffer.clear();
    }

    fn lock_crypto(&mut self) {
        if self.state.crypto_state != CryptoState::Started {
            self.state
                .set_status_message("Crypto already locked".into(), StatusMessageKind::Success);
            return;
        }
        self.run_action(DaemonCommand::StopCrypto, "Crypto locked");
        self.poll_status();
    }

    fn start_crypto_setup(&mut self) {
        if !matches!(
            self.state.crypto_state,
            CryptoState::NotSetup | CryptoState::Failed(_)
        ) {
            self.state
                .set_status_message("Crypto already set up".into(), StatusMessageKind::Success);
            return;
        }
        self.state.input_mode = InputMode::PasswordPrompt(CryptoAction::Setup);
        self.state.input_buffer.clear();
    }

    fn do_crypto_unlock(&mut self, password: String) {
        self.run_action(
            DaemonCommand::StartCrypto {
                password: Some(password),
            },
            "Crypto unlocked",
        );
        self.state.input_mode = InputMode::Normal;
        self.poll_status();
    }

    fn do_crypto_setup(&mut self, password: SecretString, hint: String) {
        self.run_action(
            DaemonCommand::SetupCrypto {
                password: password.expose_secret().to_string(),
                hint,
            },
            "Crypto set up successfully",
        );
        self.poll_status();
    }

    // ===== Sync engine operations (pause / resume / stop) =====

    fn handle_pause_action(&mut self) {
        match self.state.sync_engine_state() {
            SyncEngineState::Running => self.pause_sync(),
            SyncEngineState::Paused => self.resume_sync(),
            SyncEngineState::Stopped => self.state.set_status_message(
                "Sync is stopped \u{2014} press Ctrl+T to start".into(),
                StatusMessageKind::Error,
            ),
            SyncEngineState::Inactive => self.state.set_status_message(
                "Sync engine is not available".into(),
                StatusMessageKind::Error,
            ),
        }
    }

    fn handle_stop_action(&mut self) {
        match self.state.sync_engine_state() {
            SyncEngineState::Running | SyncEngineState::Paused => self.stop_sync(),
            SyncEngineState::Stopped => self.resume_sync(),
            SyncEngineState::Inactive => self.state.set_status_message(
                "Sync engine is not available".into(),
                StatusMessageKind::Error,
            ),
        }
    }

    fn pause_sync(&mut self) {
        match self.send(DaemonCommand::Pause) {
            Some(DaemonResponse::Ok) | Some(DaemonResponse::OkWithMessage(_)) => {
                self.state.set_status_code(PSTATUS_PAUSED);
                self.state
                    .set_status_message("Sync paused".into(), StatusMessageKind::Success);
            }
            Some(DaemonResponse::Error(e)) => self
                .state
                .set_status_message(format!("Failed to pause: {}", e), StatusMessageKind::Error),
            _ => {}
        }
        self.poll_status();
    }

    fn resume_sync(&mut self) {
        let was_stopped = self.state.sync_engine_state() == SyncEngineState::Stopped;
        match self.send(DaemonCommand::Resume) {
            Some(DaemonResponse::Ok) | Some(DaemonResponse::OkWithMessage(_)) => {
                self.state.set_status_code(PSTATUS_READY);
                let msg = if was_stopped {
                    "Sync started"
                } else {
                    "Sync resumed"
                };
                self.state
                    .set_status_message(msg.into(), StatusMessageKind::Success);
            }
            Some(DaemonResponse::Error(e)) => self
                .state
                .set_status_message(format!("Failed to resume: {}", e), StatusMessageKind::Error),
            _ => {}
        }
        self.poll_status();
    }

    fn stop_sync(&mut self) {
        match self.send(DaemonCommand::Stop) {
            Some(DaemonResponse::Ok) | Some(DaemonResponse::OkWithMessage(_)) => {
                self.state.set_status_code(PSTATUS_STOPPED);
                self.state
                    .set_status_message("Sync stopped".into(), StatusMessageKind::Success);
            }
            Some(DaemonResponse::Error(e)) => self
                .state
                .set_status_message(format!("Failed to stop: {}", e), StatusMessageKind::Error),
            _ => {}
        }
        self.poll_status();
    }

    // ===== Auth operations =====

    fn initiate_web_login(&mut self) {
        // The daemon requests the login session and waits for completion in the
        // background; on success it sets the auth token, which the next status
        // poll observes. We just display the URL/QR.
        match self.send(DaemonCommand::AuthBeginWeb) {
            Some(DaemonResponse::AuthWeb { url, .. }) => {
                self.state.input_mode = InputMode::AuthWebWaiting(url.clone());
                let _ = crate::utils::browser::open_url(&url, true);
            }
            Some(DaemonResponse::Error(e)) => self
                .state
                .set_status_message(format!("Web login failed: {}", e), StatusMessageKind::Error),
            _ => {}
        }
    }

    fn submit_auth_token(&mut self, token: String) {
        match self.send(DaemonCommand::SetAuthToken { token }) {
            Some(DaemonResponse::Ok) | Some(DaemonResponse::OkWithMessage(_)) => {
                self.state.set_status_message(
                    "Token set, authenticating...".into(),
                    StatusMessageKind::Success,
                );
            }
            Some(DaemonResponse::Error(e)) => self
                .state
                .set_status_message(format!("Token error: {}", e), StatusMessageKind::Error),
            _ => {}
        }
    }

    // ===== Scrolling =====

    fn scroll_log_up(&mut self) {
        let i = match self.state.log_state.selected() {
            Some(i) => {
                if i > 0 {
                    i - 1
                } else {
                    0
                }
            }
            None => 0,
        };
        self.state.log_state.select(Some(i));
    }

    fn scroll_log_down(&mut self) {
        let len = self.state.activity_log.len();
        if len == 0 {
            return;
        }
        let i = match self.state.log_state.selected() {
            Some(i) => {
                if i + 1 < len {
                    i + 1
                } else {
                    len - 1
                }
            }
            None => 0,
        };
        self.state.log_state.select(Some(i));
    }

    /// Attempt to (re)start the daemon after it became unavailable.
    fn restart_daemon(&mut self) {
        match crate::daemon::process::spawn_background_daemon(
            /* allow_unauthenticated = */ true,
        ) {
            Ok(()) => self
                .state
                .set_status_message("Restarting daemon...".into(), StatusMessageKind::Success),
            Err(e) => self
                .state
                .set_status_message(format!("Restart failed: {}", e), StatusMessageKind::Error),
        }
    }

    // ===== Tick / polling =====

    /// Fetch a full dashboard snapshot from the daemon and apply it.
    fn poll_status(&mut self) {
        match self.send(DaemonCommand::StatusFull) {
            Some(DaemonResponse::StatusFull(snap)) => self.apply_snapshot(*snap),
            _ => {
                if self.state.daemon_unavailable {
                    self.state.set_status_message(
                        "Daemon unavailable \u{2014} press Ctrl+R to restart".into(),
                        StatusMessageKind::Error,
                    );
                }
            }
        }
    }

    /// Fetch activity-log entries newer than the cursor and append them.
    fn poll_activity(&mut self) {
        if let Some(DaemonResponse::Activity { entries, cursor }) =
            self.send(DaemonCommand::ActivitySince {
                cursor: self.activity_cursor,
            })
        {
            for entry in entries {
                self.state.push_activity(entry);
            }
            self.activity_cursor = cursor;
        }
    }

    /// Periodic tick -- polls the daemon for state and activity over IPC.
    pub fn tick(&mut self) {
        self.state.clear_expired_status_message();
        self.poll_status();
        self.poll_activity();

        // Keep the backups list current while the Backups screen is shown.
        if self.state.active_screen == Screen::Backups {
            self.refresh_backups();
        }
    }
}
