use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use chrono::{DateTime, Utc, Duration};
use walkdir::WalkDir;
use blake3;
use regex::Regex;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use anyhow::{Result, Context};
use crate::colors;
use crate::{FileCategory, DEFAULT_OLD_DAYS, DEFAULT_LARGE_MB};
use crate::config::{Config, ProtectedFolder, ProtectionType};

const STUDY_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "pptx", "txt", "md", "ipynb",
    "py", "java", "c", "cpp", "rs", "js", "html",
    "csv", "xlsx",
];
const EXAM_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "pptx", "txt", "md", "ipynb",
    "py", "java", "c", "cpp", "rs", "js", "html",
    "csv", "xlsx", "png", "jpg", "jpeg",
];
const STUDY_PATTERNS: &[&str] = &[
    "lecture", "notes", "assignment", "homework", "lab",
    "exam", "quiz", "week", "chapter", "slide", "tutorial",
    "worksheet", "solution", "practice", "review",
];
const DUPLICATE_PATTERNS: &[&str] = &[
    "copy", "(1)", "(2)", "_copy", "-copy",
    "final_final", "old", "backup", "version",
];
const CLOUD_FOLDERS: &[&str] = &[
    "Google Drive", "Dropbox", "OneDrive", "iCloud Drive", "Box", "Sync",
];
const COURSE_PATTERNS: &[(&str, &[&str])] = &[
    ("math", &["math", "calculus", "algebra", "geometry"]),
    ("cs", &["cs", "computer science", "programming", "data structures"]),
    ("physics", &["physics", "mechanics", "quantum"]),
    ("chemistry", &["chemistry", "organic", "inorganic"]),
    ("biology", &["biology", "genetics", "ecology"]),
    ("history", &["history", "world history", "us history"]),
    ("literature", &["literature", "english", "novel"]),
];
const MAX_FILES_TO_SCAN: usize = 5000;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified: DateTime<Utc>,
    pub created: DateTime<Utc>,
    pub days_old: i64,
    pub course: String,
    pub file_type: String,
    pub hash: Option<String>,
    pub confidence: f32,
    pub reason: String,
    pub category: FileCategory,
    pub is_in_cloud: bool,
    pub is_locked: bool,
}

#[derive(Debug)]
pub struct ScanResult {
    pub files: Vec<FileInfo>,
    pub total_files_scanned: usize,
    pub total_size_bytes: u64,
    pub duplicates_found: usize,
    pub old_files_found: usize,
    pub large_files_found: usize,
    pub cloud_files_found: usize,
    pub scan_duration: Duration,
}

pub struct Scanner {
    config: Config,
    is_exam_mode: bool,
    course_regexes: Vec<(String, Regex)>,
}

impl Scanner {
    pub fn new(config: Config, is_exam_mode: bool) -> Self {
        // Compile course detection regexes
        let course_regexes = COURSE_PATTERNS
            .iter()
            .map(|(course, patterns): &(&str, &[&str])| {
                let pattern = patterns.join("|");
                let regex = Regex::new(&format!(r"(?i)({})", pattern))
                    .expect("Invalid course regex");
                (course.to_string(), regex)
            })
            .collect();
        
        Self {
            config,
            is_exam_mode,
            course_regexes,
        }
    }
    
    /// Helper to demonstrate ProtectedFolder is used
    fn get_protection_info(&self, path: &Path) -> Option<&ProtectedFolder> {
        self.config.is_protected(path)
    }
    
    /// Scan a directory for study files
    pub fn scan(&self, path: &Path, days_threshold: u64, large_threshold_mb: u64) -> Result<ScanResult> {
        let start_time = Utc::now();
        
        println!("{} {}", "🔍 Scanning:".color(colors::HEADER), path.display());
        
        if !path.exists() {
            return Err(anyhow::anyhow!("Path does not exist: {}", path.display()));
        }
        
        if Config::is_system_path(path) {
            println!("{} Skipping system path: {}", "⚠️".yellow(), path.display());
            return Ok(ScanResult::empty());
        }
        
        // Check if path is protected - ACTUALLY USE ProtectedFolder
        if let Some(protected) = self.get_protection_info(path) {
            match protected.protection_type {
                ProtectionType::Hard => {
                    println!("{} Skipping protected folder: {}", "🛡️".blue(), path.display());
                    return Ok(ScanResult::empty());
                }
                ProtectionType::Soft => {
                    println!("{} Scanning protected folder (will warn before actions): {}", "⚠️".yellow(), path.display());
                }
            }
        }
        
        // Collect all candidate files
        let candidates = self.collect_candidates(path)?;
        let candidates_clone = candidates.clone();

        if candidates.is_empty() {
            println!("{} No study files found", "✨".green());
            return Ok(ScanResult::empty());
        }
        
        println!("Found {} candidate files", candidates.len());
        
        // Detect duplicates
        let (hash_cache, hash_groups) = self.detect_duplicates(&candidates);
        
        // Analyze each candidate
        let mut files = Vec::new();
        let mut total_size = 0;
        let mut duplicates_found = 0;
        let mut old_files_found = 0;
        let mut large_files_found = 0;
        let mut cloud_files_found = 0;
        
        let pb = ProgressBar::new(candidates.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} files ({eta})")?
                .progress_chars("#>-")
        );
        
        for (path, size, modified, created) in candidates {
            pb.inc(1);
            
            // Skip if file no longer exists
            if !path.exists() {
                continue;
            }
            
            let days_old = (Utc::now() - modified).num_days();
            let course = self.detect_course(&path);
            let file_type = self.get_file_type(&path);
            
            // Check for duplicates using hash_groups
            let is_duplicate = if let Some(hash) = hash_cache.get(&path) {
                hash_groups.get(hash).map(|g| g.len() > 1).unwrap_or(false)
            } else {
                false
            };
            
            let category = if is_duplicate {
                FileCategory::Duplicate
            } else {
                self.categorize_file(&path, days_old, size, large_threshold_mb)
            };
            
            let is_in_cloud = self.is_in_cloud_folder(&path);
            let is_locked = self.is_file_locked(&path);
            
            if is_in_cloud {
                cloud_files_found += 1;
            }
            
            // Calculate confidence and reason - PASS hash_groups
            let (confidence, reason) = self.calculate_confidence(
                &path, days_old, size, days_threshold, large_threshold_mb, 
                &hash_groups, &category, is_duplicate
            );
            
            // Skip low confidence files during normal mode
            if !self.is_exam_mode && confidence < 0.4 {
                continue;
            }
            
            // Count categories
            match category {
                FileCategory::Duplicate => duplicates_found += 1,
                FileCategory::Old => old_files_found += 1,
                FileCategory::Large => large_files_found += 1,
                _ => {}
            }
            
            total_size += size;
            
            files.push(FileInfo {
                path: path.clone(),
                size_bytes: size,
                modified,
                created,
                days_old,
                course,
                file_type,
                hash: hash_cache.get(&path).cloned(),
                confidence,
                reason,
                category,
                is_in_cloud,
                is_locked,
            });
        }
        
        pb.finish_and_clear();
        
        // Sort by confidence (highest first)
        files.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        
        let scan_duration = Utc::now() - start_time;
        
        Ok(ScanResult {
            files,
            total_files_scanned: candidates_clone.len(),
            total_size_bytes: total_size,
            duplicates_found,
            old_files_found,
            large_files_found,
            cloud_files_found,
            scan_duration,
        })
    }
    
    /// Collect candidate study files
    fn collect_candidates(&self, path: &Path) -> Result<Vec<(PathBuf, u64, DateTime<Utc>, DateTime<Utc>)>> {
        let mut candidates = Vec::new();
        let mut file_count = 0;
        
        let walker = WalkDir::new(path)
            .max_depth(3) // Limit depth for performance
            .follow_links(false) // Don't follow symlinks
            .into_iter()
            .filter_map(|e| e.ok());
        
        for entry in walker {
            if file_count >= MAX_FILES_TO_SCAN {
                println!("{} Scanned maximum {} files. Stopping early.", "⚠️".yellow(), MAX_FILES_TO_SCAN);
                break;
            }
            
            let entry_path = entry.path();
            
            // Skip directories
            if !entry.file_type().is_file() {
                continue;
            }
            
            // Skip system files
            if Config::is_system_path(entry_path) {
                continue;
            }
            
            // Check protection - USE ProtectedFolder
            if let Some(protected) = self.get_protection_info(entry_path) {
                if matches!(protected.protection_type, ProtectionType::Hard) {
                    continue;
                }
            }
            
            // Check file extension
            let extension = entry_path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase();
            
            let extensions = if self.is_exam_mode {
                EXAM_EXTENSIONS
            } else {
                STUDY_EXTENSIONS
            };
            
            if !extensions.contains(&extension.as_str()) {
                continue;
            }
            
            // Get file metadata
            let metadata = match fs::metadata(entry_path) {
                Ok(m) => m,
                Err(_) => continue, // Skip files we can't read
            };
            
            let size = metadata.len();
            let modified: DateTime<Utc> = metadata.modified()
                .unwrap_or_else(|_| SystemTime::now())
                .into();
            let created: DateTime<Utc> = metadata.created()
                .unwrap_or_else(|_| SystemTime::now())
                .into();
            
            candidates.push((entry_path.to_path_buf(), size, modified, created));
            file_count += 1;
        }
        
        Ok(candidates)
    }
    
    /// Detect duplicate files using hashing
    fn detect_duplicates(
        &self, 
        candidates: &[(PathBuf, u64, DateTime<Utc>, DateTime<Utc>)]
    ) -> (std::collections::HashMap<PathBuf, String>, std::collections::HashMap<String, Vec<PathBuf>>) {
        let mut size_groups = std::collections::HashMap::new();
        let mut hash_cache = std::collections::HashMap::new();
        let mut hash_groups = std::collections::HashMap::new();
        
        // Group by size first
        for (path, size, _, _) in candidates {
            size_groups.entry(*size).or_insert_with(Vec::new).push(path.clone());
        }
        
        // Hash only files with same size (potential duplicates)
        for (size, paths) in size_groups {
            if size == 0 || paths.len() < 2 {
                continue;
            }
            
            for path in paths {
                if let Ok(hash) = self.hash_file(&path) {
                    hash_cache.insert(path.clone(), hash.clone());
                    hash_groups.entry(hash).or_insert_with(Vec::new).push(path.clone());
                }
            }
        }
        
        (hash_cache, hash_groups)
    }
    
    /// Hash a file using streaming (memory-safe)
    fn hash_file(&self, path: &Path) -> Result<String> {
        let mut hasher = blake3::Hasher::new();
        let mut file = fs::File::open(path).context("Failed to open file for hashing")?;
        
        let mut buffer = [0u8; 8192]; // 8KB chunks - memory safe
        loop {
            let n = std::io::Read::read(&mut file, &mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        
        Ok(hasher.finalize().to_string())
    }
    
    /// Detect course from filename
    fn detect_course(&self, path: &Path) -> String {
        let filename = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        
        for (course, regex) in &self.course_regexes {
            if regex.is_match(&filename) {
                return course.clone();
            }
        }
        
        "general".to_string()
    }
    
    /// Get file type string
    fn get_file_type(&self, path: &Path) -> String {
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown")
            .to_lowercase()
    }
    
    /// Categorize file
    fn categorize_file(
        &self, 
        path: &Path, 
        days_old: i64, 
        size: u64, 
        large_threshold_mb: u64,
    ) -> FileCategory {
        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        
        // Check filename patterns
        if filename.contains("lecture") || filename.contains("slide") || filename.contains("presentation") {
            return FileCategory::Lecture;
        }
        
        if filename.contains("assignment") || filename.contains("homework") || filename.contains("hw") {
            return FileCategory::Assignment;
        }
        
        if filename.contains("textbook") || filename.contains("book") || filename.contains("reference") {
            return FileCategory::Reference;
        }
        
        // Check age and size
        let large_threshold_bytes = large_threshold_mb * 1024 * 1024;
        
        if days_old > DEFAULT_OLD_DAYS as i64 {
            return FileCategory::Old;
        }
        
        if size > large_threshold_bytes {
            return FileCategory::Large;
        }
        
        FileCategory::Other
    }
    
    /// Check if file is in cloud folder
    fn is_in_cloud_folder(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        CLOUD_FOLDERS.iter().any(|folder: &&str| path_str.contains(&folder.to_lowercase()))
    }
    
    /// Check if file is locked (open in another program)
    fn is_file_locked(&self, path: &Path) -> bool {
        // Try to open file in read-write mode to check if it's locked
        match fs::OpenOptions::new().read(true).write(true).open(path) {
            Ok(_) => false,
            Err(_) => true,
        }
    }
    
    /// Calculate confidence score and reason - USE hash_groups parameter
    fn calculate_confidence(
        &self,
        path: &Path,
        days_old: i64,
        size: u64,
        days_threshold: u64,
        large_threshold_mb: u64,
        hash_groups: &std::collections::HashMap<String, Vec<PathBuf>>, // USE THIS!
        category: &FileCategory,
        is_duplicate: bool,
    ) -> (f32, String) {
        let mut confidence: f32 = 0.0;
        let mut reasons = Vec::new();
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        
        // Check for exact duplicates using hash_groups
        if is_duplicate {
            // ACTUALLY USE hash_groups to count duplicates
            let mut duplicate_count = 0;
            for (_, group) in hash_groups {
                if group.contains(&path.to_path_buf()) && group.len() > 1 {
                    duplicate_count = group.len();
                    break;
                }
            }
            
            if duplicate_count > 0 {
                confidence = 0.99;
                reasons.push(format!("Exact duplicate ({} copies)", duplicate_count));
            }
        }
        
        // Check for duplicate filename patterns
        for pattern in DUPLICATE_PATTERNS {
            if filename.to_lowercase().contains(pattern) {
                confidence = confidence.max(0.85);
                reasons.push("Filename suggests duplicate".to_string());
                break;
            }
        }
        
        // Age-based confidence
        if days_old > 90 {
            confidence = confidence.max(0.95);
            reasons.push(format!("Very old ({} days)", days_old));
        } else if days_old > days_threshold as i64 {
            let age_confidence = 0.7 + ((days_old - days_threshold as i64) as f32 / 30.0).min(0.25);
            confidence = confidence.max(age_confidence);
            reasons.push(format!("Old ({} days)", days_old));
        }
        
        // Size-based confidence
        let large_threshold_bytes = large_threshold_mb * 1024 * 1024;
        if size > large_threshold_bytes {
            let size_mb = size as f32 / (1024.0 * 1024.0);
            let size_confidence = 0.7 + (size_mb / 1000.0).min(0.25);
            confidence = confidence.max(size_confidence);
            reasons.push(format!("Large file ({:.1} MB)", size_mb));
        }
        
        // Study pattern confidence
        for pattern in STUDY_PATTERNS {
            if filename.to_lowercase().contains(pattern) {
                confidence = confidence.max(0.75);
                reasons.push("Study-related file".to_string());
                break;
            }
        }
        
        // Category-based adjustments
        match category {
            FileCategory::Lecture | FileCategory::Assignment | FileCategory::Reference => {
                confidence = confidence.max(0.65);
            }
            FileCategory::Old => {
                confidence = confidence.max(0.85);
            }
            FileCategory::Large => {
                confidence = confidence.max(0.75);
            }
            FileCategory::Other => {
                // Lower confidence for uncategorized
                confidence = confidence.max(0.4);
            }
            FileCategory::Duplicate => {
                // Already handled above
            }
        }
        
        // Exam mode adjustments (screenshots have lower confidence)
        if self.is_exam_mode {
            let extension = path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase();
            
            if extension == "png" || extension == "jpg" || extension == "jpeg" {
                confidence = confidence.min(0.4); // Cap screenshot confidence
                reasons.push("Screenshot (lower confidence)".to_string());
            }
        }
        
        // Default minimum confidence
        confidence = confidence.max(0.1);
        
        // Build reason string
        let reason = if reasons.is_empty() {
            "General study file".to_string()
        } else {
            reasons.join(" + ")
        };
        
        (confidence.min(1.0), reason)
    }
    
    /// Print scan results in a nice format
    pub fn print_results(&self, result: &ScanResult, show_detailed: bool) {
        println!();
        println!("{}", "📊 SCAN RESULTS".bold().color(colors::HEADER));
        println!("{}", "─".repeat(50).color(colors::PATH));
        
        // USE total_suggestions method
        println!("🎯 Cleanup suggestions: {}", 
            result.total_suggestions().to_string().color(colors::SUCCESS));
        
        println!("📁 Total files scanned: {}", 
            result.total_files_scanned.to_string().color(colors::SUCCESS));
        println!("💾 Total size: {:.2} MB", 
            (result.total_size_bytes as f64 / (1024.0 * 1024.0)).to_string().color(colors::SUCCESS));
        println!("⏱️  Scan time: {} seconds", 
            result.scan_duration.num_seconds().to_string().dimmed());
        
        println!();
        println!("{}", "🎯 FINDINGS".bold().color(colors::HEADER));
        println!("🔄 Duplicates: {}", 
            result.duplicates_found.to_string().color(colors::WARNING));
        println!("📅 Old files (>{} days): {}", DEFAULT_OLD_DAYS,
            result.old_files_found.to_string().color(colors::WARNING));
        println!("💪 Large files (>{} MB): {}", DEFAULT_LARGE_MB,
            result.large_files_found.to_string().color(colors::WARNING));
        
        if result.cloud_files_found > 0 {
            println!("☁️  Cloud files: {}", 
                result.cloud_files_found.to_string().color(colors::WARNING));
        }
        
        if !result.files.is_empty() {
            println!();
            println!("{}", "✨ TOP SUGGESTIONS".bold().color(colors::HEADER));
            println!("{}", "─".repeat(50).color(colors::PATH));
            
            for (i, file) in result.files.iter().take(10).enumerate() {
                let confidence_color = if file.confidence > 0.8 {
                    colors::HIGH_CONFIDENCE
                } else if file.confidence > 0.6 {
                    colors::MEDIUM_CONFIDENCE
                } else {
                    colors::LOW_CONFIDENCE
                };
                
                let size_mb = file.size_bytes as f32 / (1024.0 * 1024.0);
                
                print!("{:3}. [{}{:.2}{}] {}",
                    i + 1,
                    "⚡".color(confidence_color),
                    file.confidence,
                    "⚡".color(colors::SUCCESS),
                    file.path.display().to_string().color(colors::PATH)
                );
                
                if show_detailed {
                    println!();
                    // USE all FileInfo fields
                    println!("     Type: {}, Course: {}, Size: {:.1} MB", 
                        file.file_type.to_uppercase().color(colors::HEADER),
                        file.course.color(colors::SUCCESS),
                        size_mb
                    );
                    println!("     Modified: {} ({} days ago), Created: {}", 
                        file.modified.format("%Y-%m-%d").to_string().dimmed(),
                        file.days_old,
                        file.created.format("%Y-%m-%d").to_string().dimmed()
                    );
                    println!("     Hash: {}", 
                        file.hash.as_ref().unwrap_or(&"N/A".to_string()).color(colors::PATH));
                    println!("     Reason: {}", file.reason.dimmed());
                    
                    if file.is_in_cloud {
                        println!("     {} In cloud folder", "☁️".yellow());
                    }
                    if file.is_locked {
                        println!("     {} File may be open in another program", "⚠️".yellow());
                    }
                } else {
                    println!();
                }
            }
            
            if result.files.len() > 10 {
                println!("     ... and {} more files", result.files.len() - 10);
            }
            
            println!();
            println!("{} Run {} for detailed suggestions", 
                "💡".cyan(),
                "cleancrush suggest".bold());
        } else {
            println!();
            println!("{} No cleanup suggestions! Your files look clean ✨", "🎉".green());
        }
    }
}

impl ScanResult {
    /// Create empty scan result
    fn empty() -> Self {
        Self {
            files: Vec::new(),
            total_files_scanned: 0,
            total_size_bytes: 0,
            duplicates_found: 0,
            old_files_found: 0,
            large_files_found: 0,
            cloud_files_found: 0,
            scan_duration: Duration::zero(),
        }
    }
    
    /// Get files by category
    pub fn files_by_category(&self, category: FileCategory) -> Vec<&FileInfo> {
        self.files.iter()
            .filter(|f| f.category == category)
            .collect()
    }
    
    /// Get total number of suggestions
    pub fn total_suggestions(&self) -> usize {
        self.files.len()
    }
}