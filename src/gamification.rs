#[allow(unused_imports)]
use chrono::{Utc, Duration, Datelike};

use serde::{Deserialize, Serialize};
use colored::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;
use crate::{colors, ENCOURAGEMENTS, Config};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gamification {
    pub current_streak: u32,
    pub longest_streak: u32,
    pub last_cleanup_date: Option<chrono::DateTime<Utc>>,
    pub achievements: HashMap<String, Achievement>,
    pub total_cleanups: u32,
    pub total_files_cleaned: u64,
    pub total_space_freed_mb: u64,
    pub daily_stats: HashMap<String, DailyStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub unlocked: bool,
    pub unlocked_date: Option<chrono::DateTime<Utc>>,
    pub progress: f32, // 0.0 to 1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub date: String,
    pub files_cleaned: u32,
    pub space_freed_mb: u32,
    pub cleanup_type: CleanupType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupType {
    Normal,
    Exam,
    Archive,
    Duplicate,
}

impl Gamification {
    /// Create new gamification system
    pub fn new() -> Self {
        let mut achievements = HashMap::new();
        
        // Define all achievements
        let achievement_list = vec![
            Achievement {
                id: "first_sweep".to_string(),
                name: "🧹 First Sweep".to_string(),
                description: "Complete your first cleanup".to_string(),
                icon: "🧹".to_string(),
                unlocked: false,
                unlocked_date: None,
                progress: 0.0,
            },
            Achievement {
                id: "exam_reset".to_string(),
                name: "🎓 Exam Reset".to_string(),
                description: "Complete your first post-exam cleanup".to_string(),
                icon: "🎓".to_string(),
                unlocked: false,
                unlocked_date: None,
                progress: 0.0,
            },
            Achievement {
                id: "duplicate_slayer".to_string(),
                name: "🔁 Duplicate Slayer".to_string(),
                description: "Remove 10+ duplicate files".to_string(),
                icon: "🔁".to_string(),
                unlocked: false,
                unlocked_date: None,
                progress: 0.0,
            },
            Achievement {
                id: "space_hero".to_string(),
                name: "💾 Space Hero".to_string(),
                description: "Free 500+ MB of space".to_string(),
                icon: "💾".to_string(),
                unlocked: false,
                unlocked_date: None,
                progress: 0.0,
            },
            Achievement {
                id: "consistency_cutie".to_string(),
                name: "📆 Consistency Cutie".to_string(),
                description: "Clean for 3 weeks in a row".to_string(),
                icon: "📆".to_string(),
                unlocked: false,
                unlocked_date: None,
                progress: 0.0,
            },
            Achievement {
                id: "organized_ace".to_string(),
                name: "✨ Organized Ace".to_string(),
                description: "Achieve 90+ cleanliness score".to_string(),
                icon: "✨".to_string(),
                unlocked: false,
                unlocked_date: None,
                progress: 0.0,
            },
            Achievement {
                id: "fresh_start".to_string(),
                name: "🌸 Fresh Start".to_string(),
                description: "First cleanup after setup".to_string(),
                icon: "🌸".to_string(),
                unlocked: false,
                unlocked_date: None,
                progress: 0.0,
            },
        ];
        
        for achievement in achievement_list {
            achievements.insert(achievement.id.clone(), achievement);
        }
        
        Self {
            current_streak: 0,
            longest_streak: 0,
            last_cleanup_date: None,
            achievements,
            total_cleanups: 0,
            total_files_cleaned: 0,
            total_space_freed_mb: 0,
            daily_stats: HashMap::new(),
        }
    }
    
    /// Load gamification from config
    pub fn load_from_config(config: &Config) -> Self {
        let mut gamification = Self::new();
        
        gamification.current_streak = config.streaks;
        gamification.total_files_cleaned = config.total_files_cleaned;
        gamification.total_space_freed_mb = config.total_space_freed_mb;
        
        // Update achievements from config
        for achievement_name in &config.achievements {
            if let Some(achievement) = gamification.achievements.get_mut(achievement_name) {
                achievement.unlocked = true;
                achievement.progress = 1.0;
            }
        }
        
        // Update longest streak
        if config.streaks > gamification.longest_streak {
            gamification.longest_streak = config.streaks;
        }
        
        gamification
    }
    
    /// Update gamification after cleanup
    pub fn update_after_cleanup(
        &mut self, 
        files_cleaned: usize, 
        space_freed_bytes: u64,
        cleanup_type: CleanupType,
        is_exam_cleanup: bool,
    ) -> Vec<AchievementUnlock> {
        let today = Utc::now();
        let today_str = today.format("%Y-%m-%d").to_string();
        let space_freed_mb = space_freed_bytes / (1024 * 1024);
        
        // Update totals
        self.total_cleanups += 1;
        self.total_files_cleaned += files_cleaned as u64;
        self.total_space_freed_mb += space_freed_mb;
        
        // Update daily stats
        let daily_stat = DailyStats {
            date: today_str.clone(),
            files_cleaned: files_cleaned as u32,
            space_freed_mb: space_freed_mb as u32,
            cleanup_type: cleanup_type.clone(),
        };
        self.daily_stats.insert(today_str, daily_stat);
        
        // Update streak
        self.update_streak(today);
        
        // Check for achievement unlocks
        let mut unlocks = Vec::new();
        
        // Check each achievement
        unlocks.extend(self.check_achievements(files_cleaned, space_freed_mb, is_exam_cleanup));
        
        unlocks
    }
    
    /// Update streak counter
    fn update_streak(&mut self, cleanup_date: chrono::DateTime<Utc>) {
        if let Some(last_date) = self.last_cleanup_date {
            let days_since = (cleanup_date - last_date).num_days();
            
            if days_since == 1 {
                // Consecutive day
                self.current_streak += 1;
            } else if days_since > 1 {
                // Streak broken
                self.current_streak = 1;
            }
            // days_since == 0 means same day, don't increment
        } else {
            // First cleanup
            self.current_streak = 1;
        }
        
        // Update longest streak
        if self.current_streak > self.longest_streak {
            self.longest_streak = self.current_streak;
        }
        
        self.last_cleanup_date = Some(cleanup_date);
    }
    
    /// Check for achievement unlocks
    fn check_achievements(
        &mut self, 
        _files_cleaned: usize, 
        _space_freed_mb: u64,
        is_exam_cleanup: bool,
    ) -> Vec<AchievementUnlock> {
        let mut unlocks = Vec::new();
        let today = Utc::now();
        
        // First Sweep
        if !self.achievements["first_sweep"].unlocked && self.total_cleanups == 1 {
            let achievement = self.achievements.get_mut("first_sweep").unwrap();
            achievement.unlocked = true;
            achievement.unlocked_date = Some(today);
            achievement.progress = 1.0;
            unlocks.push(AchievementUnlock::new(achievement));
        }
        
        // Fresh Start (first cleanup after setup)
        if !self.achievements["fresh_start"].unlocked && self.total_cleanups == 1 {
            let achievement = self.achievements.get_mut("fresh_start").unwrap();
            achievement.unlocked = true;
            achievement.unlocked_date = Some(today);
            achievement.progress = 1.0;
            unlocks.push(AchievementUnlock::new(achievement));
        }
        
        // Exam Reset
        if !self.achievements["exam_reset"].unlocked && is_exam_cleanup {
            let achievement = self.achievements.get_mut("exam_reset").unwrap();
            achievement.unlocked = true;
            achievement.unlocked_date = Some(today);
            achievement.progress = 1.0;
            unlocks.push(AchievementUnlock::new(achievement));
        }
        
        // Duplicate Slayer
        if !self.achievements["duplicate_slayer"].unlocked {
            let achievement = self.achievements.get_mut("duplicate_slayer").unwrap();
            let progress = (self.total_files_cleaned as f32 / 10.0).min(1.0);
            achievement.progress = progress;
            
            if self.total_files_cleaned >= 10 {
                achievement.unlocked = true;
                achievement.unlocked_date = Some(today);
                unlocks.push(AchievementUnlock::new(achievement));
            }
        }
        
        // Space Hero
        if !self.achievements["space_hero"].unlocked {
            let achievement = self.achievements.get_mut("space_hero").unwrap();
            let progress = (self.total_space_freed_mb as f32 / 500.0).min(1.0);
            achievement.progress = progress;
            
            if self.total_space_freed_mb >= 500 {
                achievement.unlocked = true;
                achievement.unlocked_date = Some(today);
                unlocks.push(AchievementUnlock::new(achievement));
            }
        }
        
        // Consistency Cutie
        if !self.achievements["consistency_cutie"].unlocked {
            let achievement = self.achievements.get_mut("consistency_cutie").unwrap();
            let progress = (self.current_streak as f32 / 21.0).min(1.0); // 3 weeks = 21 days
            achievement.progress = progress;
            
            if self.current_streak >= 21 {
                achievement.unlocked = true;
                achievement.unlocked_date = Some(today);
                unlocks.push(AchievementUnlock::new(achievement));
            }
        }
        
        unlocks
    }
    
    /// Calculate cleanliness score for a folder
    pub fn calculate_cleanliness_score(
        &self,
        duplicates: usize,
        old_files: usize,
        large_files: usize,
        very_large_files: usize,
    ) -> (u32, String) {
        let mut score: u32 = 100;
        let mut breakdown = Vec::new();
        
        // Penalties
        let duplicate_penalty = duplicates * 2;
        let old_penalty = old_files * 1;
        let large_penalty = large_files * 1;
        let very_large_penalty = very_large_files * 3;
        
        score = score.saturating_sub(duplicate_penalty as u32);
        score = score.saturating_sub(old_penalty as u32);
        score = score.saturating_sub(large_penalty as u32);
        score = score.saturating_sub(very_large_penalty as u32);
        
        // Build breakdown
        if duplicate_penalty > 0 {
            breakdown.push(format!("-{}: {} duplicate{}", 
                duplicate_penalty, duplicates, 
                if duplicates == 1 { "" } else { "s" }));
        }
        if old_penalty > 0 {
            breakdown.push(format!("-{}: {} old file{}", 
                old_penalty, old_files, 
                if old_files == 1 { "" } else { "s" }));
        }
        if large_penalty > 0 {
            breakdown.push(format!("-{}: {} large file{}", 
                large_penalty, large_files, 
                if large_files == 1 { "" } else { "s" }));
        }
        if very_large_penalty > 0 {
            breakdown.push(format!("-{}: {} very large file{}", 
                very_large_penalty, very_large_files, 
                if very_large_files == 1 { "" } else { "s" }));
        }
        
        let breakdown_str = if breakdown.is_empty() {
            "Perfect! No issues found ✨".to_string()
        } else {
            breakdown.join("\n")
        };
        
        (score, breakdown_str)
    }
    
    /// Get a random encouragement message
    pub fn get_encouragement_message(&self) -> String {
        let mut rng = thread_rng();
        ENCOURAGEMENTS.choose(&mut rng)
            .unwrap_or(&"Great job! 🎉")
            .to_string()
    }
    
    /// Display statistics
    pub fn display_stats(&self) {
        println!();
        println!("{}", "📊 YOUR STATISTICS".bold().color(colors::HEADER));
        println!("{}", "─".repeat(50).color(colors::PATH));
        
        println!("🔥 Current streak: {} day{}", 
            self.current_streak.to_string().color(colors::SUCCESS),
            if self.current_streak == 1 { "" } else { "s" });
        
        if self.longest_streak > self.current_streak {
            println!("🏆 Longest streak: {} day{}", 
                self.longest_streak.to_string().color(colors::SUCCESS),
                if self.longest_streak == 1 { "" } else { "s" });
        }
        
        println!("🧹 Total cleanups: {}", 
            self.total_cleanups.to_string().color(colors::PATH));
        println!("📁 Total files cleaned: {}", 
            self.total_files_cleaned.to_string().color(colors::PATH));
        println!("💾 Total space freed: {:.1} MB", 
            self.total_space_freed_mb.to_string().color(colors::PATH));
        
        // Show recent activity
        self.display_recent_activity();
        
        // Show achievements
        self.display_achievements();
    }
    
    /// Display recent activity
    fn display_recent_activity(&self) {
        let mut dates: Vec<_> = self.daily_stats.keys().collect();
        dates.sort();
        dates.reverse();
        
        let recent: Vec<_> = dates.into_iter()
            .take(5)
            .filter_map(|date| self.daily_stats.get(date))
            .collect();
        
        if !recent.is_empty() {
            println!();
            println!("{}", "📈 RECENT ACTIVITY".dimmed());
            for stat in recent {
                let icon = match stat.cleanup_type {
                    CleanupType::Normal => "🧹",
                    CleanupType::Exam => "🎓",
                    CleanupType::Archive => "📁",
                    CleanupType::Duplicate => "🔁",
                };
                println!("   {} {}: {} files, {:.1} MB",
                    icon,
                    stat.date,
                    stat.files_cleaned,
                    stat.space_freed_mb as f32
                );
            }
        }
    }
    
    /// Display achievements
    pub fn display_achievements(&self) {
        let unlocked: Vec<_> = self.achievements.values()
            .filter(|a| a.unlocked)
            .collect();
        
        let locked: Vec<_> = self.achievements.values()
            .filter(|a| !a.unlocked)
            .collect();
        
        if !unlocked.is_empty() {
            println!();
            println!("{}", "🏆 ACHIEVEMENTS UNLOCKED".bold().color(colors::SUCCESS));
            for achievement in unlocked {
                let date_str = achievement.unlocked_date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "Recently".to_string());
                println!("   {} {} - {} ({})",
                    achievement.icon,
                    achievement.name,
                    achievement.description.dimmed(),
                    date_str.dimmed()
                );
            }
        }
        
        if !locked.is_empty() {
            println!();
            println!("{}", "🔒 ACHIEVEMENTS TO EARN".dimmed());
            for achievement in locked {
                let progress_bar = self.create_progress_bar(achievement.progress, 10);
                println!("   {} {} - {} [{}]",
                    achievement.icon,
                    achievement.name.dimmed(),
                    achievement.description.dimmed(),
                    progress_bar
                );
            }
        }
    }
    
    /// Create progress bar string
    fn create_progress_bar(&self, progress: f32, width: usize) -> String {
        let filled = (progress * width as f32).round() as usize;
        let empty = width.saturating_sub(filled);
        
        format!("[{}{}] {:.0}%",
            "█".repeat(filled),
            "░".repeat(empty),
            progress * 100.0
        )
    }
    
    /// Show encouragement after cleanup
    pub fn show_encouragement(
        &self, 
        files_cleaned: usize, 
        space_freed_mb: u64,
        unlocks: &[AchievementUnlock],
    ) {
        println!();
        
        // Show main encouragement
        let message = self.get_encouragement_message();
        println!("{} {}", "💖".color(colors::HIGH_CONFIDENCE), message);
        
        // Show streak update if applicable
        if self.current_streak > 1 {
            println!("{} Streak: {} days in a row!", 
                "🔥".color(colors::WARNING), 
                self.current_streak);
        }
        
        // Show achievement unlocks
        if !unlocks.is_empty() {
            println!();
            println!("{} NEW ACHIEVEMENT{} UNLOCKED!", 
                "🎉".color(colors::SUCCESS),
                if unlocks.len() == 1 { "" } else { "S" });
            
            for unlock in unlocks {
                println!("   {} {} - {}",
                    unlock.icon,
                    unlock.name.bold(),
                    unlock.description.dimmed()
                );
            }
        }
        
        // Show cleanup summary
        println!();
        println!("{} Cleaned {} files, freed {:.1} MB",
            "✅".green(),
            files_cleaned,
            space_freed_mb as f32
        );
    }
}

#[derive(Debug, Clone)]
pub struct AchievementUnlock {
    pub name: String,
    pub description: String,
    pub icon: String,
}

impl AchievementUnlock {
    fn new(achievement: &Achievement) -> Self {
        Self {
            name: achievement.name.clone(),
            description: achievement.description.clone(),
            icon: achievement.icon.clone(),
        }
    }
}