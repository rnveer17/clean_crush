use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use chrono::{DateTime, Utc, Duration, TimeZone, NaiveDate};
use serde::{Deserialize, Serialize};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use anyhow::{Result, Context};
use crate::colors;
use crate::config::{Config, CleanupAction, ProtectedFolder, ProtectionType};

const COURSE_PATTERNS: &[(&str, &[&str])] = &[
    ("cs", &["cs", "computer", "programming", "algorithm", "software"]),
    ("math", &["math", "calculus", "algebra", "statistics", "geometry"]),
    ("science", &["physics", "chemistry", "biology", "science", "lab"]),
    ("engineering", &["engineer", "mechanical", "electrical", "civil", "robotics"]),
    ("business", &["business", "management", "finance", "economics", "marketing"]),
    ("humanities", &["history", "literature", "philosophy", "art", "psychology"]),
];

const CLOUD_FOLDERS: &[&str] = &[
    "onedrive",
    "dropbox",
    "google drive",
    "icloud drive",
    "box",
];


#[derive(Debug, Clone)]
pub struct ArchiveSystem {
    archive_path: PathBuf,
    config: Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveInfo {
    pub archive_date: DateTime<Utc>,
    pub total_files: usize,
    pub total_size_bytes: u64,
    pub files: Vec<ArchivedFileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivedFileInfo {
    pub original_path: PathBuf,
    pub archived_path: PathBuf,
    pub course: String,
    pub file_type: String,
    pub size_bytes: u64,
    pub archived_date: DateTime<Utc>,
    pub original_modified: DateTime<Utc>,
}

impl ArchiveSystem {
    /// Create new archive system
    pub fn new(config: Config) -> Result<Self> {
        let archive_path = match &config.default_action {
            CleanupAction::Archive => {
                let home = dirs::home_dir()
                    .context("Could not find home directory")?;
                let archive = home.join("CleanCrush-Archive");
                fs::create_dir_all(&archive)?;
                archive
            }
            CleanupAction::RecycleBin => {
                // Still create archive path for tracking, but won't be used for actual archiving
                let home = dirs::home_dir()
                    .context("Could not find home directory")?;
                home.join("CleanCrush-Temp")
            }
        };
        
        Ok(Self {
            archive_path,
            config,
        })
    }
    
    /// Clean files (either to Recycle Bin or Archive based on config)
    pub fn clean_files(
        &self, 
        files: &[PathBuf], 
        dry_run: bool, 
        safe_mode: bool,
        operation_name: &str,
    ) -> Result<CleanupResult> {
        if files.is_empty() {
            println!("{} No files to clean", "ℹ️".cyan());
            return Ok(CleanupResult::empty());
        }
        
        println!();
        println!("{} {}", "🧹 CLEANING FILES".bold().color(colors::HEADER), operation_name.dimmed());
        println!("{}", "─".repeat(50).color(colors::PATH));
        
        if safe_mode {
            println!("{} SAFE MODE: Showing preview only", "🔒".yellow());
            println!("   No files will be modified");
            return self.preview_cleanup(files);
        }
        
        if dry_run {
            println!("{} DRY RUN: Showing what would be done", "🌵".yellow());
            println!("   No files will be modified");
            return self.preview_cleanup(files);
        }
        
        match &self.config.default_action {
            CleanupAction::RecycleBin => self.clean_to_recycle_bin(files),
            CleanupAction::Archive => self.clean_to_archive(files),
        }
    }
    
    /// Preview cleanup without actually doing anything
    fn preview_cleanup(&self, files: &[PathBuf]) -> Result<CleanupResult> {
        let mut result = CleanupResult::empty();
        let mut total_size = 0;
        
        for (i, file) in files.iter().enumerate() {
            if !file.exists() {
                continue;
            }
            
            let size = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
            total_size += size;
            
            println!("{:3}. {} ({:.1} MB)",
                i + 1,
                file.display().to_string().color(colors::PATH),
                size as f64 / (1024.0 * 1024.0)
            );
            
            // Check for special conditions
            if self.is_in_cloud_folder(file) {
                println!("     {} In cloud folder", "☁️".yellow());
            }
            
            if self.is_file_locked(file) {
                println!("     {} File may be open", "⚠️".yellow());
            }
            
            if let Some(protected) = self.config.is_protected(file) {
                println!("     {} Protected folder ({})", 
                    "🛡️".blue(),
                    match protected.protection_type {
                        ProtectionType::Hard => "hard",
                        ProtectionType::Soft => "soft",
                    }
                );
            }
            
            result.files_processed += 1;
        }
        
        result.total_size_bytes = total_size;
        
        println!();
        println!("{} Would process {} files ({:.1} MB)", 
            "📊".cyan(),
            result.files_processed,
            total_size as f64 / (1024.0 * 1024.0)
        );
        
        match &self.config.default_action {
            CleanupAction::RecycleBin => {
                println!("{} Files would go to Recycle Bin", "🗑️".green());
            }
            CleanupAction::Archive => {
                println!("{} Files would be archived to: {}", "📁".green(), self.archive_path.display());
            }
        }
        
        Ok(result)
    }
    
    /// Clean files to Recycle Bin
    fn clean_to_recycle_bin(&self, files: &[PathBuf]) -> Result<CleanupResult> {
        let mut result = CleanupResult::empty();
        let mut cloud_warnings = Vec::new();
        let mut locked_files = Vec::new();
        let mut protected_files = Vec::new();
        
        let pb = ProgressBar::new(files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files {msg}")?
                .progress_chars("#>-")
        );
        
        for file in files {
            pb.inc(1);
            
            if !file.exists() {
                pb.set_message("Skipped (not found)");
                continue;
            }
            
            // Check for special conditions
            if self.is_in_cloud_folder(file) {
                cloud_warnings.push(file.display().to_string());
                if !self.confirm_cloud_deletion(file)? {
                    pb.set_message("Skipped (cloud)");
                    continue;
                }
            }
            
            if self.is_file_locked(file) {
                locked_files.push(file.display().to_string());
                if !self.handle_locked_file(file)? {
                    pb.set_message("Skipped (locked)");
                    continue;
                }
            }
            
            if let Some(protected) = self.config.is_protected(file) {
                protected_files.push((file.display().to_string(), protected.protection_type.clone()));
                if !self.confirm_protected_deletion(file, protected)? {
                    pb.set_message("Skipped (protected)");
                    continue;
                }
            }
            
            // Get file size before deletion
            let size = fs::metadata(file).map(|m| m.len()).unwrap_or(0);
            
            // Send to Recycle Bin
            match trash::delete(file) {
                Ok(_) => {
                    result.files_processed += 1;
                    result.total_size_bytes += size;
                    result.successful_files.push(file.clone());
                    pb.set_message("Deleted");
                }
                Err(e) => {
                    result.failed_files.push((file.clone(), e.to_string()));
                    pb.set_message("Failed");
                }
            }
        }
        
        pb.finish_and_clear();
        
        // Print summary
        self.print_cleanup_summary(&result, &cloud_warnings, &locked_files, &protected_files);
        
        Ok(result)
    }
    
    /// Clean files to Archive
    fn clean_to_archive(&self, files: &[PathBuf]) -> Result<CleanupResult> {
        let archive_date = Utc::now();
        let date_folder = archive_date.format("%Y-%m-%d").to_string();
        let archive_dir = self.archive_path.join(&date_folder);
        
        fs::create_dir_all(&archive_dir)?;
        
        let mut result = CleanupResult::empty();
        let mut archive_info = ArchiveInfo {
            archive_date,
            total_files: 0,
            total_size_bytes: 0,
            files: Vec::new(),
        };
        
        let pb = ProgressBar::new(files.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files {msg}")?
                .progress_chars("#>-")
        );
        
        for file in files {
            pb.inc(1);
            
            if !file.exists() {
                pb.set_message("Skipped (not found)");
                continue;
            }
            
            // Check for locked files
            if self.is_file_locked(file) {
                if !self.handle_locked_file(file)? {
                    pb.set_message("Skipped (locked)");
                    continue;
                }
            }
            
            // Get file info
            let metadata = match fs::metadata(file) {
                Ok(m) => m,
                Err(_) => {
                    result.failed_files.push((file.clone(), "Cannot read metadata".to_string()));
                    pb.set_message("Failed");
                    continue;
                }
            };
            
            let size = metadata.len();
            let modified: DateTime<Utc> = metadata.modified()
                .unwrap_or_else(|_| SystemTime::now())
                .into();
            
            // Determine course
            let course = self.detect_course(file);
            let course_dir = archive_dir.join(&course);
            fs::create_dir_all(&course_dir)?;
            
            // Generate unique filename
            let filename = file.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            
            let mut dest_path = course_dir.join(&filename);
            let mut counter = 1;
            
            while dest_path.exists() {
                let stem = file.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy();
                let extension = file.extension()
                    .unwrap_or_default()
                    .to_string_lossy();
                
                let new_filename = if extension.is_empty() {
                    format!("{}_{}", stem, counter)
                } else {
                    format!("{}_{}.{}", stem, counter, extension)
                };
                
                dest_path = course_dir.join(new_filename);
                counter += 1;
                
                if counter > 100 {
                    result.failed_files.push((file.clone(), "Too many filename conflicts".to_string()));
                    pb.set_message("Failed");
                    continue;
                }
            }
            
            // Move file to archive
            match fs::rename(file, &dest_path) {
                Ok(_) => {
                    // Create archive info entry
                    let archived_info = ArchivedFileInfo {
                        original_path: file.clone(),
                        archived_path: dest_path.clone(),
                        course: course.clone(),
                        file_type: file.extension()
                            .and_then(|ext| ext.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        size_bytes: size,
                        archived_date: Utc::now(),
                        original_modified: modified,
                    };
                    
                    archive_info.files.push(archived_info);
                    archive_info.total_files += 1;
                    archive_info.total_size_bytes += size;
                    
                    result.files_processed += 1;
                    result.total_size_bytes += size;
                    result.successful_files.push(file.clone());
                    pb.set_message("Archived");
                }
                Err(e) => {
                    result.failed_files.push((file.clone(), e.to_string()));
                    pb.set_message("Failed");
                }
            }
        }
        
        pb.finish_and_clear();
        
        // Save archive info
        if !archive_info.files.is_empty() {
            let info_path = archive_dir.join("archive_info.json");
            let info_data = serde_json::to_string_pretty(&archive_info)?;
            fs::write(info_path, info_data)?;
        }
        
        // Print summary
        println!();
        println!("{} {} files archived to {}", 
            "✅".green(),
            result.files_processed,
            archive_dir.display().to_string().color(colors::PATH)
        );
        println!("💾 Freed {:.1} MB", result.total_size_bytes as f64 / (1024.0 * 1024.0));
        
        if !result.failed_files.is_empty() {
            println!("{} {} files failed:", "⚠️".yellow(), result.failed_files.len());
            for (file, error) in &result.failed_files {
                println!("   • {}: {}", file.display(), error);
            }
        }
        
        // Create reminder for 30 days from now
        self.schedule_archive_reminder(&archive_dir)?;
        
        Ok(result)
    }
    
    /// Check if file is in cloud folder
    fn is_in_cloud_folder(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        CLOUD_FOLDERS.iter().any(|folder: &&str| path_str.contains(&folder.to_lowercase()))
    }
    
    /// Check if file is locked
    fn is_file_locked(&self, path: &Path) -> bool {
        match fs::OpenOptions::new().read(true).write(true).open(path) {
            Ok(_) => false,
            Err(_) => true,
        }
    }
    
    /// Confirm deletion from cloud folder
    fn confirm_cloud_deletion(&self, file: &Path) -> Result<bool> {
        println!();
        println!("{} {} is in cloud folder!", "☁️".yellow(), file.display());
        println!("   Deleting will remove from cloud too!");
        
        let choices = &["Skip (keep in cloud)", "Delete anyway", "Cancel all"];
        
        // Add dialoguer import at the top of the file
        use dialoguer::{theme::ColorfulTheme, Select};
        let selection = Select::with_theme(&ColorfulTheme::default())
            .items(choices)
            .default(0)
            .interact()?;
        
        match selection {
            0 => Ok(false), // Skip
            1 => Ok(true),  // Delete anyway
            2 => Err(anyhow::anyhow!("Operation cancelled by user")),
            _ => unreachable!(),
        }
    }
    
    /// Handle locked file
    fn handle_locked_file(&self, file: &Path) -> Result<bool> {
        println!();
        println!("{} {} is open in another program", "⚠️".yellow(), file.display());
        
        let choices = &["Skip this file", "Retry in 10 seconds", "Cancel all"];
        
        use dialoguer::{theme::ColorfulTheme, Select};
        let selection = Select::with_theme(&ColorfulTheme::default())
            .items(choices)
            .default(0)
            .interact()?;
        
        match selection {
            0 => Ok(false), // Skip
            1 => {
                // Wait and retry
                println!("   Waiting 10 seconds...");
                std::thread::sleep(std::time::Duration::from_secs(10));
                
                if self.is_file_locked(file) {
                    println!("   File still locked, skipping");
                    Ok(false)
                } else {
                    println!("   File unlocked, continuing");
                    Ok(true)
                }
            }
            2 => Err(anyhow::anyhow!("Operation cancelled by user")),
            _ => unreachable!(),
        }
    }
    
    /// Confirm deletion from protected folder
    fn confirm_protected_deletion(&self, file: &Path, protected: &ProtectedFolder) -> Result<bool> {
        println!();
        println!("{} {} is in protected folder", "🛡️".blue(), file.display());
        
        match protected.protection_type {
            ProtectionType::Hard => {
                println!("   This folder should never be scanned!");
                return Ok(false);
            }
            ProtectionType::Soft => {
                use dialoguer::{theme::ColorfulTheme, Confirm};
                let confirm = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Proceed with deletion from protected folder?")
                    .default(false)
                    .interact()?;
                
                Ok(confirm)
            }
        }
    }
    
    /// Detect course from filename
    fn detect_course(&self, path: &Path) -> String {
        let filename = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        
        for (course, patterns) in COURSE_PATTERNS {
            for pattern in *patterns {
                if filename.contains(pattern) {
                    let course_str: &str = *course;
                    return course_str.to_string();
                }
            }
        }
        
        "general".to_string()
    }
    
    /// Print cleanup summary
    fn print_cleanup_summary(
        &self, 
        result: &CleanupResult,
        cloud_warnings: &[String],
        locked_files: &[String],
        protected_files: &[(String, ProtectionType)],
    ) {
        println!();
        println!("{}", "🧹 CLEANUP COMPLETE".bold().color(colors::HEADER));
        println!("{}", "─".repeat(50).color(colors::PATH));
        
        println!("✅ Processed {} files", result.files_processed);
        println!("💾 Freed {:.1} MB", result.total_size_bytes as f64 / (1024.0 * 1024.0));
        
        if !result.failed_files.is_empty() {
            println!();
            println!("{} {} files failed:", "⚠️".yellow(), result.failed_files.len());
            for (file, error) in &result.failed_files {
                println!("   • {}: {}", file.display(), error);
            }
        }
        
        if !cloud_warnings.is_empty() {
            println!();
            println!("{} {} files from cloud folders:", "☁️".yellow(), cloud_warnings.len());
            for file in cloud_warnings {
                println!("   • {}", file);
            }
        }
        
        if !locked_files.is_empty() {
            println!();
            println!("{} {} locked files:", "🔒".yellow(), locked_files.len());
            for file in locked_files {
                println!("   • {}", file);
            }
        }
        
        if !protected_files.is_empty() {
            println!();
            println!("{} {} files from protected folders:", "🛡️".blue(), protected_files.len());
            for (file, protection_type) in protected_files {
                let protection_str = match protection_type {
                    ProtectionType::Hard => "hard protected",
                    ProtectionType::Soft => "soft protected",
                };
                println!("   • {} ({})", file, protection_str);
            }
        }
        
        match &self.config.default_action {
            CleanupAction::RecycleBin => {
                println!();
                println!("{} Files moved to Recycle Bin", "🗑️".green());
                println!("   You have 30 days to restore them if needed");
            }
            CleanupAction::Archive => {
                println!();
                println!("{} Files archived to: {}", "📁".green(), self.archive_path.display());
                println!("   You will be reminded to clean old archives after 30 days");
            }
        }
    }
    
    /// Schedule archive reminder for 30 days later
    fn schedule_archive_reminder(&self, archive_dir: &Path) -> Result<()> {
        let reminder_file = archive_dir.join(".reminder_date");
        let reminder_date = Utc::now() + Duration::days(30);
        
        fs::write(reminder_file, reminder_date.to_rfc3339())?;
        Ok(())
    }
    
    /// Check archive reminders
pub fn check_archive_reminders(&self) -> Result<Vec<PathBuf>> {
    let archives = self.list_archives()?;
    let mut old_archives = Vec::new();
    let now = Utc::now();
    
    for (archive_path, archive_date) in archives {
        let days_old = (now - archive_date).num_days();
        
        // 30-day reminder
        if days_old >= 30 {
            old_archives.push(archive_path.clone());
            
            println!();
            println!("{} ARCHIVE REMINDER", "⏰".bold().color(colors::WARNING));
            println!("{}", "─".repeat(50).color(colors::PATH));
            println!("Archive from {} is {} days old.", 
                archive_date.format("%b %d, %Y").to_string().color(colors::SUCCESS),
                days_old.to_string().color(colors::WARNING));
            
            let archive_size = self.dir_size(&archive_path)?;
            let size_mb = archive_size as f64 / (1024.0 * 1024.0);
            println!("Size: {:.1} MB", size_mb);
            
            // Show options
            println!();
            println!("Options:");
            println!("  1. Clean (delete archive)");
            println!("  2. Snooze (remind again in 7 days)");
            println!("  3. Keep forever");
            
            use dialoguer::{theme::ColorfulTheme, Select};
            let choice = Select::with_theme(&ColorfulTheme::default())
                .items(&["Clean", "Snooze 7 days", "Keep forever"])
                .default(0)
                .interact()?;
            
            match choice {
                0 => {
                    // Clean archive
                    println!("Cleaning archive: {}", archive_path.display());
                    if let Err(e) = fs::remove_dir_all(&archive_path) {
                        println!("{} Failed to clean: {}", "⚠️".yellow(), e);
                    } else {
                        println!("{} Archive cleaned", "✅".green());
                    }
                }
                1 => {
                    println!("{} Will remind again in 7 days", "⏰".cyan());
                    // Implement snooze by updating reminder file
                    let snooze_date = Utc::now() + Duration::days(7);
                    let reminder_file = archive_path.join(".reminder_date");
                    fs::write(reminder_file, snooze_date.to_rfc3339())?;
                }
                2 => {
                    println!("{} Archive marked to keep forever", "💾".green());
                    // Create a .keep_forever file
                    let keep_file = archive_path.join(".keep_forever");
                    fs::write(keep_file, "Keep forever - user choice")?;
                }
                _ => unreachable!(),
            }
        }
    }
    
    Ok(old_archives)
}
    
    /// Clean old archives with confirmation
    pub fn clean_old_archives(&self, older_than_days: i64, skip_confirmation: bool) -> Result<CleanupResult> {
        let mut result = CleanupResult::empty();
        let cutoff_date = Utc::now() - Duration::days(older_than_days);
        
        if !self.archive_path.exists() {
            println!("{} No archive directory found", "ℹ️".cyan());
            return Ok(result);
        }
        
        let archives = self.list_archives()?;
        let old_archives: Vec<_> = archives.into_iter()
            .filter(|(_, date)| *date < cutoff_date)
            .collect();
        
        if old_archives.is_empty() {
            println!("{} No archives older than {} days", "✨".green(), older_than_days);
            return Ok(result);
        }
        
        println!();
        println!("{} Found {} old archives:", "📅".cyan(), old_archives.len());
        for (path, date) in &old_archives {
            let duration = Utc::now() - *date;
            let days_old = duration.num_days();
            println!("   • {} ({} days old)", path.display(), days_old);
        }
        
        let mut should_clean = skip_confirmation;
        if !skip_confirmation {
            use dialoguer::{theme::ColorfulTheme, Confirm};
            should_clean = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Clean these old archives?")
                .default(false)
                .interact()?;
        }
        
        if !should_clean {
            println!("{} Archive cleaning cancelled", "ℹ️".cyan());
            return Ok(result);
        }
        
        for (archive_path, _) in old_archives {
            match fs::remove_dir_all(&archive_path) {
                Ok(_) => {
                    result.files_processed += 1;
                    let path_clone = archive_path.clone();
                    result.successful_files.push(path_clone);
                    println!("{} Cleaned: {}", "✅".green(), archive_path.display());
                }
                Err(e) => {
                    let path_clone = archive_path.clone();
                    result.failed_files.push((path_clone, e.to_string()));
                    println!("{} Failed to clean: {} - {}", "❌".red(), archive_path.display(), e);
                }
            }
        }
        
        Ok(result)
    }
    
    /// List all archives with their dates
    pub fn list_archives(&self) -> Result<Vec<(PathBuf, DateTime<Utc>)>> {
        let mut archives = Vec::new();
        
        if !self.archive_path.exists() {
            return Ok(archives);
        }
        
        for entry in fs::read_dir(&self.archive_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if !path.is_dir() {
                continue;
            }
            
            // Try to parse date from folder name (YYYY-MM-DD format)
            if let Some(folder_name) = path.file_name() {
                if let Ok(date) = folder_name.to_string_lossy().parse::<NaiveDate>() {
                    let datetime = date.and_hms_opt(0, 0, 0).unwrap();
                    // Fix deprecated from_utc
                    let utc_date = Utc.from_utc_datetime(&datetime);
                    archives.push((path, utc_date));
                }
            }
        }
        
        // Sort by date (oldest first)
        archives.sort_by_key(|(_, date)| *date);
        
        Ok(archives)
    }
    
    /// Show archive statistics
    pub fn show_stats(&self) -> Result<()> {
        let archives = self.list_archives()?;
        
        if archives.is_empty() {
            println!("{} No archives found", "📭".cyan());
            return Ok(());
        }
        
        println!();
        println!("{}", "📁 ARCHIVE STATISTICS".bold().color(colors::HEADER));
        println!("{}", "─".repeat(50).color(colors::PATH));
        
        let total_archives = archives.len();
        let now = Utc::now();
        let oldest = archives.first().map(|(_, date)| date).unwrap_or(&now);
        let newest = archives.last().map(|(_, date)| date).unwrap_or(&now);
        
        println!("📊 Total archives: {}", total_archives.to_string().color(colors::SUCCESS));
        println!("📅 Oldest: {}", oldest.format("%Y-%m-%d").to_string().color(colors::PATH));
        println!("📅 Newest: {}", newest.format("%Y-%m-%d").to_string().color(colors::PATH));
        
        // Calculate total size
        let mut total_size = 0u64;
        for (path, _) in &archives {
            total_size += self.dir_size(path)?;
        }
        
        println!("💾 Total size: {:.1} MB", total_size as f64 / (1024.0 * 1024.0));
        
        // Show archives that need cleaning (older than 30 days)
        let cutoff_date = Utc::now() - Duration::days(30);
        let old_archives: Vec<_> = archives.into_iter()
            .filter(|(_, date)| *date < cutoff_date)
            .collect();
        
        if !old_archives.is_empty() {
            println!();
            println!("{} {} archives older than 30 days:", "📅".yellow(), old_archives.len());
            for (path, date) in old_archives.iter().take(5) {
                let duration = Utc::now() - *date;
                let days_old = duration.num_days();
                let size_mb = self.dir_size(path)? as f64 / (1024.0 * 1024.0);
                println!("   • {} ({} days old, {:.1} MB)", 
                    path.display(), 
                    days_old,
                    size_mb);
            }
            
            if old_archives.len() > 5 {
                println!("   ... and {} more", old_archives.len() - 5);
            }
            
            println!();
            println!("{} Run {} to clean old archives", 
                "💡".cyan(),
                "cleancrush archive clean".bold());
        }
        
        Ok(())
    }
    
    /// Calculate directory size recursively
    pub fn dir_size(&self, path: &Path) -> Result<u64> {
        let mut total = 0u64;
        
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    total += self.dir_size(&path)?;
                } else {
                    total += fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                }
            }
        }
        
        Ok(total)
    }
}

#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub files_processed: usize,
    pub total_size_bytes: u64,
    pub successful_files: Vec<PathBuf>,
    pub failed_files: Vec<(PathBuf, String)>,
}

impl CleanupResult {
    fn empty() -> Self {
        Self {
            files_processed: 0,
            total_size_bytes: 0,
            successful_files: Vec::new(),
            failed_files: Vec::new(),
        }
    }
}