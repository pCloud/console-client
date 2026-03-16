use std::sync::{Arc, Mutex};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use secrecy::SecretString;

use crate::ffi::raw;
use crate::ffi::types::{PSTATUS_BAD_LOGIN_DATA, PSTATUS_BAD_LOGIN_TOKEN, PSTATUS_LOGIN_REQUIRED};
use crate::security::zeroize_string;
use crate::utils::qrcode::generate_qr_code;
use crate::wrapper::{AuthState, CryptoState, PCloudClient, WebLoginConfig};

use super::event_types::TuiEvent;
use super::state::{ActivityEntry, CryptoAction, InputMode, StatusMessageKind, TuiState};

/// The main TUI application.
pub struct App {
    pub state: TuiState,
    client: Arc<Mutex<PCloudClient>>,
}

impl App {
    pub fn new(client: Arc<Mutex<PCloudClient>>) -> Self {
        Self {
            state: TuiState::new(),
            client,
        }
    }

    pub fn should_quit(&self) -> bool {
        self.state.should_quit
    }

    /// Handle a TUI event from the channel.
    pub fn handle_event(&mut self, event: TuiEvent) {
        match event {
            TuiEvent::StatusUpdate(snapshot) => {
                // Check if we need to show auth screen
                let needs_auth = matches!(
                    snapshot.status,
                    PSTATUS_LOGIN_REQUIRED | PSTATUS_BAD_LOGIN_DATA | PSTATUS_BAD_LOGIN_TOKEN
                );

                self.state.status = snapshot;

                if needs_auth && self.state.input_mode == InputMode::Normal {
                    self.state.auth_state = AuthState::NotAuthenticated;
                    self.state.input_mode = InputMode::AuthMenu;
                } else if !needs_auth {
                    if self.state.auth_state != AuthState::Authenticated {
                        self.state.auth_state = AuthState::Authenticated;
                    }
                    // If we were in auth mode, switch to normal
                    if matches!(
                        self.state.input_mode,
                        InputMode::AuthMenu | InputMode::AuthToken | InputMode::AuthWebWaiting(_)
                    ) {
                        self.state.input_mode = InputMode::Normal;
                    }
                }
            }
            TuiEvent::FileEvent {
                description,
                is_error,
            } => {
                let now = chrono_time();
                self.state.push_activity(ActivityEntry {
                    timestamp: now,
                    description,
                    is_error,
                });
            }
            TuiEvent::FsMounted => {
                self.state.fs_mounted = true;
            }
            TuiEvent::WebAuthResult(result) => match result {
                Ok(()) => {
                    self.state.auth_state = AuthState::Authenticated;
                    self.state.input_mode = InputMode::Normal;
                    self.state.set_status_message(
                        "Authentication successful!".into(),
                        StatusMessageKind::Success,
                    );
                }
                Err(e) => {
                    self.state.input_mode = InputMode::AuthMenu;
                    self.state.set_status_message(
                        format!("Auth failed: {}", e),
                        StatusMessageKind::Error,
                    );
                }
            },
            TuiEvent::Tick => {
                self.tick();
            }
            TuiEvent::Quit => {
                self.state.should_quit = true;
            }
            TuiEvent::Key(_) => {} // handled separately
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
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.state.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
            }
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
            KeyCode::Char('u') => {
                self.start_crypto_unlock();
            }
            KeyCode::Char('l') => {
                self.lock_crypto();
            }
            KeyCode::Char('S') => {
                self.start_crypto_setup();
            }
            _ => {}
        }
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
                self.state.input_mode = InputMode::AuthMenu;
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

    fn start_crypto_unlock(&mut self) {
        if let Ok(guard) = self.client.lock() {
            if guard.is_crypto_started() {
                self.state.set_status_message(
                    "Crypto already unlocked".into(),
                    StatusMessageKind::Success,
                );
                return;
            }
            if !guard.is_crypto_setup() {
                self.state.set_status_message(
                    "Crypto not set up. Press Shift+S to setup".into(),
                    StatusMessageKind::Error,
                );
                return;
            }
        }
        self.state.input_mode = InputMode::PasswordPrompt(CryptoAction::Unlock);
        self.state.input_buffer.clear();
    }

    fn lock_crypto(&mut self) {
        if let Ok(mut guard) = self.client.lock() {
            if !guard.is_crypto_started() {
                self.state
                    .set_status_message("Crypto already locked".into(), StatusMessageKind::Success);
                return;
            }
            match guard.stop_crypto() {
                Ok(()) => {
                    self.state.crypto_state = CryptoState::Stopped;
                    self.state
                        .set_status_message("Crypto locked".into(), StatusMessageKind::Success);
                }
                Err(e) => {
                    self.state.set_status_message(
                        format!("Failed to lock: {}", e),
                        StatusMessageKind::Error,
                    );
                }
            }
        }
    }

    fn start_crypto_setup(&mut self) {
        if let Ok(guard) = self.client.lock() {
            if guard.is_crypto_setup() {
                self.state
                    .set_status_message("Crypto already set up".into(), StatusMessageKind::Success);
                return;
            }
        }
        self.state.input_mode = InputMode::PasswordPrompt(CryptoAction::Setup);
        self.state.input_buffer.clear();
    }

    fn do_crypto_unlock(&mut self, password: String) {
        let secret = SecretString::from(password);
        if let Ok(mut guard) = self.client.lock() {
            match guard.start_crypto(&secret) {
                Ok(()) => {
                    self.state.crypto_state = CryptoState::Started;
                    self.state
                        .set_status_message("Crypto unlocked".into(), StatusMessageKind::Success);
                }
                Err(e) => {
                    self.state
                        .set_status_message(format!("Failed: {}", e), StatusMessageKind::Error);
                }
            }
        }
        self.state.input_mode = InputMode::Normal;
    }

    fn do_crypto_setup(&mut self, password: SecretString, hint: String) {
        if let Ok(mut guard) = self.client.lock() {
            match guard.setup_crypto(&password, &hint) {
                Ok(()) => {
                    self.state.crypto_state = CryptoState::SetupComplete;
                    self.state.set_status_message(
                        "Crypto set up successfully".into(),
                        StatusMessageKind::Success,
                    );
                }
                Err(e) => {
                    self.state.set_status_message(
                        format!("Setup failed: {}", e),
                        StatusMessageKind::Error,
                    );
                }
            }
        }
    }

    // ===== Auth operations =====

    fn initiate_web_login(&mut self) {
        // Acquire lock in a block so the guard is dropped before we use self again
        let session_result = {
            let mut guard = match self.client.lock() {
                Ok(g) => g,
                Err(_) => {
                    self.state.set_status_message(
                        "Failed to acquire lock".into(),
                        StatusMessageKind::Error,
                    );
                    return;
                }
            };
            guard.initiate_web_login(&WebLoginConfig::default())
        };

        match session_result {
            Ok(session) => {
                let url = session.login_url.clone();
                let _qr = generate_qr_code(&url).ok();
                self.state.input_mode = InputMode::AuthWebWaiting(url.clone());

                // Try to open browser
                let _ = crate::utils::browser::open_url(&url);

                // Spawn background thread for waiting
                let request_id = session.request_id.clone();
                std::thread::spawn(move || {
                    let _ = crate::wrapper::weblogin::wait_for_web_auth(&request_id);
                });
            }
            Err(e) => {
                self.state.set_status_message(
                    format!("Web login failed: {}", e),
                    StatusMessageKind::Error,
                );
            }
        }
    }

    fn submit_auth_token(&mut self, token: String) {
        let secret = SecretString::from(token);
        if let Ok(mut guard) = self.client.lock() {
            match guard.set_auth_token(&secret, true) {
                Ok(()) => {
                    self.state.set_status_message(
                        "Token set, authenticating...".into(),
                        StatusMessageKind::Success,
                    );
                }
                Err(e) => {
                    self.state.set_status_message(
                        format!("Token error: {}", e),
                        StatusMessageKind::Error,
                    );
                }
            }
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

    // ===== Tick =====

    /// Periodic tick -- polls PCloudClient for state changes.
    pub fn tick(&mut self) {
        self.state.clear_expired_status_message();

        if let Ok(mut guard) = self.client.lock() {
            // Refresh states
            guard.refresh_auth_state();
            guard.refresh_crypto_state();
            guard.refresh_mount_state();

            self.state.auth_state = guard.auth_state().clone();
            self.state.crypto_state = guard.crypto_state().clone();
            self.state.fs_mounted = guard.is_mounted();
            self.state.mountpoint = guard.mountpoint().map(|p| p.display().to_string());

            // Get account info
            self.state.account_email = guard.get_username();

            // Get quota
            let (used, total) = get_quota();
            self.state.quota_used = used;
            self.state.quota_total = total;

            // Check if auth completed while in web waiting mode
            if matches!(self.state.input_mode, InputMode::AuthWebWaiting(_))
                && self.state.auth_state == AuthState::Authenticated
            {
                self.state.input_mode = InputMode::Normal;
                self.state.set_status_message(
                    "Authentication successful!".into(),
                    StatusMessageKind::Success,
                );
            }
        }
    }
}

/// Get quota values from the C library settings DB.
fn get_quota() -> (u64, u64) {
    let used = unsafe {
        let key = std::ffi::CString::new("usedquota").unwrap();
        raw::psync_get_uint_value(key.as_ptr())
    };
    let total = unsafe {
        let key = std::ffi::CString::new("quota").unwrap();
        raw::psync_get_uint_value(key.as_ptr())
    };
    (used, total)
}

/// Get current time as HH:MM:SS string.
fn chrono_time() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now % 60;
    let mins = (now / 60) % 60;
    let hours = (now / 3600) % 24;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}
