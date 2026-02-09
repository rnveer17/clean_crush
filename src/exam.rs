#[allow(unused_imports)]
use chrono::{DateTime, Utc, Duration, Datelike};
#[allow(unused_imports)]
use anyhow::{Result, Context};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use colored::*;
use dialoguer::{theme::ColorfulTheme, Select, Confirm};
use crate::colors;
use crate::config::Config;

pub const DEFAULT_EXAM_DETECTION_FILES: usize = 15;
pub const DEFAULT_EXAM_DETECTION_DAYS: u64 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamTracker {
    pub active: bool,
    pub start_date: DateTime<Utc>,
    pub end_date: Option<DateTime<Utc>>,
    pub auto_detected: bool,
    pub tracked_files: HashMap<PathBuf, FileTrackingInfo>,
    pub exam_period_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTrackingInfo {
    pub added_date: DateTime<Utc>,
    pub size_bytes: u64,
    pub file_type: String,
    pub course: String,
    pub category: FileCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileCategory {
    Lecture,
    Assignment,
    Reference,
    Other,
}

impl ExamTracker {
    /// Create a new exam tracker
    pub fn new(auto_detected: bool, exam_name: Option<String>) -> Self {
        Self {
            active: true,
            start_date: Utc::now(),
            end_date: None,
            auto_detected,
            tracked_files: HashMap::new(),
            exam_period_name: exam_name,
        }
    }
    
    /// Check if exam tracking should be auto-started
    pub fn should_auto_start(config: &Config, recent_study_files: usize) -> bool {
        if !config.enable_exam_monitoring {
            return false;
        }
        
        recent_study_files >= DEFAULT_EXAM_DETECTION_FILES
    }
    
    /// Show auto-detection prompt
    pub fn show_auto_detection_prompt(recent_files: usize, existing_files: usize) -> Result<Option<Self>> {
        println!();
        println!("{}", "📚 EXAM DETECTION".bold().color(colors::HEADER));
        println!("{}", "─".repeat(50).color(colors::PATH));
        
        println!("Detected {} new study files in {} days.", 
            recent_files.to_string().color(colors::SUCCESS),
            DEFAULT_EXAM_DETECTION_DAYS.to_string().color(colors::SUCCESS));
        
        if existing_files > 0 {
            println!("Found {} existing study files from last 30 days.", 
                existing_files.to_string().color(colors::SUCCESS));
        }
        
        println!();
        println!("{} Start tracking for post-exam cleanup?", "🎓".color(colors::HEADER));
        
        let choices = &[
            "Yes, start tracking",
            "No, don't track",
            "Snooze (ask again in 3 days)",
        ];
        
        let selection = Select::with_theme(&ColorfulTheme::default())
            .items(choices)
            .default(0)
            .interact()?;
        
        match selection {
            0 => {
                // Ask for exam period name
                println!();
                let name = if Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Give this exam period a name? (e.g., 'Final Exams Fall 2024')")
                    .default(false)
                    .interact()?
                {
                    use dialoguer::Input;
                    Some(Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Exam period name")
                        .interact_text()?)
                } else {
                    None
                };
                
                let tracker = Self::new(true, name);
                println!("{} Exam tracking started!", "✅".green());
                Ok(Some(tracker))
            }
            1 => {
                println!("{} Exam tracking skipped.", "ℹ️".cyan());
                Ok(None)
            }
            2 => {
                println!("{} Will ask again in 3 days.", "⏰".yellow());
                Ok(None)
            }
            _ => unreachable!(),
        }
    }
    
    /// Add a file to tracking
    pub fn add_file(&mut self, path: PathBuf, size_bytes: u64, file_type: String, course: String, category: FileCategory) {
        let info = FileTrackingInfo {
            added_date: Utc::now(),
            size_bytes,
            file_type,
            course,
            category,
        };
        
        self.tracked_files.insert(path, info);
    }
    
    /// End exam tracking
    pub fn end_exam(&mut self) {
        self.active = false;
        self.end_date = Some(Utc::now());
    }
    
    /// Check if exam has ended
    pub fn has_ended(&self) -> bool {
        !self.active
    }
    
    /// Get days since exam started
    pub fn days_since_start(&self) -> i64 {
        let now = Utc::now();
        (now - self.start_date).num_days()
    }
    
    /// Get total tracked files count
    pub fn total_files(&self) -> usize {
        self.tracked_files.len()
    }
    
    /// Get total tracked size in MB
    pub fn total_size_mb(&self) -> f64 {
        let total_bytes: u64 = self.tracked_files.values().map(|info| info.size_bytes).sum();
        total_bytes as f64 / (1024.0 * 1024.0)
    }
    
    /// Get files by category
    pub fn files_by_category(&self, category: FileCategory) -> Vec<(&PathBuf, &FileTrackingInfo)> {
        self.tracked_files.iter()
            .filter(|(_, info)| info.category == category)
            .collect()
    }
    
    /// Display exam status
    pub fn display_status(&self) {
        println!();
        println!("{}", "🎓 EXAM MODE STATUS".bold().color(colors::HEADER));
        println!("{}", "─".repeat(50).color(colors::PATH));
        
        if let Some(name) = &self.exam_period_name {
            println!("📝 Period: {}", name.color(colors::SUCCESS));
        }
        
        println!("📅 Started: {}", self.start_date.format("%Y-%m-%d").to_string().color(colors::SUCCESS));
        
        if let Some(end_date) = self.end_date {
            println!("🏁 Ended: {}", end_date.format("%Y-%m-%d").to_string().color(colors::SUCCESS));
        } else {
            println!("⏳ Duration: {} days", self.days_since_start().to_string().color(colors::SUCCESS));
        }
        
        println!("📁 Files tracked: {}", self.total_files().to_string().color(colors::SUCCESS));
        println!("💾 Total size: {:.1} MB", self.total_size_mb().to_string().color(colors::SUCCESS));
        
        // Show breakdown by category
        let lectures = self.files_by_category(FileCategory::Lecture).len();
        let assignments = self.files_by_category(FileCategory::Assignment).len();
        let references = self.files_by_category(FileCategory::Reference).len();
        let other = self.files_by_category(FileCategory::Other).len();
        
        println!();
        println!("{}", "📊 CATEGORY BREAKDOWN".dimmed());
        println!("📚 Lectures: {}", lectures.to_string().color(colors::PATH));
        println!("📝 Assignments: {}", assignments.to_string().color(colors::PATH));
        println!("📖 References: {}", references.to_string().color(colors::PATH));
        println!("🎫 Other: {}", other.to_string().color(colors::PATH));
        
        if self.active {
            println!();
            println!("{} Run {} when exams end to clean up.", 
                "💡".cyan(), 
                "cleancrush exam end".bold());
        }
    }
    
    /// Show post-exam cleanup options
    pub fn show_post_exam_options(&self, _config: &Config) -> Result<PostExamChoice> {
        println!();
        println!("{}", "🎓 EXAM PERIOD COMPLETE!".bold().color(colors::HEADER));
        println!("{}", "─".repeat(50).color(colors::PATH));
        
        println!("Found {} files tracked during exams ({:.1} MB).", 
            self.total_files().to_string().color(colors::SUCCESS),
            self.total_size_mb().to_string().color(colors::SUCCESS));
        
        println!();
        println!("{}", "Choose cleanup method:".bold());
        
        let options = vec![
            PostExamOption::QuickClean {
                description: "Move ALL files to Recycle Bin/Archive".to_string(),
                details: vec![
                    "Fast and simple".to_string(),
                    "30-day restore window".to_string(),
                    "Recommended for most students".to_string(),
                ],
            },
            PostExamOption::SelectiveClean {
                description: "Review files by category".to_string(),
                details: vec![
                    "Choose what to keep/delete".to_string(),
                    "More control".to_string(),
                    "Takes 5-10 minutes".to_string(),
                ],
            },
            PostExamOption::SmartClean {
                description: "Keep references, clean others".to_string(),
                details: vec![
                    "Automatic smart selection".to_string(),
                    "Keeps reference materials".to_string(),
                    "Cleans lectures & assignments".to_string(),
                ],
            },
        ];
        
        for (i, option) in options.iter().enumerate() {
            println!();
            print!("{}. {} ", i + 1, option.get_description().bold());
            
            match option {
                PostExamOption::QuickClean { .. } => print!("{}", "🚀".color(colors::SUCCESS)),
                PostExamOption::SelectiveClean { .. } => print!("{}", "🎯".color(colors::HEADER)),
                PostExamOption::SmartClean { .. } => print!("{}", "🤖".color(colors::PATH)),
            }
            
            println!();
            for detail in option.get_details() {
                println!("   • {}", detail.dimmed());
            }
        }
        
        println!();
        let choice_idx = Select::with_theme(&ColorfulTheme::default())
            .items(&["Quick Clean", "Selective Clean", "Smart Clean"])
            .default(0)
            .interact()?;
        
        let choice = match choice_idx {
            0 => PostExamChoice::QuickClean,
            1 => PostExamChoice::SelectiveClean,
            2 => PostExamChoice::SmartClean,
            _ => unreachable!(),
        };
        
        // Show confirmation
        println!();
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("Proceed with {}?", choice.display_name()))
            .default(true)
            .interact()?;
        
        if !confirm {
            return Err(anyhow::anyhow!("Cleanup cancelled"));
        }
        
        Ok(choice)
    }
    
    /// Get files for post-exam cleanup based on choice
    pub fn get_files_for_cleanup(&self, choice: PostExamChoice) -> Vec<PathBuf> {
        match choice {
            PostExamChoice::QuickClean => {
                // All files
                self.tracked_files.keys().cloned().collect()
            }
            PostExamChoice::SelectiveClean => {
                // All files (user will select in UI)
                self.tracked_files.keys().cloned().collect()
            }
            PostExamChoice::SmartClean => {
                // Keep references, clean others
                self.tracked_files.iter()
                    .filter(|(_, info)| info.category != FileCategory::Reference)
                    .map(|(path, _)| path.clone())
                    .collect()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum PostExamOption {
    QuickClean {
        description: String,
        details: Vec<String>,
    },
    SelectiveClean {
        description: String,
        details: Vec<String>,
    },
    SmartClean {
        description: String,
        details: Vec<String>,
    },
}

impl PostExamOption {
    fn get_description(&self) -> &str {
        match self {
            Self::QuickClean { description, .. } => description,
            Self::SelectiveClean { description, .. } => description,
            Self::SmartClean { description, .. } => description,
        }
    }
    
    fn get_details(&self) -> &[String] {
        match self {
            Self::QuickClean { details, .. } => details,
            Self::SelectiveClean { details, .. } => details,
            Self::SmartClean { details, .. } => details,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PostExamChoice {
    QuickClean,
    SelectiveClean,
    SmartClean,
}

impl PostExamChoice {
    fn display_name(&self) -> &'static str {
        match self {
            Self::QuickClean => "Quick Clean",
            Self::SelectiveClean => "Selective Clean",
            Self::SmartClean => "Smart Clean",
        }
    }
}

/// Manage exam mode state
pub struct ExamManager {
    tracker: Option<ExamTracker>,
    config: Config,
}

impl ExamManager {
    pub fn new(config: Config) -> Self {
        Self {
            tracker: None,
            config,
        }
    }
    
    /// Check and update exam tracking state
    pub fn update_tracking(&mut self, recent_study_files: usize, existing_study_files: usize) -> Result<()> {
        // Check if we should auto-start exam tracking
        if self.tracker.is_none() 
            && ExamTracker::should_auto_start(&self.config, recent_study_files) 
            && self.config.enable_exam_monitoring {
        
            if let Some(tracker) = ExamTracker::show_auto_detection_prompt(recent_study_files, existing_study_files)? {
                // Clone BEFORE moving into self.tracker
                let tracker_clone = tracker.clone();
                self.tracker = Some(tracker);
                self.config.exam_tracking = Some(tracker_clone.into());
                self.config.save()?;
            }
        }
        
        Ok(())
    }
    
/// Start exam tracking manually
pub fn start_manual(&mut self, exam_name: Option<String>) -> Result<()> {
    // Check if we already have an ACTIVE exam
    let has_active_exam = self.tracker.as_ref().map_or(false, |t| t.active) ||
        self.config.exam_tracking.as_ref().map_or(false, |t| t.active);
    
    if has_active_exam {
        return Err(anyhow::anyhow!("Exam tracking is already active"));
    }
    
    let tracker = ExamTracker::new(false, exam_name);
    println!("{} Exam tracking started manually", "✅".green());
    
    self.tracker = Some(tracker.clone());
    
    // Ensure config is updated
    self.config.exam_tracking = Some(tracker.into());
    self.config.save()?;
    
    Ok(())
}

/// Stop exam tracking
pub fn stop(&mut self) -> Result<()> {
    let was_active = self.tracker.is_some() || 
        self.config.exam_tracking.as_ref().map_or(false, |t| t.active);
    
    if let Some(tracker) = &mut self.tracker {
        tracker.end_exam();
        self.tracker = None;
    }
    
    // Use the existing method to deactivate exam tracking
    self.config.deactivate_exam_tracking()?;
    
    if was_active {
        println!("{} Exam tracking stopped", "✅".green());
    } else {
        println!("{} No active exam tracking", "ℹ️".cyan());
    }
    
    Ok(())
}
    
    /// Set exam dates manually
    pub fn set_dates(&mut self, start_date: DateTime<Utc>, end_date: DateTime<Utc>, exam_name: Option<String>) -> Result<()> {
    if self.tracker.is_none() {
        self.start_manual(exam_name.clone())?;
    }
    
    if let Some(tracker) = &mut self.tracker {
        tracker.start_date = start_date;
        tracker.end_date = Some(end_date);
        
        if let Some(name) = exam_name {
            tracker.exam_period_name = Some(name);
        }
        
        self.config.exam_tracking = Some(tracker.clone().into());
        self.config.save()?;
        
        // Show appropriate message with name if available
        if let Some(name) = &tracker.exam_period_name {
            println!("{} Exam '{}' dates set: {} to {}", 
                "✅".green(),
                name,
                start_date.format("%Y-%m-%d"),
                end_date.format("%Y-%m-%d"));
        } else {
            println!("{} Exam dates set: {} to {}", 
                "✅".green(),
                start_date.format("%Y-%m-%d"),
                end_date.format("%Y-%m-%d"));
        }
    }
    
    Ok(())
}
    
    /// End exam and show cleanup options
    pub fn end_exam(&mut self) -> Result<Option<PostExamChoice>> {
        if let Some(tracker) = &mut self.tracker {
            if tracker.has_ended() {
                println!("{} Exam already ended", "ℹ️".cyan());
                tracker.display_status();
                return Ok(None);
            }
            
            tracker.end_exam();
            tracker.display_status();
            
            let choice = tracker.show_post_exam_options(&self.config)?;
            
            // Update config
            self.config.exam_tracking = Some(tracker.clone().into());
            self.config.save()?;
            
            self.tracker = None;

            Ok(Some(choice))
        } else {
            println!("{} No active exam to end", "⚠️".yellow());
            Ok(None)
        }
    }
    
    /// Get current tracker
    pub fn get_tracker(&self) -> Option<&ExamTracker> {
        self.tracker.as_ref()
    }
    
    /// Check if exam mode is active
    pub fn is_active(&self) -> bool {
        self.tracker.as_ref().map_or(false, |t| t.active)
    }
    
    /// Add file to tracking if exam mode is active
    pub fn track_file_if_active(
        &mut self, 
        path: PathBuf, 
        size_bytes: u64, 
        file_type: String, 
        course: String, 
        category: crate::exam::FileCategory
    ) {
        if let Some(tracker) = &mut self.tracker {
            if tracker.active {
                tracker.add_file(path, size_bytes, file_type, course, category);
            }
        }
    }
    
    /// Show current status
    pub fn show_status(&self) {
        if let Some(tracker) = &self.tracker {
            tracker.display_status();
        } else {
            println!("{} Exam mode: Not active", "ℹ️".cyan());
            println!("   Run {} to start tracking", "cleancrush exam on".bold());
        }
    }
    
    /// Load tracker from config
pub fn load_from_config(&mut self) -> Result<()> {
    if let Some(tracking_state) = &self.config.exam_tracking {
        // FIX: Only load if ACTIVE
        if tracking_state.active {
            // Convert ExamTrackingState to ExamTracker
            let tracker = ExamTracker {
                active: tracking_state.active,
                start_date: tracking_state.start_date.parse().unwrap_or(Utc::now()),
                end_date: tracking_state.end_date.as_ref().and_then(|d| d.parse().ok()),
                auto_detected: false,
                tracked_files: tracking_state.tracked_files.iter()
                    .map(|path| (path.clone(), FileTrackingInfo {
                        added_date: Utc::now(),
                        size_bytes: 0,
                        file_type: "unknown".to_string(),
                        course: "general".to_string(),
                        category: FileCategory::Other,
                    }))
                    .collect(),
                exam_period_name: tracking_state.exam_period_name.clone(),
            };
            
            self.tracker = Some(tracker);
        } else {
            // If config says INACTIVE, don't load tracker!
            self.tracker = None;
        }
    }
    
    Ok(())
}
}

impl From<ExamTracker> for crate::config::ExamTrackingState {
     fn from(tracker: ExamTracker) -> Self {
        Self {
            active: tracker.active,
            start_date: tracker.start_date.to_rfc3339(),
            end_date: tracker.end_date.map(|d| d.to_rfc3339()),
            tracked_files: tracker.tracked_files.keys().cloned().collect(),
            exam_period_name: tracker.exam_period_name.clone(),
        }
    }
}
