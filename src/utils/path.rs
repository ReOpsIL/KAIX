//! Path utilities and workspace management

use crate::utils::errors::KaiError;
use std::path::{Path, PathBuf};

/// Represents a workspace/working directory context
#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    /// The root path of the workspace
    pub root: PathBuf,
    /// Whether this workspace has been validated
    pub validated: bool,
}

impl WorkspaceContext {
    /// Create a new workspace context
    pub fn new<P: AsRef<Path>>(root: P) -> Result<Self, KaiError> {
        let root_path = root.as_ref();
        let root = root_path.canonicalize()
            .map_err(|e| KaiError::file_system(root_path.to_path_buf(), e))?;

        if !root.is_dir() {
            return Err(KaiError::execution(format!(
                "Workspace root must be a directory: {}",
                root.display()
            )));
        }

        Ok(Self {
            root,
            validated: false,
        })
    }

    /// Validate the workspace (check for common project files)
    pub fn validate(&mut self) -> Result<(), KaiError> {
        // Check if it looks like a valid project directory
        let common_files = [
            "Cargo.toml",
            "package.json",
            "pyproject.toml",
            "go.mod",
            "pom.xml",
            "build.gradle",
            ".git",
        ];

        let has_project_file = common_files.iter().any(|file| {
            self.root.join(file).exists()
        });

        if !has_project_file {
            tracing::warn!(
                "Workspace {} doesn't appear to contain a recognized project",
                self.root.display()
            );
        }

        self.validated = true;
        Ok(())
    }

    /// Resolve a relative path against the workspace root
    pub fn resolve<P: AsRef<Path>>(&self, relative_path: P) -> PathBuf {
        let path = relative_path.as_ref();
        if path.is_absolute() {
            // Ensure the absolute path is within the workspace
            if let Ok(canonical) = path.canonicalize() {
                if canonical.starts_with(&self.root) {
                    return canonical;
                }
            }
            // If not within workspace, treat as relative
            self.root.join(path.strip_prefix("/").unwrap_or(path))
        } else {
            self.root.join(path)
        }
    }

    /// Check if a path is within the workspace
    pub fn contains<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        if let Ok(canonical) = path.canonicalize() {
            canonical.starts_with(&self.root)
        } else {
            false
        }
    }

    /// Get a relative path from the workspace root
    pub fn relative_path<P: AsRef<Path>>(&self, path: P) -> Option<PathBuf> {
        let path = path.as_ref();
        if let Ok(canonical) = path.canonicalize() {
            canonical.strip_prefix(&self.root).ok().map(|p| p.to_path_buf())
        } else {
            None
        }
    }
}

/// Normalize a path for consistent representation
pub fn normalize_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    
    // Convert to forward slashes for consistency across platforms
    let path_str = path.to_string_lossy();
    let normalized = path_str.replace('\\', "/");
    
    PathBuf::from(normalized)
}

/// Check if a path has a specific extension
pub fn has_extension<P: AsRef<Path>>(path: P, extension: &str) -> bool {
    path.as_ref()
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

/// Get file stem (filename without extension)
pub fn file_stem<P: AsRef<Path>>(path: P) -> Option<String> {
    path.as_ref()
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|s| s.to_string())
}