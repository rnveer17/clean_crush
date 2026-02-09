mod config;
mod scanner;
mod exam;
mod archive;
mod gamification;
mod cli;

use anyhow::{Result, Context};
use clap::Parser;
use colored::*;
use chrono::Utc;
use std::path::PathBuf;
use std::fs;
use dirs;
use crate::cli::{Cli, Commands};
use crate::config::{Config, ProtectedFolder, ProtectionType, ReminderSchedule};
use crate::scanner::Scanner;
use crate::exam::{ExamManager, PostExamChoice};
use crate::archive::ArchiveSystem;
use crate::gamification::{Gamification, CleanupType};

const DEFAULT_OLD_DAYS: u64 = 60;
const DEFAULT_LARGE_MB: u64 = 100;
const ENCOURAGEMENTS: &[&str] = &[
    "✨ Your folder is 72% cleaner than last week!",
    "💖 Small steps beat big chaos. You've got this!",
    "🔥 Streak +1! Your consistency is inspiring!",
    "🎓 Exam reset complete! Space for new learnings.",
    "🌸 Fresh start achieved. Proud of you!",
    "🧹 Look at you go! Making digital space for growth.",
    "💫 Every cleaned file is a step toward focus.",
    "🌟 Organized space, organized mind. Great job!",
];
/// Unified FileCategory enum
#[derive(Debug, Clone, PartialEq)]
pub enum FileCategory {
    Lecture,
    Assignment,
    Reference,
    Other,
    Duplicate,
    Old,
    Large,
}
pub mod colors {
    use colored::Color;
    
    pub const HIGH_CONFIDENCE: Color = Color::TrueColor { r: 255, g: 107, b: 157 };
    pub const MEDIUM_CONFIDENCE: Color = Color::TrueColor { r: 255, g: 154, b: 61 };
    pub const LOW_CONFIDENCE: Color = Color::TrueColor { r: 77, g: 150, b: 255 };
    pub const SUCCESS: Color = Color::TrueColor { r: 77, g: 255, b: 157 };
    pub const HEADER: Color = Color::TrueColor { r: 157, g: 77, b: 255 };
    pub const PATH: Color = Color::TrueColor { r: 77, g: 195, b: 255 };
    pub const WARNING: Color = Color::TrueColor { r: 255, g: 217, b: 61 };
}

fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();
    
    // Disable colors if requested
    if cli.no_color {
        colored::control::set_override(false);
    }
    
    // Handle help and version commands first
    match cli.command {
        Commands::ShowHelp => {
            Cli::print_help();
            return Ok(());
        }
        Commands::Version => {
            Cli::print_version();
            return Ok(());
        }
        _ => {}
    }

    // Handle detailed help flag
    if cli.detailed_help {
        Cli::print_command_help(&cli.command);
        return Ok(());
    }

    // Handle safe mode
    if cli.safe {
        println!("{}", "🔒 SAFE MODE ENABLED".bold().color(colors::WARNING));
        println!("   Showing previews only - no files will be modified");
        println!();
    }
    
    // Load or create config WITH CONTEXT
    let mut config = Config::load().context("Failed to load configuration")?;
    
    // Check for reminders
    if !cli.safe && config.is_reminder_due() {
        show_reminder(&config);
    }
    
    // Check for archive reminders
    if !cli.safe {
        let archive_system = ArchiveSystem::new(config.clone())
            .context("Failed to create archive system")?;
        
        if let Ok(old_archives) = archive_system.check_archive_reminders() {
            if !old_archives.is_empty() {
                println!();
                println!("{} {} archive{} need attention", 
                    "📁".yellow(),
                    old_archives.len(),
                    if old_archives.len() == 1 { "" } else { "s" });
            }
        }
    }

    // Create gamification system
    let mut gamification = Gamification::load_from_config(&config);
    
    // Create exam manager
    let mut exam_manager = ExamManager::new(config.clone());
    exam_manager.load_from_config()?;
    
    // Handle command
    match cli.command {
        Commands::Scan(args) => handle_scan(
            &config, 
            &mut exam_manager, 
            &args, 
            cli.safe, 
            cli.verbose,
        )?,
        
        Commands::Suggest(args) => handle_suggest(
            &config, 
            &exam_manager, 
            &args, 
            cli.safe,
        )?,
        
        Commands::Clean(args) => handle_clean(
            &mut config, 
            &exam_manager, 
            &args, 
            cli.safe,
            &mut gamification,
        )?,
        
        Commands::Delete(args) => handle_delete(
            &mut config, 
            &exam_manager, 
            &args, 
            cli.safe, 
            &mut gamification,
        )?,
        
        Commands::Exam(subcommand) => handle_exam(
            &mut config, 
            &mut exam_manager, 
            subcommand, 
            cli.safe,
            &mut gamification,
        )?,
        
        Commands::Protect(subcommand) => handle_protect(&mut config, subcommand)?,
        
        Commands::Archive(subcommand) => handle_archive(&config, subcommand, cli.safe)?,
        
        Commands::Schedule(subcommand) => handle_schedule(&mut config, subcommand)?,
        
        Commands::Stats => handle_stats(&config, &gamification)?,
        
        Commands::Score(args) => handle_score(&config, &args)?,
        
        Commands::Config => config.display(),
        
        Commands::Achievements => handle_achievements(&gamification)?,

        Commands::ShowHelp | Commands::Version => unreachable!(),
    }
    
    Ok(())
}

fn handle_scan(
    config: &Config,
    exam_manager: &mut ExamManager, // Changed to mutable
    args: &cli::ScanArgs,
    safe_mode: bool,
    verbose: bool,
) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path.clone());
    
    let scanner = Scanner::new(config.clone(), exam_manager.is_active());
    let result = scanner.scan(&path, args.days, args.large)
        .context("Failed to scan directory")?;
    
    scanner.print_results(&result, args.detailed);
    
    // AUTO-DETECTION FOR EXAM MODE (from blueprint)
    if !exam_manager.is_active() && config.enable_exam_monitoring {
        // Calculate recent study files (last 7 days)
        let recent_study_files = result.files.iter()
            .filter(|f| f.days_old <= 7)
            .filter(|f| f.confidence > 0.4) // Study files
            .count();
        
        // Calculate existing study files (last 30 days)
        let existing_study_files = result.files.iter()
            .filter(|f| f.days_old <= 30 && f.days_old > 7)
            .filter(|f| f.confidence > 0.4)
            .count();
        
        // Trigger auto-detection if criteria met
        if recent_study_files >= crate::exam::DEFAULT_EXAM_DETECTION_FILES {
            exam_manager.update_tracking(recent_study_files, existing_study_files)
                .context("Failed to update exam tracking")?;
        }
    }
    
    // Track files for exam mode
    for file in &result.files {
        // Only track recent files during exam mode
        if exam_manager.is_active() && file.days_old <= 7 {
            let category = match file.category {
                FileCategory::Lecture => crate::exam::FileCategory::Lecture,
                FileCategory::Assignment => crate::exam::FileCategory::Assignment,
                FileCategory::Reference => crate::exam::FileCategory::Reference,
                _ => crate::exam::FileCategory::Other,
            };
            
            exam_manager.track_file_if_active(
                file.path.clone(),
                file.size_bytes,
                file.file_type.clone(),
                file.course.clone(),
                category,
            );
        }
    }
    
    // Show exam mode status if active
    if exam_manager.is_active() {
        if let Some(tracker) = exam_manager.get_tracker() {
            println!();
            println!("{} Exam mode active: tracking {} files", 
                "🎓".color(colors::HEADER),
                tracker.total_files().to_string().color(colors::SUCCESS)
            );
        }
    }
    
    // Show gamification
    if !safe_mode && !result.files.is_empty() && !verbose {
        println!("{}", "💖".color(colors::HIGH_CONFIDENCE));
        println!("{}", ENCOURAGEMENTS[rand::random::<usize>() % ENCOURAGEMENTS.len()]);
    }
    
    Ok(())
}

fn handle_suggest(
    config: &Config,
    exam_manager: &ExamManager,
    args: &cli::SuggestArgs,
    safe_mode: bool,
) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path.clone());
    
    let scanner = Scanner::new(config.clone(), exam_manager.is_active());
    let result = scanner.scan(&path, DEFAULT_OLD_DAYS, DEFAULT_LARGE_MB)
        .context("Failed to scan directory for suggestions")?;
    
    if result.files.is_empty() {
        println!("{} No suggestions found. Your files look clean! ✨", "✨".green());
        return Ok(());
    }
    
    println!();
    println!("{}", "🎯 CLEANUP SUGGESTIONS".bold().color(colors::HEADER));
    println!("{}", "─".repeat(50).color(colors::PATH));
    println!("{} files found - use numbers with {}", 
        result.files.len().to_string().color(colors::SUCCESS),
        "cleancrush delete".bold()
    );
    println!();
    
    for (i, file) in result.files.iter().enumerate() {
        let confidence_color = if file.confidence > 0.8 {
            colors::HIGH_CONFIDENCE
        } else if file.confidence > 0.6 {
            colors::MEDIUM_CONFIDENCE
        } else {
            colors::LOW_CONFIDENCE
        };
        
        let size_mb = file.size_bytes as f32 / (1024.0 * 1024.0);
        
        println!("{:3}. [{}{:.2}{}] {}",
            i + 1,
            "⚡".color(confidence_color),
            file.confidence,
            "⚡".color(colors::SUCCESS),
            file.path.display().to_string().color(colors::PATH)
        );
        
        println!("     {} ({:.1} MB, {} days old, {})",
            file.reason.dimmed(),
            size_mb,
            file.days_old,
            file.course.color(colors::HEADER)
        );
        
        if file.is_in_cloud {
            println!("     {} In cloud folder", "☁️".yellow());
        }
        if file.is_locked {
            println!("     {} File may be open", "⚠️".yellow());
        }
        if let Some(protected) = config.is_protected(&file.path) {
            println!("     {} Protected folder ({})", 
                "🛡️".blue(),
                match protected.protection_type {
                    ProtectionType::Hard => "hard",
                    ProtectionType::Soft => "soft",
                }
            );
        }
        println!();
    }
    
    // Show quick action options
    println!("{}", "🚀 QUICK ACTIONS".bold().color(colors::HEADER));
    println!("{}", "─".repeat(50).color(colors::PATH));
    println!("{} Delete all suggestions", "• cleancrush clean --mode all".bold());
    println!("{} Delete only duplicates", "• cleancrush clean --mode duplicates".bold());
    println!("{} Delete old files (>{} days)", "• cleancrush clean --mode old --days X".bold(), DEFAULT_OLD_DAYS);
    println!();
    
    // Show gamification
    if !safe_mode {
        println!("{}", "💖".color(colors::HIGH_CONFIDENCE));
        println!("{}", ENCOURAGEMENTS[rand::random::<usize>() % ENCOURAGEMENTS.len()]);
    }
    
    Ok(())
}

fn handle_clean(
    config: &mut Config,
    exam_manager: &ExamManager,
    args: &cli::CleanArgs,
    safe_mode: bool,
    gamification: &mut Gamification,
) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path.clone());
    
    // Create scanner to get file list
    let scanner = Scanner::new(config.clone(), exam_manager.is_active());
    let scan_result = scanner.scan(&path, args.days, DEFAULT_LARGE_MB)
        .context("Failed to scan directory for cleanup")?;
    
    if scan_result.files.is_empty() {
        println!("{} No files to clean", "ℹ️".cyan());
        return Ok(());
    }
    
    // Determine which files to clean based on mode
    let files_to_clean: Vec<PathBuf> = match args.mode {
        cli::CleanMode::All => {
            scan_result.files.iter().map(|f| f.path.clone()).collect()
        }
        cli::CleanMode::Duplicates => {
            scan_result.files_by_category(FileCategory::Duplicate)
                .iter()
                .map(|f| f.path.clone())
                .collect()
        }
        cli::CleanMode::Old => {
            scan_result.files.iter()
                .filter(|f| f.category == FileCategory::Old)
                .map(|f| f.path.clone())
                .collect()
        }
        cli::CleanMode::Large => {
            scan_result.files.iter()
                .filter(|f| f.category == FileCategory::Large)
                .map(|f| f.path.clone())
                .collect()
        }
        cli::CleanMode::Confidence => {
            scan_result.files.iter()
                .filter(|f| f.confidence > 0.8)
                .map(|f| f.path.clone())
                .collect()
        }
        cli::CleanMode::Interactive => {
            // Show interactive selection
            let choices: Vec<String> = scan_result.files.iter()
                .enumerate()
                .map(|(i, f)| format!("{:3}. {} ({:.1} MB, {:.2} confidence)", 
                    i + 1, 
                    f.path.file_name().unwrap_or_default().to_string_lossy(),
                    f.size_bytes as f64 / (1024.0 * 1024.0),
                    f.confidence))
                .collect();
            
            use dialoguer::{theme::ColorfulTheme, MultiSelect};
            let selected = MultiSelect::with_theme(&ColorfulTheme::default())
                .items(&choices)
                .interact()
                .context("Failed to get user selection")?;
            
            selected.iter()
                .map(|&idx| scan_result.files[idx].path.clone())
                .collect()
        }
    };
    
    if files_to_clean.is_empty() {
        println!("{} No files match the criteria for mode {:?}", "ℹ️".cyan(), args.mode);
        return Ok(());
    }
    
    // Confirm if not auto-yes
    if !args.yes && !args.dry_run && !safe_mode {
        println!("{} Found {} files to clean", "📊".cyan(), files_to_clean.len());
        let total_size: u64 = files_to_clean.iter()
            .map(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .sum();
        println!("Total size: {:.1} MB", total_size as f64 / (1024.0 * 1024.0));
        
        use dialoguer::{theme::ColorfulTheme, Confirm};
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Proceed with cleanup?")
            .default(false)
            .interact()
            .context("Failed to get confirmation")?;
        
        if !confirm {
            println!("{} Cleanup cancelled", "ℹ️".cyan());
            return Ok(());
        }
    }
    
    // Create archive system and clean files
    let archive_system = ArchiveSystem::new(config.clone())
        .context("Failed to create archive system")?;
    
    let operation_name = match args.mode {
        cli::CleanMode::All => "all suggestions",
        cli::CleanMode::Duplicates => "duplicates",
        cli::CleanMode::Old => "old files",
        cli::CleanMode::Large => "large files",
        cli::CleanMode::Confidence => "high confidence files",
        cli::CleanMode::Interactive => "selected files",
    };
    
    let cleanup_result = archive_system.clean_files(
        &files_to_clean, 
        args.dry_run, 
        safe_mode,
        operation_name,
    )?;
    
    // Update stats if not in safe/dry mode
    if !safe_mode && !args.dry_run && cleanup_result.files_processed > 0 {
        // Update config stats
        config.total_files_cleaned += cleanup_result.files_processed as u64;
        config.total_space_freed_mb += cleanup_result.total_size_bytes / (1024 * 1024);
        
        // Check for streak
        if cleanup_result.files_processed >= 5 || 
           cleanup_result.total_size_bytes >= 50 * 1024 * 1024 {
            config.streaks += 1;
            
            // Check for achievements
            if cleanup_result.files_processed >= 10 {
                config.add_achievement("🔁 Duplicate Slayer");
            }
            if config.total_space_freed_mb >= 500 {
                config.add_achievement("💾 Space Hero");
            }
            if config.streaks >= 21 {
                config.add_achievement("📆 Consistency Cutie");
            }
        }
        
        config.update_last_cleanup()?;
        
        // Update gamification WITH CleanupType
        let cleanup_type = match args.mode {
            cli::CleanMode::All => CleanupType::Normal,
            cli::CleanMode::Duplicates => CleanupType::Duplicate,
            cli::CleanMode::Old => CleanupType::Normal,
            cli::CleanMode::Large => CleanupType::Normal,
            cli::CleanMode::Confidence => CleanupType::Normal,
            cli::CleanMode::Interactive => CleanupType::Normal,
        };
        
        let unlocks = gamification.update_after_cleanup(
            cleanup_result.files_processed,
            cleanup_result.total_size_bytes,
            cleanup_type,  // USING CleanupType
            exam_manager.is_active(),
        );
        
        // Show encouragement
        gamification.show_encouragement(
            cleanup_result.files_processed,
            cleanup_result.total_size_bytes / (1024 * 1024),
            &unlocks,
        );
    }
    
    Ok(())
}

fn handle_delete(
    config: &mut Config,
    exam_manager: &ExamManager,
    args: &cli::DeleteArgs,
    safe_mode: bool,
    gamification: &mut Gamification,
) -> Result<()> {
    // Get context path
    let context_path = if let Some(path) = &args.path {
        path.clone()
    } else {
        dirs::download_dir().unwrap_or_else(|| PathBuf::from("."))
    };
    
    // If indices provided, we need a previous scan context
    if !args.indices.is_empty() && !args.all && !args.duplicates && args.old.is_none() && args.large.is_none() {
        println!("{} Please specify a path with --path when using indices", "⚠️".yellow());
        println!("Example: cleancrush delete 1 3 5 --path ~/Downloads");
        return Ok(());
    }
    
    // Create scanner
    let scanner = Scanner::new(config.clone(), exam_manager.is_active());
    
    // Determine which files to delete
    let files_to_delete = if !args.indices.is_empty() {
        // Need to scan to get files for indices
        let scan_result = scanner.scan(&context_path, DEFAULT_OLD_DAYS, DEFAULT_LARGE_MB)
            .context("Failed to scan directory")?;
        
        args.indices.iter()
            .filter_map(|&idx| {
                if idx > 0 && idx <= scan_result.files.len() {
                    Some(scan_result.files[idx - 1].path.clone())
                } else {
                    eprintln!("{} Invalid index: {}", "⚠️".yellow(), idx);
                    None
                }
            })
            .collect()
    } else if args.all {
        let scan_result = scanner.scan(&context_path, DEFAULT_OLD_DAYS, DEFAULT_LARGE_MB)
            .context("Failed to scan directory")?;
        scan_result.files.iter().map(|f| f.path.clone()).collect()
    } else if args.duplicates {
        let scan_result = scanner.scan(&context_path, DEFAULT_OLD_DAYS, DEFAULT_LARGE_MB)
            .context("Failed to scan directory")?;
        scan_result.files_by_category(FileCategory::Duplicate)
            .iter()
            .map(|f| f.path.clone())
            .collect()
    } else if let Some(days) = args.old {
        let scan_result = scanner.scan(&context_path, days, DEFAULT_LARGE_MB)
            .context("Failed to scan directory")?;
        scan_result.files.iter()
            .filter(|f| f.category == FileCategory::Old || f.days_old > days as i64)
            .map(|f| f.path.clone())
            .collect()
    } else if let Some(size_mb) = args.large {
        let scan_result = scanner.scan(&context_path, DEFAULT_OLD_DAYS, size_mb)
            .context("Failed to scan directory")?;
        scan_result.files.iter()
            .filter(|f| f.category == FileCategory::Large)
            .map(|f| f.path.clone())
            .collect()
    } else {
        Vec::new()
    };
    
    if files_to_delete.is_empty() {
        println!("{} No files to delete", "ℹ️".cyan());
        return Ok(());
    }
    
    // Create archive system and clean files
    let archive_system = ArchiveSystem::new(config.clone())
        .context("Failed to create archive system")?;
    
    let operation_name = if !args.indices.is_empty() {
        "selected indices"
    } else if args.all {
        "all suggestions"
    } else if args.duplicates {
        "duplicates"
    } else if args.old.is_some() {
        "old files"
    } else if args.large.is_some() {
        "large files"
    } else {
        "files"
    };
    
    let cleanup_result = archive_system.clean_files(
        &files_to_delete, 
        safe_mode, // Use safe mode for dry-run effect
        safe_mode,
        operation_name,
    )?;
    
    // Update stats if not in safe mode
    if !safe_mode && cleanup_result.files_processed > 0 {
        config.update_stats(
            cleanup_result.files_processed,
            cleanup_result.total_size_bytes,
        );
        
        // Check for achievements
        if config.total_files_cleaned >= 10 {
            config.add_achievement("🔁 Duplicate Slayer");
        }
        if config.total_space_freed_mb >= 500 {
            config.add_achievement("💾 Space Hero");
        }
        
        config.update_last_cleanup()?;
        
        // Update gamification
        let is_exam_cleanup = exam_manager.is_active() && (args.all || args.duplicates);
        let unlocks = gamification.update_after_cleanup(
            cleanup_result.files_processed,
            cleanup_result.total_size_bytes,
            CleanupType::Normal,  // USING CleanupType
            is_exam_cleanup,
        );
        
        if is_exam_cleanup {
            config.add_achievement("🎓 Exam Reset");
        }
        
        // Show encouragement
        gamification.show_encouragement(
            cleanup_result.files_processed,
            cleanup_result.total_size_bytes / (1024 * 1024),
            &unlocks,
        );
    }
    
    Ok(())
}

fn handle_exam(
    config: &mut Config,
    exam_manager: &mut ExamManager,
    subcommand: cli::ExamArgs,
    safe_mode: bool,
    gamification: &mut Gamification,
) -> Result<()> {
    if safe_mode {
        println!("{} Exam commands disabled in safe mode", "⚠️".yellow());
        return Ok(());
    }
    
    match subcommand {
        cli::ExamArgs::On { name } => {
            exam_manager.start_manual(name)
                .context("Failed to start exam tracking")?;
        }
        cli::ExamArgs::Off => {
            exam_manager.stop()
                .context("Failed to stop exam tracking")?;
        }
        cli::ExamArgs::Set { start_date, end_date, name } => {
            use chrono::NaiveDate;
    
            let start = NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
                .context("Invalid start date format (use YYYY-MM-DD)")?
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let end = NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
                .context("Invalid end date format (use YYYY-MM-DD)")?
                .and_hms_opt(0, 0, 0)
                .unwrap();
            
            let start_utc = chrono::DateTime::from_naive_utc_and_offset(start, Utc);
            let end_utc = chrono::DateTime::from_naive_utc_and_offset(end, Utc);
    
            exam_manager.set_dates(start_utc, end_utc, name)
                .context("Failed to set exam dates")?;
        }
        cli::ExamArgs::Status => {
            exam_manager.show_status();
        }
        cli::ExamArgs::List => {
            if let Some(tracker) = exam_manager.get_tracker() {
                println!();
                println!("{}", "📚 TRACKED EXAM FILES".bold().color(colors::HEADER));
                println!("{}", "─".repeat(50).color(colors::PATH));
                
                for (i, (path, info)) in tracker.tracked_files.iter().enumerate() {
                    println!("{:3}. {} ({:.1} MB, {})",
                        i + 1,
                        path.display().to_string().color(colors::PATH),
                        info.size_bytes as f64 / (1024.0 * 1024.0),
                        info.course.color(colors::HEADER)
                    );
                }
            } else {
                println!("{} No active exam tracking", "ℹ️".cyan());
            }
        }
        cli::ExamArgs::End => {
            // USING PostExamChoice
            if let Some(choice) = exam_manager.end_exam()? {
                // Log which PostExamChoice was selected
                match &choice {
                    PostExamChoice::QuickClean => println!("{} Quick clean selected", "🚀".green()),
                    PostExamChoice::SelectiveClean => println!("{} Selective clean selected", "🎯".yellow()),
                    PostExamChoice::SmartClean => println!("{} Smart clean selected", "🤖".blue()),
                }
                
                // Get files for cleanup
                if let Some(tracker) = exam_manager.get_tracker() {
                    let files_to_clean = tracker.get_files_for_cleanup(choice.clone());
                    
                    if !files_to_clean.is_empty() {
                        println!();
                        println!("{} Cleaning {} exam files...", 
                            "🧹".color(colors::SUCCESS),
                            files_to_clean.len()
                        );
                        
                        let archive_system = ArchiveSystem::new(config.clone())?;
                        let cleanup_result = archive_system.clean_files(
                            &files_to_clean,
                            false, // Not dry run
                            false, // Not safe mode
                            "post-exam cleanup",
                        )?;
                        
                        // Update stats
                        if cleanup_result.files_processed > 0 {
                            config.update_stats(
                                cleanup_result.files_processed,
                                cleanup_result.total_size_bytes,
                            );
                            
                            config.add_achievement("🎓 Exam Reset");
                            config.streaks += 1;
                            config.update_last_cleanup()?;
                            
                            // Update gamification
                            let unlocks = gamification.update_after_cleanup(
                                cleanup_result.files_processed,
                                cleanup_result.total_size_bytes,
                                CleanupType::Exam,  // USING CleanupType::Exam
                                true,
                            );
                            
                            // Show encouragement
                            gamification.show_encouragement(
                                cleanup_result.files_processed,
                                cleanup_result.total_size_bytes / (1024 * 1024),
                                &unlocks,
                            );
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn handle_protect(
    config: &mut Config,
    subcommand: cli::ProtectArgs,
) -> Result<()> {
    match subcommand {
        cli::ProtectArgs::Add { path, protection } => {
            let abs_path = path.canonicalize()
                .context(format!("Failed to canonicalize path: {}", path.display()))?;
            
            // Check if already protected
            if config.is_protected(&abs_path).is_some() {
                println!("{} Already protected: {}", "ℹ️".cyan(), abs_path.display());
                return Ok(());
            }
            
            let protection_type = match protection {
                cli::ProtectionTypeCli::Hard => ProtectionType::Hard,
                cli::ProtectionTypeCli::Soft => ProtectionType::Soft,
            };
            
            config.protected_folders.push(ProtectedFolder {
                path: abs_path.clone(),
                protection_type,
            });
            
            config.save()
                .context("Failed to save configuration")?;
            println!("{} Protected: {}", "✅".green(), abs_path.display());
        }
        cli::ProtectArgs::Remove { path } => {
            let abs_path = path.canonicalize()
                .context(format!("Failed to canonicalize path: {}", path.display()))?;
            let before_len = config.protected_folders.len();
            
            config.protected_folders.retain(|p| p.path != abs_path);
            
            if config.protected_folders.len() < before_len {
                config.save()
                    .context("Failed to save configuration")?;
                println!("{} Removed protection: {}", "✅".green(), abs_path.display());
            } else {
                println!("{} Not in protected list: {}", "ℹ️".cyan(), abs_path.display());
            }
        }
        cli::ProtectArgs::List => {
            println!("{}", "🛡️ PROTECTED FOLDERS".bold().color(colors::HEADER));
            println!("{}", "─".repeat(50).color(colors::PATH));
            
            if config.protected_folders.is_empty() {
                println!("No protected folders");
            } else {
                for protected in &config.protected_folders {
                    let protection_type = match protected.protection_type {
                        ProtectionType::Hard => "Hard (never scan)",
                        ProtectionType::Soft => "Soft (scan but warn)",
                    };
                    println!("• {} ({})", protected.path.display(), protection_type);
                }
            }
        }
        cli::ProtectArgs::Clear => {
            if !config.protected_folders.is_empty() {
                use dialoguer::{theme::ColorfulTheme, Confirm};
                let confirm = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Clear all protected folders?")
                    .default(false)
                    .interact()
                    .context("Failed to get confirmation")?;
                
                if confirm {
                    config.protected_folders.clear();
                    config.save()
                        .context("Failed to save configuration")?;
                    println!("{} All protected folders cleared", "✅".green());
                }
            } else {
                println!("{} No protected folders to clear", "ℹ️".cyan());
            }
        }
    }
    
    Ok(())
}

fn handle_archive(
    config: &Config,
    subcommand: cli::ArchiveArgs,
    safe_mode: bool,
) -> Result<()> {
    let archive_system = ArchiveSystem::new(config.clone())
        .context("Failed to create archive system")?;
    
    match subcommand {
        cli::ArchiveArgs::List => {
            let archives = archive_system.list_archives()
                .context("Failed to list archives")?;
            
            if archives.is_empty() {
                println!("{} No archives found", "📭".cyan());
                return Ok(());
            }
            
            println!();
            println!("{}", "📁 ARCHIVES".bold().color(colors::HEADER));
            println!("{}", "─".repeat(50).color(colors::PATH));
            
            for (path, date) in archives {
                let days_old = (Utc::now() - date).num_days();
                let size_mb = archive_system.dir_size(&path)
                    .context(format!("Failed to get size of archive: {}", path.display()))? as f64 / (1024.0 * 1024.0);
                
                let age_color = if days_old > 30 {
                    colors::WARNING
                } else {
                    colors::SUCCESS
                };
                
                println!("• {} ({:.1} MB, {} days old)",
                    path.display().to_string().color(colors::PATH),
                    size_mb,
                    days_old.to_string().color(age_color)
                );
            }
        }
        cli::ArchiveArgs::Clean { days, yes } => {
            if safe_mode {
                println!("{} Archive cleaning disabled in safe mode", "⚠️".yellow());
                return Ok(());
            }
            
            archive_system.clean_old_archives(days, yes)?;
        }
        cli::ArchiveArgs::Stats => {
            archive_system.show_stats()?;
        }
        cli::ArchiveArgs::Restore { .. } => {
            println!("{} Archive restore not yet implemented", "⚠️".yellow());
            println!("Coming in a future update!");
        }
    }
    
    Ok(())
}

fn handle_schedule(
    config: &mut Config,
    subcommand: cli::ScheduleArgs,
) -> Result<()> {
    match subcommand {
        cli::ScheduleArgs::Set { schedule } => {
            let schedule_type = match schedule {
                cli::ScheduleType::Never => ReminderSchedule::Never,
                cli::ScheduleType::Weekly => ReminderSchedule::Weekly,
                cli::ScheduleType::Monthly => ReminderSchedule::Monthly,
            };
            
            config.reminder_schedule = schedule_type.clone();
            config.save()
                .context("Failed to save configuration")?;
            
            match schedule_type {
                ReminderSchedule::Never => println!("{} Reminders disabled", "✅".green()),
                ReminderSchedule::Weekly => println!("{} Weekly reminders enabled (Sundays)", "✅".green()),
                ReminderSchedule::Monthly => println!("{} Monthly reminders enabled (1st of month)", "✅".green()),
            }
        }
        cli::ScheduleArgs::Show => {
            let schedule = match config.reminder_schedule {
                ReminderSchedule::Never => "Never",
                ReminderSchedule::Weekly => "Weekly (Sundays)",
                ReminderSchedule::Monthly => "Monthly (1st of month)",
            };
            
            println!("{} Reminder schedule: {}", "⏰".cyan(), schedule);
            
            if let Some(last) = &config.last_cleanup {
                let last_date: chrono::DateTime<Utc> = last.parse()
                    .context("Failed to parse last cleanup date")?;
                let days_ago = (Utc::now() - last_date).num_days();
                println!("{} Last cleanup: {} ({} days ago)", 
                    "📅".cyan(),
                    last_date.format("%Y-%m-%d"),
                    days_ago
                );
            }
        }
        cli::ScheduleArgs::Run => {
            println!("{} Running scheduled cleanup...", "🧹".cyan());
            // This would trigger a cleanup based on schedule
            // For now, just remind
            show_reminder(config);
        }
    }
    
    Ok(())
}

fn handle_stats(
    config: &Config,
    gamification: &Gamification,
) -> Result<()> {
    println!();
    println!("{}", "📊 CLEANCRUSH STATISTICS".bold().color(colors::HEADER));
    println!("{}", "─".repeat(50).color(colors::PATH));
    
    println!("🎯 Files cleaned: {}", 
        config.total_files_cleaned.to_string().color(colors::SUCCESS));
    println!("💾 Space freed: {:.1} MB", 
        config.total_space_freed_mb.to_string().color(colors::SUCCESS));
    println!("🔥 Current streak: {} days", 
        config.streaks.to_string().color(colors::WARNING));
    
    if let Some(last) = &config.last_cleanup {
        let last_date: chrono::DateTime<Utc> = last.parse()
            .context("Failed to parse last cleanup date")?;
        let days_ago = (Utc::now() - last_date).num_days();
        println!("📅 Last cleanup: {} days ago", 
            days_ago.to_string().color(if days_ago > 7 { colors::WARNING } else { colors::SUCCESS }));
    }
    
    // Show exam status
    if let Some(tracking) = &config.exam_tracking {
        if tracking.active {
            println!("🎓 Exam mode: Active ({} files tracked)", 
                tracking.tracked_files.len().to_string().color(colors::SUCCESS));
        }
    }
    
    println!();
    gamification.display_stats();
    
    Ok(())
}

fn handle_score(
    config: &Config,
    args: &cli::ScoreArgs,
) -> Result<()> {
    let path = args.path.canonicalize()
        .context(format!("Failed to canonicalize path: {}", args.path.display()))?;
    
    let scanner = Scanner::new(config.clone(), false);
    let result = scanner.scan(&path, DEFAULT_OLD_DAYS, DEFAULT_LARGE_MB)
        .context("Failed to scan directory for scoring")?;
    
    // Calculate cleanliness score USING the gamification method
    let gamification = Gamification::load_from_config(config);
    
    let mut duplicate_count = 0;
    let mut old_count = 0;
    let mut large_count = 0;
    let mut very_large_count = 0;
    
    for file in &result.files {
        match file.category {
            FileCategory::Duplicate => duplicate_count += 1,
            FileCategory::Old => old_count += 1,
            FileCategory::Large => {
                if file.size_bytes > 500 * 1024 * 1024 {
                    very_large_count += 1;
                } else {
                    large_count += 1;
                }
            }
            _ => {}
        }
    }

// USE the calculate_cleanliness_score method
let (score, breakdown) = gamification.calculate_cleanliness_score(
        duplicate_count,
        old_count,
        large_count,
        very_large_count,
    );
    
    println!();
    println!("{}", "🏆 CLEANLINESS SCORE".bold().color(colors::HEADER));
    println!("{}", "─".repeat(50).color(colors::PATH));
    
    // Show score with emoji
    let score_emoji = if score >= 90 {
        "✨".to_string()
    } else if score >= 70 {
        "🌟".to_string()
    } else if score >= 50 {
        "💫".to_string()
    } else {
        "🌱".to_string()
    };
    
    let score_color = if score >= 80 {
        colors::SUCCESS
    } else if score >= 60 {
        colors::WARNING
    } else {
        colors::HIGH_CONFIDENCE
    };
    
    println!("{} {}/100 {}", 
        score_emoji,
        score.to_string().color(score_color),
        match score {
            90..=100 => "Excellent!".to_string(),
            70..=89 => "Good job!".to_string(),
            50..=69 => "Room for improvement".to_string(),
            _ => "Time for cleanup!".to_string(),
        }.color(score_color)
    );
    
    // Show breakdown from the gamification method
if !breakdown.is_empty() && breakdown != "Perfect! No issues found ✨" {
        println!();
        println!("{} Breakdown:", "📊".cyan());
        println!("{}", breakdown);
    } else if breakdown == "Perfect! No issues found ✨" {
        println!();
        println!("{} Perfect! No issues found ✨", "🎉".green());
    }
    
    // Show suggestions
    println!();
    println!("{} To improve your score:", "💡".cyan());
    
    if duplicate_count > 0 {
        println!("   • Run {} to remove duplicates", 
            "cleancrush clean --mode duplicates".bold());
    }
    
    if old_count > 0 {
        println!("   • Run {} to clean old files", 
            format!("cleancrush clean --mode old --days {}", DEFAULT_OLD_DAYS).bold());
    }
    
    if large_count > 0 || very_large_count > 0 {
        println!("   • Review large files with {}", 
            "cleancrush suggest".bold());
    }
    
    Ok(())
}

fn handle_achievements(gamification: &Gamification) -> Result<()> {
    println!();
    println!("{}", "🏆 ACHIEVEMENTS".bold().color(colors::HEADER));
    println!("{}", "─".repeat(50).color(colors::PATH));
    
    gamification.display_achievements();
    
    Ok(())
}

fn show_reminder(config: &Config) {
    println!();
    println!("{}", "💡 CLEANUP REMINDER".bold().color(colors::HEADER));
    println!("{}", "─".repeat(50).color(colors::PATH));
    
    let days_since = match &config.last_cleanup {
        None => {
            println!("You haven't cleaned your files yet!");
            println!("Start with: {}", "cleancrush scan ~/Downloads".bold());
            return;
        }
        Some(last) => {
            let last_date: chrono::DateTime<Utc> = last.parse().unwrap_or(Utc::now());
            (Utc::now() - last_date).num_days()
        }
    };
    
    println!("It's been {} days since your last cleanup.", 
        days_since.to_string().color(colors::WARNING));
    
    use dialoguer::{theme::ColorfulTheme, Confirm};
    let want_scan = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Want to scan your Downloads folder?")
        .default(true)
        .interact()
        .unwrap_or(false);
    
    if want_scan {
        println!("{} Run: {}", "💡".cyan(), "cleancrush scan ~/Downloads".bold());
    }
    
    println!();
}