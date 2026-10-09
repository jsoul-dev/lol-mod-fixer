//! lol-mod-fixer: Standalone native CLI tool for League of Legends mod diagnosis and repair.
//!
//! Reuses the official LeagueToolkit/ltk-manager repair and health checking engine.

pub mod beautify;
pub mod champions;
pub mod cleanup;
pub mod mapping;
pub mod migrate;
pub mod quarantine;
mod cli;
mod config;
mod engine;


mod error;
mod formats;
mod health;
mod output;
pub mod permissions;
mod repair;
mod replacement;
pub mod rose;
mod scanner;

#[cfg(test)]
mod tests;

use clap::Parser;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

use crate::cli::{Cli, Commands, ConfigAction};
use crate::config::{AppConfig, auto_detect_league_path, is_valid_league_dir};
use crate::engine::{ensure_hashtables, execute_check, execute_repair};
use crate::error::{FixerError, exit_codes};
use crate::output::Printer;

#[cfg(windows)]
fn enable_windows_ansi_support() {
    unsafe {
        type HANDLE = *mut std::ffi::c_void;
        type BOOL = i32;
        type DWORD = u32;

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;
            fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut DWORD) -> BOOL;
            fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> BOOL;
        }

        const STD_OUTPUT_HANDLE: DWORD = -11i32 as u32;
        const ENABLE_VIRTUAL_TERMINAL_PROCESSING: DWORD = 0x0004;

        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if !handle.is_null() && handle != (-1isize as *mut std::ffi::c_void) {
            let mut mode: DWORD = 0;
            if GetConsoleMode(handle, &mut mode) != 0 {
                SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}

fn main() -> ExitCode {
    #[cfg(windows)]
    enable_windows_ansi_support();

    let raw_args: Vec<String> = std::env::args().collect();
    let has_no_args = raw_args.len() <= 1;

    let cli = Cli::parse();

    // Check if Administrator elevation was explicitly requested
    let wants_elevate = cli.elevate
        || match &cli.command {
            Some(Commands::Check(args)) => args.elevate,
            Some(Commands::Repair(args)) => args.elevate,
            Some(Commands::Auto(args)) => args.elevate,
            _ => false,
        };

    if wants_elevate && !permissions::is_elevated() {
        if permissions::try_self_elevate() {
            return ExitCode::SUCCESS;
        } else {
            eprintln!("Failed to acquire elevated Administrator privileges via UAC.");
            return ExitCode::from(exit_codes::REPAIRABLE_OR_FAILED as u8);
        }
    }

    // Determine logging level:
    // Silence internal engine tracing by default so normal CLI output is clean and professional.
    // Use --verbose to display detailed debug logs.
    let log_level = if cli.verbose {
        "debug"
    } else {
        "off"
    };

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr) // Logs go to stderr so stdout remains clean JSON
        .without_time()
        .with_target(false)
        .with_ansi(false) // Disable raw ANSI escape sequences for clean Windows console output
        .init();

    // Sync or check hashtables
    if let Err(e) = ensure_hashtables(cli.sync_hashtables) {
        eprintln!("Hashtables initialization notice: {e}");
    }

    let app_config = AppConfig::load();
    let printer = Printer::new(cli.json, cli.verbose);

    // Determine if pause should be enabled
    let should_pause = if cli.no_pause {
        false
    } else if cli.pause {
        true
    } else if let Some(p) = app_config.pause_on_exit {
        p
    } else {
        // Default to true if launched with no arguments on Windows
        cfg!(windows) && has_no_args && !cli.json
    };

    let result = run_app(cli, &printer);

    let exit_code = match result {
        Ok(code) => code,
        Err(e) => {
            if printer.json {
                let err_json = serde_json::json!({
                    "error": e.to_string(),
                    "status": "failed"
                });
                println!("{err_json}");
            } else {
                eprintln!("\nError: {e}");
            }
            exit_codes::REPAIRABLE_OR_FAILED
        }
    };

    printer.maybe_pause(should_pause);

    ExitCode::from(exit_code as u8)
}

fn run_app(cli: Cli, printer: &Printer) -> Result<i32, FixerError> {
    let beautify = !cli.no_beautify;
    let cleanup = !cli.no_cleanup;
    let mapping = !cli.no_mapping;
    let migrate = !cli.no_migrate;
    let quarantine = !cli.no_quarantine && !cli.no_delete_unrepairable;
    let delete_unrepairable = cli.delete_unrepairable;

    match cli.command {
        Some(Commands::Check(args)) => {
            let target = args.path.or(args.dir).or(cli.dir).or(cli.target);
            let league = args.league.or(cli.league);
            let recursive = args.recursive || cli.recursive;
            let check_printer = Printer::new(args.json || cli.json, args.verbose || cli.verbose);
            let b = if args.no_beautify { false } else { beautify };
            execute_check(target, league, recursive, b, &check_printer)
        }
        Some(Commands::Repair(args)) => {
            let input = args.input.or(args.dir).or(cli.dir).or(cli.target);
            let output = args.output;
            let league = args.league.or(cli.league);
            let backup = args.backup || cli.backup;
            let dry_run = args.dry_run || cli.dry_run;
            let recursive = args.recursive || cli.recursive;
            let repair_printer = Printer::new(args.json || cli.json, args.verbose || cli.verbose);
            let b = if args.no_beautify { false } else { beautify };
            let c = if args.no_cleanup { false } else { cleanup };
            let m = if args.no_mapping { false } else { mapping };
            let mig = if args.no_migrate { false } else { migrate };
            let quaran = if args.no_quarantine || args.no_delete_unrepairable { false } else { quarantine };
            let del = if args.delete_unrepairable { true } else { delete_unrepairable };
            execute_repair(
                input,
                output,
                league,
                backup,
                dry_run,
                recursive,
                b,
                c,
                m,
                mig,
                quaran,
                del,
                &repair_printer,
            )
        }
        Some(Commands::Auto(args)) => {
            let input = args.input.or(args.dir).or(cli.dir).or(cli.target);
            let output = args.output;
            let league = args.league.or(cli.league);
            let backup = args.backup || cli.backup;
            let dry_run = args.dry_run || cli.dry_run;
            let recursive = args.recursive || cli.recursive;
            let repair_printer = Printer::new(args.json || cli.json, args.verbose || cli.verbose);
            let b = if args.no_beautify { false } else { beautify };
            let c = if args.no_cleanup { false } else { cleanup };
            let m = if args.no_mapping { false } else { mapping };
            let mig = if args.no_migrate { false } else { migrate };
            let quaran = if args.no_quarantine || args.no_delete_unrepairable { false } else { quarantine };
            let del = if args.delete_unrepairable { true } else { delete_unrepairable };
            execute_repair(
                input,
                output,
                league,
                backup,
                dry_run,
                recursive,
                b,
                c,
                m,
                mig,
                quaran,
                del,
                &repair_printer,
            )
        }
        Some(Commands::Config(args)) => {
            handle_config(args.action)?;
            Ok(exit_codes::SUCCESS)
        }
        None => {
            // Default mode: scan directory (containing executable, or explicit --dir / drag-and-drop) and repair.
            // Automatically recursive by default so nested mods (e.g. skins/<champ_id>/<mod_name>) are discovered.
            let target = cli.dir.or(cli.target);
            let league = cli.league;
            let backup = cli.backup;
            let dry_run = cli.dry_run;
            let recursive = true;
            execute_repair(
                target, None, league, backup, dry_run, recursive, beautify, cleanup, mapping, migrate, quarantine, delete_unrepairable, printer,
            )
        }
    }
}



fn handle_config(action: Option<ConfigAction>) -> Result<(), FixerError> {
    let mut config = AppConfig::load();

    match action.unwrap_or(ConfigAction::Show) {
        ConfigAction::Show => {
            let config_file = AppConfig::file_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            println!("Configuration file: {config_file}");
            println!(
                "Configured League path: {}",
                config
                    .league_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(none)".to_string())
            );
            println!(
                "Auto-detected League path: {}",
                auto_detect_league_path()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(not detected)".to_string())
            );
            println!(
                "Pause on exit: {}",
                config
                    .pause_on_exit
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "default (auto)".to_string())
            );
        }
        ConfigAction::SetLeague { path } => {
            if !is_valid_league_dir(&path) {
                eprintln!(
                    "Warning: '{}' does not appear to be a valid League install root or Game directory.",
                    path.display()
                );
            }
            config.league_path = Some(path.clone());
            config.save()?;
            println!("League path updated to: {}", path.display());
        }
        ConfigAction::SetPause { enabled } => {
            config.pause_on_exit = Some(enabled);
            config.save()?;
            println!("Pause on exit set to: {enabled}");
        }
    }

    Ok(())
}
