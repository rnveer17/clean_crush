# 🧹 CleanCrush

CleanCrush is a Rust-based command-line tool that intelligently tracks study files during exam periods and provides privacy-first cleanup options afterward. Built by a student for students, it solves the real problem of post-exam digital clutter while respecting academic workflows and privacy.

## ✨ Features

### 🎓 Exam-Aware Tracking
- **Auto-detects exam periods** when you create >15 study files in 7 days
- **Tracks files during exams** for organized post-exam cleanup
- **Smart categorization** into Lectures, Assignments, References, Other

### 🔒 Privacy-First Design
- **Never reads file contents** - metadata analysis only
- **Local-only processing** - no cloud, no internet required
- **Protected folders** - your personal files stay safe

### 🧹 Intelligent Cleanup
- **Confidence scoring** (0.0-1.0) for cleanup suggestions
- **Course detection** (CS, Math, Science, Engineering, etc.)
- **Duplicate detection** using Blake3 hashing
- **Old file identification** (>60 days)
- **Large file identification** (>100 MB)

### 🎮 Gamification & Motivation
- **Cleanliness scores** (0-100) for folders
- **Achievements & streaks** for consistent cleaning
- **Encouraging messages** to keep you motivated

### 🛡️ Safety First
- **Recycle Bin/Trash first** - 30-day restore window
- **Dry run mode** - preview changes before applying
- **Safe mode** - disable all file modifications
- **Confirmation prompts** - prevent accidental deletions

## 📦 Installation

### Prerequisites
- Install **Rust** on your system (version 1.70+)
- Ensure `cargo` is available in your terminal

### Installation Steps
```bash
# 1. Clone the repository
git clone https://github.com/rnveer17/clean_crush.git
cd clean_crush

# 2. Build in release mode
cargo build --release

# 3. Install globally
sudo cp target/release/clean_crush /usr/local/bin/cleancrush

# 4. Verify installation
cleancrush --version
```

## 🚀 Getting Started
### First-Time Setup
Run CleanCrush without arguments to start the interactive wizard:

```bash
cleancrush
```
The wizard guides you through:
1. Default cleanup action (Recycle Bin or organized archive)

2. Protected folders (personal files never scanned)

3. Exam monitoring (auto-detect exam periods)

4. Reminder schedule (weekly/monthly cleanup reminders)

### Basic Workflow
1. Scan your files: `cleancrush scan ~/Downloads`

2. Get suggestions: `cleancrush suggest ~/Downloads`

3. Start exam mode: `cleancrush exam on --name "Final Exams"`

4. End and cleanup: `cleancrush exam end`

## 💻 Essential Commands
### Quick Start Commands
```bash
# Scan your files
cleancrush scan ~/Downloads

# See cleanup suggestions
cleancrush suggest ~/Downloads

# Start exam tracking
cleancrush exam on --name "Final Exams"

# Quick cleanup
cleancrush clean --mode all ~/Downloads

# View your archives (if using Archive mode)
cleancrush archive list
```
### Student Workflow
```bash
# Before exams
cleancrush exam on --name "Final Exams"

# During exams (CleanCrush auto-tracks)

# After exams
cleancrush exam end

# Manage archives (if using Archive mode)
cleancrush archive list
cleancrush archive clean --days 30
```

### Information & Stats
```bash
# Check folder health
cleancrush score ~/Downloads

# View your progress
cleancrush stats
cleancrush achievements

# Show configuration (check Archive/Recycle Bin mode)
cleancrush config
```
📄 View Full Command Reference PDF - Complete list of all commands and options

🏗️ Project Structure
```text
clean_crush/
├── src/
│   ├── main.rs              # Entry point & command routing
│   ├── lib.rs               # Library exports & constants
│   ├── config.rs            # Configuration & first-run wizard
│   ├── scanner.rs           # File scanning & categorization
│   ├── exam.rs              # Exam tracking logic
│   ├── archive.rs           # Archive/delete operations
│   ├── gamification.rs      # Streaks, achievements, scoring
│   └── cli.rs               # CLI argument parsing
├── docs/
│   └── index.html          # Project website
├── .github/workflows/
│   └── build.yml           # CI/CD for cross-compilation
├── Cargo.toml              # Rust dependencies & metadata
├── .gitignore              # Git ignore rules
├── README.md               # This file
└── LICENSE                 # MIT License
```

##🌐 Project Website
Visit our website for interactive demos, downloads, and documentation:


##📄 License
MIT License