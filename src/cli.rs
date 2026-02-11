#[allow(unused_imports)]
use chrono::{DateTime, Utc};

use clap::{Parser, Subcommand, Args, ValueEnum};
use std::path::PathBuf;
use colored::*;

#[derive(Parser, Debug)]
#[command(
    name = "cleancrush",
    about = "Student-focused exam file cleanup tool with smart archiving",
    version,
    author,
    long_about = "CleanCrush helps students manage post-exam file clutter by\n\
                  intelligently tracking study files during exams and providing\n\
                  safe, privacy-first cleanup options afterward.\n\n\
                  Features:\n\
                  • Exam mode: Track study files during exams\n\
                  • Smart scanning: Find duplicates, old files, large files\n\
                  • Privacy-first: Never reads file contents\n\
                  • Safe cleanup: Recycle Bin or organized archive\n\
                  • Student-friendly: Gamification and encouragement"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Enable safe mode (preview only, no changes)
    #[arg(long, global = true)]
    pub safe: bool,
    
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
    
    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Show detailed help for specific command
    #[arg(long, short = 'H', global = true)]
    pub detailed_help: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Scan folder for study files and show summary
    Scan(ScanArgs),
    
    /// Show detailed cleanup suggestions with confidence scores
    Suggest(SuggestArgs),
    
    /// Clean files (delete or archive based on config)
    Clean(CleanArgs),
    
    /// Delete specific files by index or pattern
    Delete(DeleteArgs),
    
    /// Manage exam mode tracking
    #[command(subcommand)]
    Exam(ExamArgs),
    
    /// Manage protected folders
    #[command(subcommand)]
    Protect(ProtectArgs),
    
    /// Manage archive system
    #[command(subcommand)]
    Archive(ArchiveArgs),
    
    /// Manage schedule and reminders
    #[command(subcommand)]
    Schedule(ScheduleArgs),
    
    /// Show statistics and achievements
    Stats,
    
    /// Calculate folder cleanliness score
    Score(ScoreArgs),
    
    /// Show configuration
    Config,
    
    /// Show achievements and progress
    Achievements,

    /// Show help and examples
    ShowHelp,
    
    /// Show version information
    Version,
}

#[derive(Args, Debug)]
pub struct ScanArgs {
    /// Path to scan (default: Downloads folder)
    #[arg(default_value = ".")]
    pub path: PathBuf,
    
    /// Consider files older than N days as "old"
    #[arg(short = 'D', long, default_value_t = 60)]
    pub days: u64,
    
    /// Consider files larger than N MB as "large"
    #[arg(short = 's', long, default_value_t = 100)]
    pub large: u64,
    
    /// Show detailed file information
    #[arg(short = 'd', long)]
    pub detailed: bool,
    
    /// Maximum files to scan
    #[arg(long, default_value_t = 5000)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct SuggestArgs {
    /// Path to scan for suggestions
    #[arg(default_value = ".")]
    pub path: PathBuf,
    
    /// Minimum confidence score to show (0.0-1.0)
    #[arg(long, default_value_t = 0.4)]
    pub confidence: f32,
    
    /// Filter by category
    #[arg(long, value_enum)]
    pub category: Option<FileCategory>,
    
    /// Show all files, not just suggestions
    #[arg(long)]
    pub all: bool,
}

#[derive(Args, Debug)]
pub struct CleanArgs {
    /// Path to clean
    #[arg(default_value = ".")]
    pub path: PathBuf,
    
    /// Cleanup mode
    #[arg(long, value_enum, default_value_t = CleanMode::All)]
    pub mode: CleanMode,
    
    /// Days threshold for old files
    #[arg(long, default_value_t = 60)]
    pub days: u64,
    
    /// Dry run (show what would be done)
    #[arg(long)]
    pub dry_run: bool,
    
    /// Skip confirmation prompts
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// Path that was scanned (for context)
    #[arg(long)]
    pub path: Option<PathBuf>,
    
    /// File indices to delete (from suggest command)
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub indices: Vec<usize>,
    
    /// Delete all suggested files
    #[arg(long, conflicts_with = "indices")]
    pub all: bool,
    
    /// Delete only duplicate files
    #[arg(long, conflicts_with_all = &["indices", "all"])]
    pub duplicates: bool,
    
    /// Delete only old files (older than N days)
    #[arg(long, conflicts_with_all = &["indices", "all", "duplicates"])]
    pub old: Option<u64>,
    
    /// Delete only large files (larger than N MB)
    #[arg(long, conflicts_with_all = &["indices", "all", "duplicates"])]
    pub large: Option<u64>,
    
    /// Skip confirmation prompts
    #[arg(short = 'y', long)]
    pub yes: bool,
}


#[derive(Subcommand, Debug)]
pub enum ExamArgs {
    /// Start exam tracking
    On {
        /// Exam period name
        #[arg(short, long)]
        name: Option<String>,
    },
    
    /// Stop exam tracking
    Off,
    
    /// Set exam dates manually
    Set {
        /// Start date (YYYY-MM-DD)
        start_date: String,
        
        /// End date (YYYY-MM-DD)
        end_date: String,
        
        /// Exam period name
        #[arg(short, long)]
        name: Option<String>,
    },
    
    /// End exam and show cleanup options
    End,
    
    /// Show exam status
    Status,
    
    /// List tracked exam files
    List,
}

#[derive(Subcommand, Debug)]
pub enum ProtectArgs {
    /// Add folder to protection list
    Add {
        /// Folder to protect
        path: PathBuf,
        
        /// Protection type
        #[arg(long, value_enum, default_value_t = ProtectionTypeCli::Soft)]
        protection: ProtectionTypeCli,
    },
    
    /// Remove folder from protection list
    Remove {
        /// Folder to unprotect
        path: PathBuf,
    },
    
    /// List protected folders
    List,
    
    /// Clear all protected folders
    Clear,
}

#[derive(Subcommand, Debug)]
pub enum ArchiveArgs {
    /// List all archives
    List,
    
    /// Clean old archives
    Clean {
        /// Clean archives older than N days
        #[arg(default_value_t = 30)]
        days: i64,
        
        /// Skip confirmation
        #[arg(short = 'y', long)]
        yes: bool,
    },
    
    /// Show archive statistics
    Stats,
    
    /// Restore files from archive
    Restore {
        /// Archive date (YYYY-MM-DD) or "latest"
        date: String,
        
        /// File indices to restore
        indices: Vec<usize>,
        
        /// Restore all files from archive
        #[arg(long, conflicts_with = "indices")]
        all: bool,
        
        /// Restore to different location
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ScheduleArgs {
    /// Set reminder schedule
    Set {
        /// Schedule type
        #[arg(value_enum)]
        schedule: ScheduleType,
    },
    
    /// Show current schedule
    Show,
    
    /// Run scheduled cleanup now
    Run,
}

#[derive(Args, Debug)]
pub struct ScoreArgs {
    /// Path to score
    #[arg(default_value = ".")]
    pub path: PathBuf,
    
    /// Show detailed breakdown
    #[arg(short, long)]
    pub detailed: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum FileCategory {
    All,
    Duplicate,
    Old,
    Large,
    Lecture,
    Assignment,
    Reference,
    Other,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum CleanMode {
    /// Clean all suggested files
    All,
    /// Clean only duplicates
    Duplicates,
    /// Clean only old files
    Old,
    /// Clean only large files
    Large,
    /// Clean by confidence score
    Confidence,
    /// Interactive selection
    Interactive,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ProtectionTypeCli {
    /// Never scan folder
    Hard,
    /// Scan but warn before actions
    Soft,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ScheduleType {
    /// No reminders
    Never,
    /// Weekly reminders (Sunday)
    Weekly,
    /// Monthly reminders (1st of month)
    Monthly,
}

impl Cli {
    /// Print help with examples
    pub fn print_help() {
        println!("{}", "🧹 CLEANCRUSH - EXAM FILE CLEANUP TOOL".bold().green());
        println!();
        println!("{}", "USAGE:".bold());
        println!("  cleancrush [OPTIONS] <COMMAND>");
        println!();
        println!("{}", "OPTIONS:".bold());
        println!("  --safe           Safe mode (preview only, no changes)");
        println!("  -v, --verbose    Verbose output");
        println!("  --no-color       Disable colored output");
        println!("  -h, --help       Print help");
        println!("  -V, --version    Print version");
        println!();
        println!("{}", "COMMANDS:".bold());
        println!();
        println!("  {}  Scan folder for study files", "scan".cyan().bold());
        println!("      cleancrush scan ~/Downloads");
        println!("      cleancrush scan --days 90 --large 200");
        println!();
        println!("  {}  Show cleanup suggestions", "suggest".cyan().bold());
        println!("      cleancrush suggest ~/Downloads");
        println!("      cleancrush suggest --confidence 0.8");
        println!();
        println!("  {}  Clean files", "clean".cyan().bold());
        println!("      cleancrush clean --mode duplicates ~/Downloads");
        println!("      cleancrush clean --mode old --days 90");
        println!();
        println!("  {}  Delete specific files", "delete".cyan().bold());
        println!("      cleancrush delete 1 3 5 --path ~/Downloads");
        println!("      cleancrush delete --duplicates --path ~/Downloads");
        println!("      cleancrush delete --all --path ~/Downloads");
        println!();
        println!("  {}  Manage exam mode", "exam".cyan().bold());
        println!("      cleancrush exam on");
        println!("      cleancrush exam set 2024-12-01 2024-12-15");
        println!("      cleancrush exam status");
        println!("      cleancrush exam end");
        println!();
        println!("  {}  Manage protected folders", "protect".cyan().bold());
        println!("      cleancrush protect add ~/Documents");
        println!("      cleancrush protect add ~/Desktop --protection hard");
        println!("      cleancrush protect list");
        println!();
        println!("  {}  Manage archives", "archive".cyan().bold());
        println!("      cleancrush archive list");
        println!("      cleancrush archive clean --days 30");
        println!("      cleancrush archive stats");
        println!();
        println!("  {}  Manage reminders", "schedule".cyan().bold());
        println!("      cleancrush schedule set weekly");
        println!("      cleancrush schedule show");
        println!();
        println!("  {}  Show statistics", "stats".cyan().bold());
        println!("      cleancrush stats");
        println!();
        println!("  {}  Calculate cleanliness score", "score".cyan().bold());
        println!("      cleancrush score ~/Downloads");
        println!("      cleancrush score --detailed");
        println!();
        println!("  {}  Show configuration", "config".cyan().bold());
        println!("      cleancrush config");
        println!();
        println!("  {}  Show help", "help".cyan().bold());
        println!("      cleancrush help");
        println!();
        println!("{}", "EXAMPLES:".dimmed());
        println!("  # First-time setup");
        println!("  cleancrush");
        println!();
        println!("  # Regular cleanup workflow");
        println!("  cleancrush scan ~/Downloads");
        println!("  cleancrush suggest ~/Downloads");
        println!("  cleancrush delete 1 3 5 --path ~/Downloads");
        println!();
        println!("  # Exam mode workflow");
        println!("  cleancrush exam on");
        println!("  # ... study for exams ...");
        println!("  cleancrush exam end");
        println!();
        println!("  # Safe testing");
        println!("  cleancrush --safe scan ~/Downloads");
        println!("  cleancrush --safe delete --all --path ~/Downloads");
        println!();
        println!("{}", "PRIVACY PROMISE:".bold().cyan());
        println!("  • Never reads file contents");
        println!("  • Never sends data to cloud");
        println!("  • All operations are local");
        println!("  • Protected folders respected");
        println!();
        println!("{}", "SAFETY FEATURES:".bold().cyan());
        println!("  • All deletions go to Recycle Bin first");
        println!("  • 30-day restore window");
        println!("  • Confirmation prompts");
        println!("  --safe flag for testing");
    }
    
    /// Print version information
    pub fn print_version() {
        println!("🧹 CleanCrush v{}", env!("CARGO_PKG_VERSION"));
        println!("Student-focused exam file cleanup tool");
        println!("Repository: {}", env!("CARGO_PKG_REPOSITORY"));
        println!("License: {}", env!("CARGO_PKG_LICENSE"));
    }

    ///Print command specifific help
    pub fn print_command_help(command: &Commands) {
        let command_name = command.name();
    
    println!("{} Help for: {}", "ℹ️".cyan(), command_name.bold());
    println!();
        
        match command {
            Commands::Scan(_) => {
                println!("Scan directory for study files and show summary");
                println!();
                println!("Usage: cleancrush scan [PATH] [OPTIONS]");
                println!();
                println!("Arguments:");
                println!("  [PATH]                  Path to scan (default: current directory)");
                println!();
                println!("Options:");
                println!("  --days N                Consider files older than N days as 'old' (default: 60)");
                println!("  --large N               Consider files larger than N MB as 'large' (default: 100)");
                println!("  --detailed              Show detailed file information");
                println!("  --limit N               Maximum files to scan (default: 5000)");
                println!();
                println!("Examples:");
                println!("  cleancrush scan ~/Downloads");
                println!("  cleancrush scan --days 90 --large 200");
                println!("  cleancrush scan --detailed --limit 1000");
            }
            Commands::Suggest(_) => {
                println!("Show detailed cleanup suggestions with confidence scores");
                println!();
                println!("Usage: cleancrush suggest [PATH] [OPTIONS]");
                println!();
                println!("Arguments:");
                println!("  [PATH]                  Path to scan (default: current directory)");
                println!();
                println!("Options:");
                println!("  --confidence FLOAT      Minimum confidence score to show (0.0-1.0, default: 0.4)");
                println!("  --category CATEGORY     Filter by category (duplicate, old, large, lecture, assignment, reference, other)");
                println!("  --all                   Show all files, not just suggestions");
                println!();
                println!("Examples:");
                println!("  cleancrush suggest ~/Downloads");
                println!("  cleancrush suggest --confidence 0.8");
                println!("  cleancrush suggest --category duplicate");
            }
            Commands::Clean(_) => {
                println!("Clean files (delete or archive based on config)");
                println!();
                println!("Usage: cleancrush clean [PATH] [OPTIONS]");
                println!();
                println!("Arguments:");
                println!("  [PATH]                  Path to clean (default: current directory)");
                println!();
                println!("Options:");
                println!("  --mode MODE             Cleanup mode: all, duplicates, old, large, confidence, interactive (default: all)");
                println!("  --days N                Days threshold for old files (default: 60)");
                println!("  --dry-run               Dry run (show what would be done)");
                println!("  -y, --yes               Skip confirmation prompts");
                println!();
                println!("Examples:");
                println!("  cleancrush clean --mode duplicates ~/Downloads");
                println!("  cleancrush clean --mode old --days 90");
                println!("  cleancrush clean --dry-run --mode all");
            }
            Commands::Delete(_) => {
                println!("Delete specific files by index or pattern");
                println!();
                println!("Usage: cleancrush delete [OPTIONS] [INDICES...]");
                println!();
                println!("Arguments:");
                println!("  [INDICES...]            File indices to delete (from suggest command)");
                println!();
                println!("Options:");
                println!("  --path PATH             Path that was scanned (for context)");
                println!("  --all                   Delete all suggested files");
                println!("  --duplicates            Delete only duplicate files");
                println!("  --old [DAYS]            Delete only old files (older than N days)");
                println!("  --large [MB]            Delete only large files (larger than N MB)");
                println!("  -y, --yes               Skip confirmation prompts");
                println!();
                println!("Examples:");
                println!("  cleancrush delete 1 3 5 --path ~/Downloads");
                println!("  cleancrush delete --duplicates --path ~/Downloads");
                println!("  cleancrush delete --all --path ~/Downloads");
                println!("  cleancrush delete --old 90 --path ~/Downloads");
            }
            
            Commands::Achievements => {
                println!("Show achievements and progress");
                println!();
                println!("Usage: cleancrush achievements");
                println!();
                println!("Description:");
                println!("  Shows all achievements, both unlocked and locked.");
                println!("  Displays progress towards each achievement.");
                println!();
                println!("Examples:");
                println!("  cleancrush achievements");
            }
            _ => {
                println!("Run 'cleancrush help' for complete usage information");
                println!();
                println!("For detailed help on a specific command, use:");
                println!("  cleancrush {} --detailed-help", command_name);
            }
        }
    }
}

impl Commands {
    /// Get the command name
    pub fn name(&self) -> &'static str {
        match self {
            Commands::Scan(_) => "scan",
            Commands::Suggest(_) => "suggest",
            Commands::Clean(_) => "clean",
            Commands::Delete(_) => "delete",
            Commands::Exam(_) => "exam",
            Commands::Protect(_) => "protect",
            Commands::Archive(_) => "archive",
            Commands::Schedule(_) => "schedule",
            Commands::Stats => "stats",
            Commands::Score(_) => "score",
            Commands::Config => "config",
            Commands::Achievements => "achievements",
            Commands::ShowHelp => "help",
            Commands::Version => "version",
        }
    }
}