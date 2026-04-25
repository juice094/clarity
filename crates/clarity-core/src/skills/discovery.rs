//! Runtime skill discovery based on file paths and project directories.

use super::{Skill, SkillLoader};
use std::path::Path;

/// Discovers skills from project-specific directories.
#[derive(Debug, Clone, Default)]
pub struct SkillDiscovery;

impl SkillDiscovery {
    /// Scan a single project directory for `.clarity/skills/` and `.claude/skills/`
    /// subdirectories and load all `.md` files found within them.
    pub fn scan_project_skills(project_dir: &Path) -> Vec<Skill> {
        let mut skills = Vec::new();
        let candidates = [
            project_dir.join(".clarity").join("skills"),
            project_dir.join(".claude").join("skills"),
        ];

        for dir in &candidates {
            if dir.is_dir() {
                match SkillLoader::load_dir(dir) {
                    Ok(mut found) => skills.append(&mut found),
                    Err(e) => {
                        tracing::warn!("Failed to load skills from {}: {}", dir.display(), e);
                    }
                }
            }
        }

        skills
    }
}

/// Check whether a file path matches a gitignore-style pattern.
///
/// Uses `glob::Pattern` when possible and falls back to simple
/// substring / suffix checks for patterns that `glob` cannot parse.
pub fn path_matches_pattern(file_path: &Path, pattern: &str) -> bool {
    let path_str = file_path.to_string_lossy().replace('\\', "/");
    let normalized = pattern.replace('\\', "/");

    // Try glob match on the full path.
    if let Ok(pat) = glob::Pattern::new(&normalized) {
        if pat.matches(&path_str) {
            return true;
        }
    }

    // Try glob match on the file name only.
    if let Some(name) = file_path.file_name() {
        let name_str = name.to_string_lossy();
        if let Ok(pat) = glob::Pattern::new(&normalized) {
            if pat.matches(&name_str) {
                return true;
            }
        }
    }

    // Fallback suffix / component match.
    path_str.ends_with(&normalized)
        || path_str.contains(&format!("/{normalized}"))
        || path_str.contains(&format!("\\{normalized}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_scan_project_skills_empty() {
        let tmp = std::env::temp_dir();
        let skills = SkillDiscovery::scan_project_skills(&tmp);
        assert!(skills.is_empty());
    }

    #[test]
    fn test_scan_project_skills_with_clarity_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join(".clarity").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        std::fs::write(
            skills_dir.join("test.md"),
            "---\nid: test-skill\nname: Test Skill\n---\n\nBody.\n",
        )
        .unwrap();

        let skills = SkillDiscovery::scan_project_skills(tmp.path());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].meta.id, "test-skill");
    }

    #[test]
    fn test_scan_project_skills_with_claude_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join(".claude").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        std::fs::write(
            skills_dir.join("claude.md"),
            "---\nid: claude-skill\nname: Claude Skill\n---\n\nBody.\n",
        )
        .unwrap();

        let skills = SkillDiscovery::scan_project_skills(tmp.path());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].meta.id, "claude-skill");
    }

    #[test]
    fn test_path_matches_glob_star() {
        assert!(path_matches_pattern(Path::new("src/main.rs"), "*.rs"));
        assert!(!path_matches_pattern(Path::new("src/main.rs"), "*.toml"));
    }

    #[test]
    fn test_path_matches_full_path() {
        assert!(path_matches_pattern(
            Path::new("src/components/App.rs"),
            "src/**/*.rs"
        ));
    }

    #[test]
    fn test_path_matches_exact_name() {
        assert!(path_matches_pattern(
            Path::new("/home/user/project/Cargo.toml"),
            "Cargo.toml"
        ));
        assert!(path_matches_pattern(Path::new("Cargo.toml"), "Cargo.toml"));
    }

    #[test]
    fn test_path_matches_fallback_suffix() {
        assert!(path_matches_pattern(
            Path::new("foo/bar/baz.rs"),
            "bar/baz.rs"
        ));
    }

    #[test]
    fn test_path_matches_backslash_normalized() {
        let p = PathBuf::from("src\\main.rs");
        assert!(path_matches_pattern(&p, "src/*.rs"));
    }
}
