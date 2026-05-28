//! Interactive command definitions for pCloud console client.
//!
//! This module defines the commands available in interactive mode,
//! which can be entered by the user when the client is running
//! with the `-o` (commands) flag.
//!
//! # Available Commands
//!
//! - `startcrypto` / `start` - Unlock encrypted folders
//! - `stopcrypto` / `stop` - Lock encrypted folders
//! - `finalize` - Finish sync and exit cleanly
//! - `status` / `s` - Show current status
//! - `quit` / `q` / `exit` - Exit the client
//! - `logout` / `lo` - Log out and clear saved credentials
//! - `unlink` / `ul` - Unlink account and clear all local data
//! - `help` / `h` / `?` - Show help

use std::fmt;
use std::io::{self, BufRead, Write};

/// Commands available in interactive mode.
///
/// These commands can be entered when the client is running in
/// interactive/commands mode (`-o` flag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractiveCommand {
    /// Start crypto - unlock encrypted folders.
    ///
    /// This command prompts for the crypto password and unlocks
    /// any encrypted folders in the pCloud account.
    StartCrypto,

    /// Stop crypto - lock encrypted folders.
    ///
    /// This command locks all encrypted folders, requiring the
    /// crypto password to access them again.
    StopCrypto,

    /// Finalize and exit cleanly.
    ///
    /// This command ensures all pending operations are completed
    /// and sync is finished before exiting.
    Finalize,

    /// Show current status.
    ///
    /// Displays information about the current state of the client,
    /// including authentication status, sync status, and crypto status.
    Status,

    /// Quit the client immediately.
    ///
    /// Exits the client without waiting for sync to complete.
    Quit,

    /// Log out and clear saved credentials.
    ///
    /// Clears the saved auth token but keeps local sync data.
    /// The client exits after logging out.
    Logout,

    /// Unlink account and clear all local data.
    ///
    /// Clears credentials AND all local sync data. This is destructive.
    /// The client exits after unlinking.
    Unlink,

    /// Show help text.
    ///
    /// Displays a list of available commands and their descriptions.
    Help,

    /// Register a local folder as a pCloud backup destination.
    BackupAdd(String),

    /// List configured backups for the current device.
    BackupList,

    /// Remove a backup by sync id (does not delete files).
    BackupRemove(u32),

    /// Stop all backups on the current device.
    BackupStopDevice,

    /// Show backup status, optionally filtered by sync id.
    BackupStatus(Option<u32>),

    /// Print the backup root folder name for this device.
    BackupRootName,

    /// Print help for the `backup` subcommand group.
    BackupHelp,

    /// Unknown or unrecognized command.
    ///
    /// Contains the original input that could not be parsed.
    Unknown(String),
}

impl InteractiveCommand {
    /// Parse a command from user input.
    ///
    /// The parsing is case-insensitive and supports multiple aliases
    /// for each command (e.g., "quit", "q", and "exit" all map to `Quit`).
    ///
    /// # Arguments
    ///
    /// * `input` - The raw user input string
    ///
    /// # Returns
    ///
    /// The parsed `InteractiveCommand` variant
    ///
    /// # Example
    ///
    /// ```
    /// use console_client::cli::InteractiveCommand;
    ///
    /// assert_eq!(
    ///     InteractiveCommand::parse("startcrypto"),
    ///     InteractiveCommand::StartCrypto
    /// );
    /// assert_eq!(
    ///     InteractiveCommand::parse("q"),
    ///     InteractiveCommand::Quit
    /// );
    /// ```
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim();
        let lower = trimmed.to_lowercase();

        // Bare `backup` -> backup-group help; `backup <verb> [args]` parses
        // into one of the BackupXxx variants. We do the split BEFORE the flat
        // single-word match below so multi-word backup forms aren't shunted
        // into Unknown.
        if lower == "backup" {
            return Self::BackupHelp;
        }
        if let Some(rest) = lower.strip_prefix("backup ") {
            // Preserve original-case args by re-slicing `trimmed`.
            let original_rest = trimmed[("backup".len())..].trim_start();
            return parse_backup_verb(original_rest, rest.trim());
        }

        match lower.as_str() {
            "startcrypto" | "start" | "crypto" => Self::StartCrypto,
            "stopcrypto" | "stop" => Self::StopCrypto,
            "finalize" | "fin" | "finish" => Self::Finalize,
            "status" | "s" | "stat" => Self::Status,
            "quit" | "q" | "exit" | "bye" => Self::Quit,
            "logout" | "lo" => Self::Logout,
            "unlink" | "ul" => Self::Unlink,
            "help" | "h" | "?" => Self::Help,
            "" => Self::Unknown(String::new()),
            other => Self::Unknown(other.to_string()),
        }
    }

    /// Check if the command should cause the client to exit.
    pub fn is_exit_command(&self) -> bool {
        matches!(
            self,
            Self::Quit | Self::Finalize | Self::Logout | Self::Unlink
        )
    }

    /// Check if the command is a crypto-related operation.
    pub fn is_crypto_command(&self) -> bool {
        matches!(self, Self::StartCrypto | Self::StopCrypto)
    }

    /// Get a short description of the command.
    pub fn description(&self) -> &'static str {
        match self {
            Self::StartCrypto => "Unlock encrypted folders",
            Self::StopCrypto => "Lock encrypted folders",
            Self::Finalize => "Finish sync and exit cleanly",
            Self::Status => "Show current status",
            Self::Quit => "Exit the client immediately",
            Self::Logout => "Log out and clear saved credentials",
            Self::Unlink => "Unlink account and clear all local data",
            Self::Help => "Show available commands",
            Self::BackupAdd(_) => "Register a local folder as a pCloud backup",
            Self::BackupList => "List backups configured for this device",
            Self::BackupRemove(_) => "Remove a backup by sync id",
            Self::BackupStopDevice => "Stop all backups on this device",
            Self::BackupStatus(_) => "Show backup status",
            Self::BackupRootName => "Print the backup root folder name",
            Self::BackupHelp => "Show backup subcommand help",
            Self::Unknown(_) => "Unknown command",
        }
    }

    /// Display help text for all available commands.
    ///
    /// Prints a formatted list of all commands with their aliases
    /// and descriptions to stdout.
    pub fn print_help() {
        println!("Available commands:");
        println!();
        println!("  startcrypto, start  - Unlock encrypted folders");
        println!("  stopcrypto, stop    - Lock encrypted folders");
        println!("  finalize, fin       - Finish sync and exit cleanly");
        println!("  status, s           - Show current status");
        println!("  quit, q, exit       - Exit the client");
        println!("  logout, lo          - Log out and clear saved credentials");
        println!("  unlink, ul          - Unlink account and clear all local data");
        println!("  help, h, ?          - Show this help");
        println!();
        Self::print_backup_help();
    }

    /// Print help for the `backup` subcommand group.
    pub fn print_backup_help() {
        println!("Backup commands:");
        println!();
        println!("  backup add <path>     - Register a local folder as a backup");
        println!("  backup list           - List backups for this device");
        println!("  backup remove <id>    - Remove a backup by sync id");
        println!("  backup stop-device    - Stop all backups on this device");
        println!("  backup status [id]    - Show backup status (optional sync id)");
        println!("  backup root-name      - Print the backup root folder name");
        println!();
    }

    /// Display help text to the given writer.
    ///
    /// This variant is useful for testing or writing to files.
    pub fn write_help<W: Write>(mut writer: W) -> io::Result<()> {
        writeln!(writer, "Available commands:")?;
        writeln!(writer)?;
        writeln!(writer, "  startcrypto, start  - Unlock encrypted folders")?;
        writeln!(writer, "  stopcrypto, stop    - Lock encrypted folders")?;
        writeln!(
            writer,
            "  finalize, fin       - Finish sync and exit cleanly"
        )?;
        writeln!(writer, "  status, s           - Show current status")?;
        writeln!(writer, "  quit, q, exit       - Exit the client")?;
        writeln!(
            writer,
            "  logout, lo          - Log out and clear saved credentials"
        )?;
        writeln!(
            writer,
            "  unlink, ul          - Unlink account and clear all local data"
        )?;
        writeln!(writer, "  help, h, ?          - Show this help")?;
        writeln!(writer)?;
        writeln!(writer, "Backup commands:")?;
        writeln!(writer)?;
        writeln!(
            writer,
            "  backup add <path>     - Register a local folder as a backup"
        )?;
        writeln!(
            writer,
            "  backup list           - List backups for this device"
        )?;
        writeln!(
            writer,
            "  backup remove <id>    - Remove a backup by sync id"
        )?;
        writeln!(
            writer,
            "  backup stop-device    - Stop all backups on this device"
        )?;
        writeln!(
            writer,
            "  backup status [id]    - Show backup status (optional sync id)"
        )?;
        writeln!(
            writer,
            "  backup root-name      - Print the backup root folder name"
        )?;
        writeln!(writer)?;
        Ok(())
    }
}

/// Parse a backup sub-verb plus optional arguments.
///
/// `original_rest` preserves case for path arguments; `lower_rest` is the
/// already-lowercased form used to dispatch on the verb name.
fn parse_backup_verb(original_rest: &str, lower_rest: &str) -> InteractiveCommand {
    let (verb_lower, args_lower) = match lower_rest.split_once(' ') {
        Some((v, rest)) => (v, rest.trim()),
        None => (lower_rest, ""),
    };
    let (_, args_original) = match original_rest.split_once(' ') {
        Some((_, rest)) => ("", rest.trim()),
        None => ("", ""),
    };

    match verb_lower {
        "add" => {
            if args_original.is_empty() {
                InteractiveCommand::Unknown("backup add (missing path)".to_string())
            } else {
                InteractiveCommand::BackupAdd(args_original.to_string())
            }
        }
        "list" | "ls" => InteractiveCommand::BackupList,
        "remove" | "rm" | "delete" | "del" => match args_lower.parse::<u32>() {
            Ok(id) => InteractiveCommand::BackupRemove(id),
            Err(_) => InteractiveCommand::Unknown(format!("backup remove {}", args_lower)),
        },
        "stop-device" | "stopdevice" => InteractiveCommand::BackupStopDevice,
        "status" | "stat" => {
            if args_lower.is_empty() {
                InteractiveCommand::BackupStatus(None)
            } else {
                match args_lower.parse::<u32>() {
                    Ok(id) => InteractiveCommand::BackupStatus(Some(id)),
                    Err(_) => InteractiveCommand::Unknown(format!("backup status {}", args_lower)),
                }
            }
        }
        "root-name" | "rootname" | "root" => InteractiveCommand::BackupRootName,
        "help" | "h" | "?" => InteractiveCommand::BackupHelp,
        other => InteractiveCommand::Unknown(format!("backup {}", other)),
    }
}

impl fmt::Display for InteractiveCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartCrypto => write!(f, "startcrypto"),
            Self::StopCrypto => write!(f, "stopcrypto"),
            Self::Finalize => write!(f, "finalize"),
            Self::Status => write!(f, "status"),
            Self::Quit => write!(f, "quit"),
            Self::Logout => write!(f, "logout"),
            Self::Unlink => write!(f, "unlink"),
            Self::Help => write!(f, "help"),
            Self::BackupAdd(p) => write!(f, "backup add {}", p),
            Self::BackupList => write!(f, "backup list"),
            Self::BackupRemove(id) => write!(f, "backup remove {}", id),
            Self::BackupStopDevice => write!(f, "backup stop-device"),
            Self::BackupStatus(None) => write!(f, "backup status"),
            Self::BackupStatus(Some(id)) => write!(f, "backup status {}", id),
            Self::BackupRootName => write!(f, "backup root-name"),
            Self::BackupHelp => write!(f, "backup help"),
            Self::Unknown(s) => write!(f, "unknown({})", s),
        }
    }
}

/// Interactive command prompt handler.
///
/// This struct provides a simple REPL (Read-Eval-Print Loop) for
/// handling user commands in interactive mode.
pub struct CommandPrompt {
    prompt: String,
}

impl CommandPrompt {
    /// Create a new command prompt with the given prompt string.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt string to display (e.g., "pcloud> ")
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
        }
    }

    /// Create a command prompt with the default prompt string.
    pub fn default_prompt() -> Self {
        Self::new("pcloud> ")
    }

    /// Read a single command from stdin.
    ///
    /// Displays the prompt, reads a line of input, and parses it
    /// into an `InteractiveCommand`.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(command))` if a command was successfully read
    /// - `Ok(None)` if EOF was reached (e.g., Ctrl+D)
    /// - `Err(e)` if an I/O error occurred
    pub fn read_command(&self) -> io::Result<Option<InteractiveCommand>> {
        print!("{}", self.prompt);
        io::stdout().flush()?;

        let stdin = io::stdin();
        let mut line = String::new();

        match stdin.lock().read_line(&mut line) {
            Ok(0) => Ok(None), // EOF
            Ok(_) => Ok(Some(InteractiveCommand::parse(&line))),
            Err(e) => Err(e),
        }
    }

    /// Get the prompt string.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// Set a new prompt string.
    pub fn set_prompt(&mut self, prompt: impl Into<String>) {
        self.prompt = prompt.into();
    }
}

impl Default for CommandPrompt {
    fn default() -> Self {
        Self::default_prompt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_startcrypto() {
        assert_eq!(
            InteractiveCommand::parse("startcrypto"),
            InteractiveCommand::StartCrypto
        );
        assert_eq!(
            InteractiveCommand::parse("start"),
            InteractiveCommand::StartCrypto
        );
        assert_eq!(
            InteractiveCommand::parse("STARTCRYPTO"),
            InteractiveCommand::StartCrypto
        );
        assert_eq!(
            InteractiveCommand::parse("  start  "),
            InteractiveCommand::StartCrypto
        );
    }

    #[test]
    fn test_parse_stopcrypto() {
        assert_eq!(
            InteractiveCommand::parse("stopcrypto"),
            InteractiveCommand::StopCrypto
        );
        assert_eq!(
            InteractiveCommand::parse("stop"),
            InteractiveCommand::StopCrypto
        );
        assert_eq!(
            InteractiveCommand::parse("STOP"),
            InteractiveCommand::StopCrypto
        );
    }

    #[test]
    fn test_parse_finalize() {
        assert_eq!(
            InteractiveCommand::parse("finalize"),
            InteractiveCommand::Finalize
        );
        assert_eq!(
            InteractiveCommand::parse("fin"),
            InteractiveCommand::Finalize
        );
        assert_eq!(
            InteractiveCommand::parse("finish"),
            InteractiveCommand::Finalize
        );
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(
            InteractiveCommand::parse("status"),
            InteractiveCommand::Status
        );
        assert_eq!(InteractiveCommand::parse("s"), InteractiveCommand::Status);
        assert_eq!(
            InteractiveCommand::parse("stat"),
            InteractiveCommand::Status
        );
    }

    #[test]
    fn test_parse_quit() {
        assert_eq!(InteractiveCommand::parse("quit"), InteractiveCommand::Quit);
        assert_eq!(InteractiveCommand::parse("q"), InteractiveCommand::Quit);
        assert_eq!(InteractiveCommand::parse("exit"), InteractiveCommand::Quit);
        assert_eq!(InteractiveCommand::parse("bye"), InteractiveCommand::Quit);
    }

    #[test]
    fn test_parse_help() {
        assert_eq!(InteractiveCommand::parse("help"), InteractiveCommand::Help);
        assert_eq!(InteractiveCommand::parse("h"), InteractiveCommand::Help);
        assert_eq!(InteractiveCommand::parse("?"), InteractiveCommand::Help);
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(
            InteractiveCommand::parse("foobar"),
            InteractiveCommand::Unknown("foobar".to_string())
        );
        assert_eq!(
            InteractiveCommand::parse(""),
            InteractiveCommand::Unknown(String::new())
        );
    }

    #[test]
    fn test_parse_logout() {
        assert_eq!(
            InteractiveCommand::parse("logout"),
            InteractiveCommand::Logout
        );
        assert_eq!(InteractiveCommand::parse("lo"), InteractiveCommand::Logout);
        assert_eq!(
            InteractiveCommand::parse("LOGOUT"),
            InteractiveCommand::Logout
        );
    }

    #[test]
    fn test_parse_unlink() {
        assert_eq!(
            InteractiveCommand::parse("unlink"),
            InteractiveCommand::Unlink
        );
        assert_eq!(InteractiveCommand::parse("ul"), InteractiveCommand::Unlink);
        assert_eq!(
            InteractiveCommand::parse("UNLINK"),
            InteractiveCommand::Unlink
        );
    }

    #[test]
    fn test_is_exit_command() {
        assert!(InteractiveCommand::Quit.is_exit_command());
        assert!(InteractiveCommand::Finalize.is_exit_command());
        assert!(InteractiveCommand::Logout.is_exit_command());
        assert!(InteractiveCommand::Unlink.is_exit_command());
        assert!(!InteractiveCommand::Status.is_exit_command());
        assert!(!InteractiveCommand::StartCrypto.is_exit_command());
    }

    #[test]
    fn test_is_crypto_command() {
        assert!(InteractiveCommand::StartCrypto.is_crypto_command());
        assert!(InteractiveCommand::StopCrypto.is_crypto_command());
        assert!(!InteractiveCommand::Quit.is_crypto_command());
        assert!(!InteractiveCommand::Status.is_crypto_command());
    }

    #[test]
    fn test_display() {
        assert_eq!(
            format!("{}", InteractiveCommand::StartCrypto),
            "startcrypto"
        );
        assert_eq!(format!("{}", InteractiveCommand::Quit), "quit");
        assert_eq!(
            format!("{}", InteractiveCommand::Unknown("foo".to_string())),
            "unknown(foo)"
        );
    }

    #[test]
    fn test_description() {
        assert!(!InteractiveCommand::StartCrypto.description().is_empty());
        assert!(!InteractiveCommand::Quit.description().is_empty());
        assert!(!InteractiveCommand::Unknown("x".to_string())
            .description()
            .is_empty());
    }

    #[test]
    fn test_write_help() {
        let mut buffer = Vec::new();
        InteractiveCommand::write_help(&mut buffer).unwrap();
        let output = String::from_utf8(buffer).unwrap();

        assert!(output.contains("startcrypto"));
        assert!(output.contains("stopcrypto"));
        assert!(output.contains("finalize"));
        assert!(output.contains("status"));
        assert!(output.contains("quit"));
        assert!(output.contains("logout"));
        assert!(output.contains("unlink"));
        assert!(output.contains("help"));
    }

    #[test]
    fn test_command_prompt_new() {
        let prompt = CommandPrompt::new("test> ");
        assert_eq!(prompt.prompt(), "test> ");
    }

    #[test]
    fn test_command_prompt_default() {
        let prompt = CommandPrompt::default();
        assert_eq!(prompt.prompt(), "pcloud> ");
    }

    #[test]
    fn test_command_prompt_set_prompt() {
        let mut prompt = CommandPrompt::default();
        prompt.set_prompt("new> ");
        assert_eq!(prompt.prompt(), "new> ");
    }

    #[test]
    fn test_clone_and_eq() {
        let cmd = InteractiveCommand::StartCrypto;
        let cloned = cmd.clone();
        assert_eq!(cmd, cloned);
    }

    // ========================================================================
    // Backup parse tests
    // ========================================================================

    #[test]
    fn test_parse_backup_bare_word_is_help() {
        assert_eq!(
            InteractiveCommand::parse("backup"),
            InteractiveCommand::BackupHelp
        );
        assert_eq!(
            InteractiveCommand::parse("BACKUP"),
            InteractiveCommand::BackupHelp
        );
    }

    #[test]
    fn test_parse_backup_add_with_path() {
        // Path case is preserved.
        assert_eq!(
            InteractiveCommand::parse("backup add /tmp/Foo"),
            InteractiveCommand::BackupAdd("/tmp/Foo".to_string())
        );
    }

    #[test]
    fn test_parse_backup_add_without_path_is_unknown() {
        match InteractiveCommand::parse("backup add") {
            InteractiveCommand::Unknown(s) => assert!(s.contains("missing path")),
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_backup_list_and_alias() {
        assert_eq!(
            InteractiveCommand::parse("backup list"),
            InteractiveCommand::BackupList
        );
        assert_eq!(
            InteractiveCommand::parse("backup ls"),
            InteractiveCommand::BackupList
        );
    }

    #[test]
    fn test_parse_backup_remove() {
        assert_eq!(
            InteractiveCommand::parse("backup remove 5"),
            InteractiveCommand::BackupRemove(5)
        );
        assert_eq!(
            InteractiveCommand::parse("backup rm 42"),
            InteractiveCommand::BackupRemove(42)
        );
    }

    #[test]
    fn test_parse_backup_remove_bad_id_is_unknown() {
        match InteractiveCommand::parse("backup remove abc") {
            InteractiveCommand::Unknown(s) => assert!(s.contains("backup remove")),
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_backup_stop_device() {
        assert_eq!(
            InteractiveCommand::parse("backup stop-device"),
            InteractiveCommand::BackupStopDevice
        );
        assert_eq!(
            InteractiveCommand::parse("backup stopdevice"),
            InteractiveCommand::BackupStopDevice
        );
    }

    #[test]
    fn test_parse_backup_status() {
        assert_eq!(
            InteractiveCommand::parse("backup status"),
            InteractiveCommand::BackupStatus(None)
        );
        assert_eq!(
            InteractiveCommand::parse("backup status 7"),
            InteractiveCommand::BackupStatus(Some(7))
        );
    }

    #[test]
    fn test_parse_backup_root_name() {
        assert_eq!(
            InteractiveCommand::parse("backup root-name"),
            InteractiveCommand::BackupRootName
        );
        assert_eq!(
            InteractiveCommand::parse("backup rootname"),
            InteractiveCommand::BackupRootName
        );
    }

    #[test]
    fn test_parse_backup_unknown_verb() {
        match InteractiveCommand::parse("backup foobar") {
            InteractiveCommand::Unknown(s) => assert!(s.contains("backup foobar")),
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_backup_help_in_write_help() {
        let mut buf = Vec::new();
        InteractiveCommand::write_help(&mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("backup add"));
        assert!(out.contains("backup list"));
        assert!(out.contains("backup remove"));
        assert!(out.contains("backup status"));
        assert!(out.contains("backup root-name"));
    }
}
