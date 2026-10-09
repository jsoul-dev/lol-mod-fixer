//! Command-line interface definitions using clap.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "lol-mod-fixer",
    author = "League Mod Fixer Team",
    version,
    about = "Portable native CLI tool for League of Legends mod diagnosis and repair (reusing LTK Manager)",
    long_about = "A standalone CLI application to inspect and repair League of Legends mod archives (.fantome).\n\
                  Reuses the official LeagueToolkit/ltk-manager health and repair engine without requiring a GUI."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Target path (file or folder). If omitted, defaults to the directory containing this executable.
    #[arg(value_name = "TARGET")]
    pub target: Option<PathBuf>,

    /// Explicit directory to scan.
    #[arg(short, long, value_name = "DIR")]
    pub dir: Option<PathBuf>,

    /// Path to League of Legends installation root or Game directory.
    #[arg(short, long, value_name = "LEAGUE_DIR")]
    pub league: Option<PathBuf>,

    /// Output results in JSON format on stdout (diagnostics sent to stderr).
    #[arg(long)]
    pub json: bool,

    /// Scan and report problems without writing or modifying files.
    #[arg(long)]
    pub dry_run: bool,

    /// Create a .bak backup before replacing any repaired archive.
    #[arg(long)]
    pub backup: bool,

    /// Recursively scan subdirectories for mod archives.
    #[arg(short, long)]
    pub recursive: bool,

    /// Verbose output (show detailed problem descriptions).
    #[arg(short, long)]
    pub verbose: bool,

    /// Force interactive pause prompt ("Press Enter to exit...") at the end.
    #[arg(long)]
    pub pause: bool,

    /// Disable interactive pause prompt at the end.
    #[arg(long)]
    pub no_pause: bool,

    /// Synchronize or update local hashtables from mimir before starting.
    #[arg(long)]
    pub sync_hashtables: bool,

    /// Request Administrator elevation via UAC to unlock restricted mod folders.
    #[arg(long)]
    pub elevate: bool,

    /// Disable automatic beautification of mod names and folders.
    #[arg(long)]
    pub no_beautify: bool,

    /// Disable automatic cleanup of empty folders and orphan Rose manifests.
    #[arg(long)]
    pub no_cleanup: bool,

    /// Disable automatic generation of skin folder ID mappings.
    #[arg(long)]
    pub no_mapping: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Inspect mods without modifying anything.
    Check(TargetArgs),

    /// Scan and repair mod archives.
    Repair(RepairArgs),

    /// Automatic check, diagnosis, repair, and verification.
    Auto(RepairArgs),

    /// View or update tool configuration (League path, preferences).
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
pub struct TargetArgs {
    /// Mod file or folder to check. Defaults to executable directory if omitted.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    /// Explicit directory to scan.
    #[arg(short, long, value_name = "DIR")]
    pub dir: Option<PathBuf>,

    /// Path to League of Legends installation.
    #[arg(short, long, value_name = "LEAGUE_DIR")]
    pub league: Option<PathBuf>,

    /// Output results in JSON format on stdout.
    #[arg(long)]
    pub json: bool,

    /// Recursively scan subdirectories.
    #[arg(short, long)]
    pub recursive: bool,

    /// Verbose diagnostic output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Request Administrator elevation via UAC to unlock restricted mod folders.
    #[arg(long)]
    pub elevate: bool,

    /// Disable automatic beautification of mod names and folders.
    #[arg(long)]
    pub no_beautify: bool,

    /// Disable automatic cleanup of empty folders and orphan Rose manifests.
    #[arg(long)]
    pub no_cleanup: bool,

    /// Disable automatic generation of skin folder ID mappings.
    #[arg(long)]
    pub no_mapping: bool,
}

#[derive(Args, Debug)]
pub struct RepairArgs {
    /// Input mod archive or directory. Defaults to executable directory if omitted.
    #[arg(value_name = "INPUT")]
    pub input: Option<PathBuf>,

    /// Output repaired file path (only valid when repairing a single mod file).
    #[arg(value_name = "OUTPUT")]
    pub output: Option<PathBuf>,

    /// Explicit directory to scan and repair.
    #[arg(short, long, value_name = "DIR")]
    pub dir: Option<PathBuf>,

    /// Path to League of Legends installation.
    #[arg(short, long, value_name = "LEAGUE_DIR")]
    pub league: Option<PathBuf>,

    /// Output results in JSON format on stdout.
    #[arg(long)]
    pub json: bool,

    /// Don't modify files, report what would be repaired.
    #[arg(long)]
    pub dry_run: bool,

    /// Create .bak backup before replacing.
    #[arg(long)]
    pub backup: bool,

    /// Recursively scan subdirectories.
    #[arg(short, long)]
    pub recursive: bool,

    /// Verbose diagnostic output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Explicitly allow in-place replacement for single file.
    #[arg(long)]
    pub in_place: bool,

    /// Request Administrator elevation via UAC to unlock restricted mod folders.
    #[arg(long)]
    pub elevate: bool,

    /// Disable automatic beautification of mod names and folders.
    #[arg(long)]
    pub no_beautify: bool,

    /// Disable automatic cleanup of empty folders and orphan Rose manifests.
    #[arg(long)]
    pub no_cleanup: bool,

    /// Disable automatic generation of skin folder ID mappings.
    #[arg(long)]
    pub no_mapping: bool,
}


#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration and detected paths.
    Show,

    /// Set persistent League of Legends path.
    SetLeague {
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },

    /// Set persistent pause-on-exit preference (true / false).
    SetPause {
        #[arg(value_name = "ENABLED")]
        enabled: bool,
    },
}
