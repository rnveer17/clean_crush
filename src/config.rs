use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use dirs;
use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use dialoguer::{theme::ColorfulTheme, Select, MultiSelect, Confirm, Input};
use colored::*;
use crate::colors;

const SYSTEM_PATHS: &[&str] = &[
    r"C:\Windows", r"C:\Program Files", r"C:\ProgramData",
    r"C:\System Volume Information", "/System", "/usr",
    "/bin", "/sbin", "/etc", "/var", "/lib",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // User preferences
    pub default_action: CleanupAction,
    pub protected_folders: Vec<ProtectedFolder>,
    pub reminder_schedule: ReminderSchedule,
    pub enable_exam_monitoring: bool,
    
    // State tracking
    pub last_cleanup: Option<String>,
    pub last_reminder: Option<String>,
    pub exam_tracking: Option<ExamTrackingState>,
    
    // Gamification
    pub streaks: u32,
    pub achievements: Vec<String>,
    pub total_files_cleaned: u64,
    pub total_space_freed_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupAction {
    RecycleBin,
    Archive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedFolder {
    pub path: PathBuf,
    pub protection_type: ProtectionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtectionType {
    Hard,  // Never scan
    Soft,  // Scan but warn before actions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReminderSchedule {
    Never,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamTrackingState {
    pub active: bool,
    pub start_date: String,
    pub end_date: Option<String>,
    pub tracked_files: Vec<PathBuf>,
    pub exam_period_name: Option<String>,
}

impl Config {
    /// Get the path to the config file
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .context("Could not find home directory")?;
        Ok(home.join(".cleancrush.json"))
    }
    
    /// Get the path to the config backup file
    pub fn backup_path() -> Result<PathBuf> {
        let config_path = Self::config_path()?;
        Ok(config_path.with_extension("json.backup"))
    }
    
    /// Load config from disk, or create default if doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if config_path.exists() {
            // Try to load existing config
            let data = fs::read_to_string(&config_path)
                .context("Failed to read config file")?;
            
            match serde_json::from_str(&data) {
                Ok(config) => Ok(config),
                Err(e) => {
                    // Config is corrupted, try backup
                    eprintln!("{} Config corrupted, trying backup...", "⚠️".yellow());
                    if let Ok(backup) = Self::load_backup() {
                        eprintln!("{} Restored from backup", "✅".green());
                        return Ok(backup);
                    }
                    Err(e.into())
                }
            }
        } else {
            // No config exists, run first-time setup
            println!("{}", "=".repeat(60).color(colors::HEADER));
            println!("{}", "   🧹 CLEANCRUSH - FIRST TIME SETUP   ".bold());
            println!("{}", "=".repeat(60).color(colors::HEADER));
            println!();
            
            let config = Self::run_first_time_wizard()?;
            config.save()?;
            
            println!();
            println!("{} Setup complete! Your preferences are saved.", "✅".green());
            println!("{} Try: {}", "💡".cyan(), "cleancrush scan ~/Downloads".bold());
            println!();
            
            Ok(config)
        }
    }
    
    /// Load config from backup file
    fn load_backup() -> Result<Self> {
        let backup_path = Self::backup_path()?;
        if backup_path.exists() {
            let data = fs::read_to_string(&backup_path)
                .context("Failed to read backup file")?;
            serde_json::from_str(&data).context("Failed to parse backup file")
        } else {
            Err(anyhow::anyhow!("No backup file found"))
        }
    }
    
    /// Save config to disk with backup
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        let backup_path = Self::backup_path()?;
        
        // Create backup of existing config if it exists
        if config_path.exists() {
            fs::copy(&config_path, &backup_path)
                .context("Failed to create backup")?;
        }
        
        // Write to temp file first
        let temp_path = config_path.with_extension("json.tmp");
        let data = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;
        fs::write(&temp_path, &data)
            .context("Failed to write temp config")?;
        
        // Atomically rename temp file to final location
        fs::rename(&temp_path, &config_path)
            .context("Failed to finalize config")?;
        
        Ok(())
    }
    
    /// Run interactive first-time wizard
    fn run_first_time_wizard() -> Result<Self> {
        let theme = ColorfulTheme::default();
        
        // 1. Default cleanup action
        println!("{}", "1. DEFAULT CLEANUP ACTION".bold());
        let action_items = &["Move to Recycle Bin (30-day restore)", "Archive to organized folders"];
        let action_idx = Select::with_theme(&theme)
            .items(action_items)
            .default(0)
            .interact()?;
        
        let default_action = match action_idx {
            0 => CleanupAction::RecycleBin,
            1 => CleanupAction::Archive,
            _ => unreachable!(),
        };
        
        println!();
        
        // 2. Folder protection
        println!("{}", "2. FOLDER PROTECTION".bold());
        println!("Which folders contain personal files?");
        
        let mut default_folders = vec![
            (dirs::home_dir().unwrap().join("Documents"), false),
            (dirs::home_dir().unwrap().join("Desktop").join("Personal"), false),
            (dirs::home_dir().unwrap().join("Pictures"), false),
            (dirs::home_dir().unwrap().join("Projects"), false),
        ];
        
        let selections = MultiSelect::with_theme(&theme)
            .items(&["Documents", "Desktop/Personal", "Pictures", "Projects"])
            .interact()?;
        
        for &idx in &selections {
            default_folders[idx].1 = true;
        }
        
        // Ask for custom folders
        let _custom_folders: Vec<PathBuf> = Vec::new();
        loop {
            let add_custom = Confirm::with_theme(&theme)
                .with_prompt("Add another custom folder?")
                .default(false)
                .interact()?;
            
            if !add_custom {
                break;
            }
            
            let custom_path: String = Input::with_theme(&theme)
                .with_prompt("Folder path")
                .interact_text()?;
            
            let path = PathBuf::from(custom_path);
            if path.exists() {
                default_folders.push((path, true));
            } else {
                println!("{} Path does not exist, skipping", "⚠️".yellow());
            }
        }
        
        println!();
        
        // 3. Protection type
        println!("{}", "3. PROTECTION TYPE".bold());
        let protection_items = &[
            "Hard - Never scan protected folders",
            "Soft - Scan but warn before any action",
        ];
        let protection_idx = Select::with_theme(&theme)
            .items(protection_items)
            .default(1)
            .interact()?;
        
        let protection_type = match protection_idx {
            0 => ProtectionType::Hard,
            1 => ProtectionType::Soft,
            _ => unreachable!(),
        };
        
        println!();
        
        // 4. Exam monitoring
        println!("{}", "4. EXAM MONITORING".bold());
        let enable_monitoring = Confirm::with_theme(&theme)
            .with_prompt("Monitor Downloads/Desktop for exam periods?")
            .default(true)
            .interact()?;
        
        if enable_monitoring {
            println!("{} Only tracks file counts, never contents", "ℹ️".cyan());
        }
        
        println!();
        
        // 5. Reminder schedule
        println!("{}", "5. REMINDER SCHEDULE".bold());
        let reminder_items = &["Never", "Weekly (Sundays)", "Monthly (1st of month)"];
        let reminder_idx = Select::with_theme(&theme)
            .items(reminder_items)
            .default(1)
            .interact()?;
        
        let reminder_schedule = match reminder_idx {
            0 => ReminderSchedule::Never,
            1 => ReminderSchedule::Weekly,
            2 => ReminderSchedule::Monthly,
            _ => unreachable!(),
        };
        
        // Build protected folders list
        let protected_folders = default_folders
            .into_iter()
            .filter(|(_, selected)| *selected)
            .map(|(path, _)| ProtectedFolder {
                path,
                protection_type: protection_type.clone(),
            })
            .collect();
        
        Ok(Config {
            default_action,
            protected_folders,
            reminder_schedule,
            enable_exam_monitoring: enable_monitoring,
            last_cleanup: None,
            last_reminder: None,
            exam_tracking: None,
            streaks: 0,
            achievements: Vec::new(),
            total_files_cleaned: 0,
            total_space_freed_mb: 0,
        })
    }
    
    /// Check if a path is protected
    pub fn is_protected(&self, path: &Path) -> Option<&ProtectedFolder> {
        for protected in &self.protected_folders {
            if path.starts_with(&protected.path) {
                return Some(protected);
            }
        }
        None
    }
    
    /// Check if a path is a system path
    pub fn is_system_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        SYSTEM_PATHS.iter().any(|sys: &&str| path_str.contains(&sys.to_lowercase()))
    }
    
    /// Update last cleanup timestamp
    pub fn update_last_cleanup(&mut self) -> Result<()> {
        self.last_cleanup = Some(Utc::now().to_rfc3339());
        self.save()
    }
    
    /// Check if reminders are due
    pub fn is_reminder_due(&self) -> bool {
        match &self.last_cleanup {
            None => true, // Never cleaned before
            Some(last) => {
                let last_date: DateTime<Utc> = last.parse().unwrap_or(Utc::now());
                let now = Utc::now();
                let days_since = (now - last_date).num_days();
                
                match self.reminder_schedule {
                    ReminderSchedule::Never => false,
                    ReminderSchedule::Weekly => days_since >= 7,
                    ReminderSchedule::Monthly => days_since >= 30,
                }
            }
        }
    }
    
    /// Add an achievement if not already earned
    pub fn add_achievement(&mut self, achievement: &str) {
        if !self.achievements.contains(&achievement.to_string()) {
            self.achievements.push(achievement.to_string());
        }
    }
    
    /// Increment streak counter
    pub fn increment_streak(&mut self) {
        self.streaks += 1;
        
        // Check for streak achievements
        if self.streaks == 1 {
            self.add_achievement("🧹 First Sweep");
        } else if self.streaks >= 21 { // 3 weeks
            self.add_achievement("📆 Consistency Cutie");
        }
    }
    
    /// Update statistics after cleanup
    pub fn update_stats(&mut self, files_cleaned: usize, space_freed_bytes: u64) {
        self.total_files_cleaned += files_cleaned as u64;
        self.total_space_freed_mb += space_freed_bytes / (1024 * 1024);
        
        // Increment streak if criteria met (from blueprint)
        if files_cleaned >= 5 || space_freed_bytes >= 50 * 1024 * 1024 {
            self.increment_streak();
        }
        
        // Check for achievements
        if self.total_files_cleaned >= 10 {
            self.add_achievement("🔁 Duplicate Slayer");
        }
        if self.total_space_freed_mb >= 500 {
            self.add_achievement("💾 Space Hero");
        }
    }
    
     /// Deactivate exam tracking in config
    pub fn deactivate_exam_tracking(&mut self) -> Result<()> {
        if let Some(tracking) = &mut self.exam_tracking {
            if tracking.active {
                tracking.active = false;
                tracking.end_date = Some(Utc::now().to_rfc3339());
                self.save()?;
            }
        }
        Ok(())
    }
    
    /// Display current configuration
    pub fn display(&self) {
        println!("{}", "🔧 CURRENT CONFIGURATION".bold().color(colors::HEADER));
        println!();
        
        println!("{} Default action: {}", "•".cyan(), match self.default_action {
            CleanupAction::RecycleBin => "Move to Recycle Bin",
            CleanupAction::Archive => "Archive to organized folders",
        });
        
        println!("{} Exam monitoring: {}", "•".cyan(), 
            if self.enable_exam_monitoring { "Enabled" } else { "Disabled" });
        
        println!("{} Reminder schedule: {}", "•".cyan(), match self.reminder_schedule {
            ReminderSchedule::Never => "Never",
            ReminderSchedule::Weekly => "Weekly (Sundays)",
            ReminderSchedule::Monthly => "Monthly (1st)",
        });
        
        println!();
        println!("{} Protected folders ({}):", "•".cyan(), self.protected_folders.len());
        for protected in &self.protected_folders {
            let protection_type = match protected.protection_type {
                ProtectionType::Hard => "Hard (never scan)",
                ProtectionType::Soft => "Soft (scan but warn)",
            };
            println!("  - {} ({})", protected.path.display(), protection_type);
        }
        
        if let Some(last) = &self.last_cleanup {
            println!("{} Last cleanup: {}", "•".cyan(), last);
        }
        
        println!("{} Current streak: {} days", "•".cyan(), self.streaks);
        println!("{} Total files cleaned: {}", "•".cyan(), self.total_files_cleaned);
        println!("{} Total space freed: {:.1} MB", "•".cyan(), self.total_space_freed_mb);
    }
}