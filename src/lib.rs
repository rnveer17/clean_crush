//! CleanCrush - Student-focused exam file cleanup tool

pub mod config;
pub mod scanner;
pub mod exam;
pub mod archive;
pub mod gamification;
pub mod cli;

// Re-exports for easy access
pub use config::{Config, CleanupAction, ProtectedFolder, ProtectionType, ReminderSchedule, ExamTrackingState};
pub use scanner::{FileInfo, ScanResult, Scanner};
pub use exam::{ExamManager, ExamTracker, PostExamChoice};
pub use archive::{ArchiveSystem, ArchiveInfo};
pub use gamification::{Gamification, AchievementUnlock, CleanupType};
pub use cli::{Cli, Commands};

// Export all constants
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

/// Current version of CleanCrush
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum files to scan before prompting user
pub const MAX_FILES_TO_SCAN: usize = 5000;

/// Default thresholds
pub const DEFAULT_OLD_DAYS: u64 = 60;
pub const DEFAULT_LARGE_MB: u64 = 100;
pub const DEFAULT_EXAM_DETECTION_FILES: usize = 15;
pub const DEFAULT_EXAM_DETECTION_DAYS: u64 = 7;

/// Study file extensions
pub const STUDY_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "pptx", "txt", "md", "ipynb",
    "py", "java", "c", "cpp", "rs", "js", "html",
    "csv", "xlsx",
];

/// Exam mode extensions (includes screenshots)
pub const EXAM_EXTENSIONS: &[&str] = &[
    "pdf", "docx", "pptx", "txt", "md", "ipynb",
    "py", "java", "c", "cpp", "rs", "js", "html",
    "csv", "xlsx", "png", "jpg", "jpeg",
];

/// Study filename patterns
pub const STUDY_PATTERNS: &[&str] = &[
    "lecture", "notes", "assignment", "homework", "lab",
    "exam", "quiz", "week", "chapter", "slide", "tutorial",
    "worksheet", "solution", "practice", "review",
];

/// Duplicate filename patterns
pub const DUPLICATE_PATTERNS: &[&str] = &[
    "copy", "(1)", "(2)", "_copy", "-copy",
    "final_final", "old", "backup", "version",
];

/// Cloud sync folder names
pub const CLOUD_FOLDERS: &[&str] = &[
    "Google Drive", "Dropbox", "OneDrive", "iCloud Drive", "Box", "Sync",
];

/// System paths to never touch
pub const SYSTEM_PATHS: &[&str] = &[
    r"C:\Windows", r"C:\Program Files", r"C:\ProgramData",
    r"C:\System Volume Information", "/System", "/usr",
    "/bin", "/sbin", "/etc", "/var", "/lib",
];

/// Course detection patterns
pub const COURSE_PATTERNS: &[(&str, &[&str])] = &[
    ("cs", &["cs", "computer", "programming", "algorithm", "software"]),
    ("math", &["math", "calculus", "algebra", "statistics", "geometry"]),
    ("science", &["physics", "chemistry", "biology", "science", "lab"]),
    ("engineering", &["engineer", "mechanical", "electrical", "civil", "robotics"]),
    ("business", &["business", "management", "finance", "economics", "marketing"]),
    ("humanities", &["history", "literature", "philosophy", "art", "psychology"]),
];

/// Cute encouragement messages
pub const ENCOURAGEMENTS: &[&str] = &[
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