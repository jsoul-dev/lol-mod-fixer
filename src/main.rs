//! lol-mod-fixer: Standalone native CLI tool for League of Legends mod diagnosis and repair.
//!
//! Reuses the official LeagueToolkit/ltk-manager repair and health checking engine.

mod cli;
mod config;
mod engine;
mod error;
mod formats;
mod health;
mod output;
mod repair;
mod replacement;
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

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().collect();
    let has_no_args = raw_args.len() <= 1;

    let cli = Cli::parse();

    // Determine logging level
    let log_level = if cli.json {
        "warn"
    } else if cli.verbose {
        "debug"
    } else {
        "info"
    };

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr) // Crucial: all logs go to stderr so stdout remains clean JSON
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
    match cli.command {
        Some(Commands::Check(args)) => {
            let target = args.path.or(args.dir).or(cli.dir).or(cli.target);
            let league = args.league.or(cli.league);
            let recursive = args.recursive || cli.recursive;
            let check_printer = Printer::new(args.json || cli.json, args.verbose || cli.verbose);
            execute_check(target, league, recursive, &check_printer)
        }
        Some(Commands::Repair(args)) => {
            let input = args.input.or(args.dir).or(cli.dir).or(cli.target);
            let output = args.output;
            let league = args.league.or(cli.league);
            let backup = args.backup || cli.backup;
            let dry_run = args.dry_run || cli.dry_run;
            let recursive = args.recursive || cli.recursive;
            let repair_printer = Printer::new(args.json || cli.json, args.verbose || cli.verbose);
            execute_repair(
                input,
                output,
                league,
                backup,
                dry_run,
                recursive,
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
            execute_repair(
                input,
                output,
                league,
                backup,
                dry_run,
                recursive,
                &repair_printer,
            )
        }
        Some(Commands::Config(args)) => {
            handle_config(args.action)?;
            Ok(exit_codes::SUCCESS)
        }
        None => {
            // Default mode: scan directory (containing executable, or explicit --dir) and repair
            let target = cli.dir.or(cli.target);
            let league = cli.league;
            let backup = cli.backup;
            let dry_run = cli.dry_run;
            let recursive = cli.recursive;
            execute_repair(target, None, league, backup, dry_run, recursive, printer)
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
