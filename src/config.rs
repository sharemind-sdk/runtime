/*
 * Copyright (C) 2026 Cybernetica
 *
 * Research/Commercial License Usage
 * Licensees holding a valid Research License or Commercial License
 * for the Software may use this file according to the written
 * agreement between you and Cybernetica.
 *
 * GNU General Public License Usage
 * Alternatively, this file may be used under the terms of the GNU
 * General Public License version 3.0 as published by the Free Software
 * Foundation and appearing in the file LICENSE included in the
 * packaging of this file.  Please review the following information to
 * ensure the GNU General Public License version 3.0 requirements will be
 * met: http://www.gnu.org/copyleft/gpl-3.0.html.
 *
 * For further information, please contact us at sharemind@cyber.ee.
 */

use std::path::{Path, PathBuf};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigPathError {
    #[error("Absolute path does not exist: {0}")]
    AbsolutePathNotFound(PathBuf),

    #[error("Path exists but is not a file: {0}")]
    NotAFile(PathBuf),

    #[error("File '{}' not found in any search paths. Searched: {}", .target.display(), .searched_paths.join(", "))]
    NotFoundInSearchPaths {
        target: PathBuf,
        searched_paths: Vec<String>,
    },
}

/// Build the ordered list of directories to search for config files.
/// Priority order:
/// 1. User-provided extra paths (highest priority)
/// 2. Current working directory
/// 3. $HOME/.config/<appname>
/// 4. /usr/local/etc/<appname>
/// 5. /etc/<appname>
pub fn build_config_search_paths(
    app_dir_name: &str,
    extra_search_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let mut search_dirs = extra_search_paths.to_vec();

    if let Ok(cwd) = std::env::current_dir() {
        search_dirs.push(cwd);
    }

    if let Some(home) = std::env::home_dir() {
        search_dirs.push(home.join(".config").join(app_dir_name));
    }

    let local_etc = PathBuf::from(format!("/usr/local/etc/{}", app_dir_name));
    if local_etc.exists() {
        search_dirs.push(local_etc);
    }

    let system_etc = PathBuf::from(format!("/etc/{}", app_dir_name));
    if system_etc.exists() {
        search_dirs.push(system_etc);
    }

    search_dirs
}

pub fn resolve_config_path(
    target: &Path,
    search_dirs: &[PathBuf],
) -> Result<PathBuf, ConfigPathError> {
    // Rule 1: absolute path -> must exist and be a file
    if target.is_absolute() {
        if !target.exists() {
            return Err(ConfigPathError::AbsolutePathNotFound(target.to_path_buf()));
        }
        if !target.is_file() {
            return Err(ConfigPathError::NotAFile(target.to_path_buf()));
        }
        return Ok(target.to_path_buf());
    }

    // Rule 2: relative/filename -> search ordered directories
    for dir in search_dirs {
        let candidate = dir.join(target);
        if candidate.exists() {
            if !candidate.is_file() {
                return Err(ConfigPathError::NotAFile(candidate));
            }
            return Ok(candidate);
        }
    }

    // Not found anywhere
    Err(ConfigPathError::NotFoundInSearchPaths {
        target: target.to_path_buf(),
        searched_paths: search_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
    })
}
