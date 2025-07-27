use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, get_config, check_command_available};
use crate::traits::*;

/// Repository datum for managing git repositories and version control systems
pub struct RepoDatum {
    pub datum: BootDatum,
}

impl RepoDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(RepoDatum {
            datum: config.b00t,
        })
    }

    /// Check if repository is cloned at the specified path
    fn is_cloned(&self) -> bool {
        if let Some(clone_path) = &self.datum.clone_path {
            std::path::Path::new(clone_path).exists() && 
            std::path::Path::new(&format!("{}/.git", clone_path)).exists()
        } else {
            false
        }
    }

    /// Get current branch of cloned repository
    fn current_branch(&self) -> Option<String> {
        if let Some(clone_path) = &self.datum.clone_path {
            if self.is_cloned() {
                let result = cmd!("git", "-C", clone_path, "branch", "--show-current").read();
                result.ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get latest commit hash
    fn current_commit(&self) -> Option<String> {
        if let Some(clone_path) = &self.datum.clone_path {
            if self.is_cloned() {
                let result = cmd!("git", "-C", clone_path, "rev-parse", "HEAD").read();
                result.ok().map(|s| s.trim()[..8].to_string()) // Short hash
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl TryFrom<(&str, &str)> for RepoDatum {
    type Error = anyhow::Error;
    
    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for RepoDatum {
    fn is_installed(&self) -> bool {
        // Repository is "installed" if it's cloned locally
        self.is_cloned()
    }
    
    fn current_version(&self) -> Option<String> {
        if let Some(branch) = self.current_branch() {
            if let Some(commit) = self.current_commit() {
                Some(format!("{}@{}", branch, commit))
            } else {
                Some(branch)
            }
        } else {
            None
        }
    }
    
    fn desired_version(&self) -> Option<String> {
        // Use branch field or fallback to desires
        self.datum.branch.clone().or_else(|| self.datum.desires.clone())
    }
    
    fn version_status(&self) -> VersionStatus {
        if !check_command_available("git") {
            return VersionStatus::Missing;
        }
        
        if !self.is_cloned() {
            return VersionStatus::Missing;
        }
        
        // Check if we're on the desired branch
        if let Some(desired_branch) = self.desired_version() {
            if let Some(current_branch) = self.current_branch() {
                if current_branch == desired_branch {
                    VersionStatus::Match
                } else {
                    VersionStatus::Older
                }
            } else {
                VersionStatus::Unknown
            }
        } else {
            VersionStatus::Unknown
        }
    }
}

impl StatusProvider for RepoDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }
    
    fn subsystem(&self) -> &str {
        "repo"
    }
    
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    
    fn is_disabled(&self) -> bool {
        // Repositories are disabled if git is not available
        !check_command_available("git")
    }
}

impl FilterLogic for RepoDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }
    
    fn prerequisites_satisfied(&self) -> bool {
        // Check if require constraints are satisfied
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            // Default: git must be available and URL must be provided
            check_command_available("git") && self.datum.url.is_some()
        }
    }
    
    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for RepoDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for RepoDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumCreator for RepoDatum {
    fn create_interactive(name: &str, _path: &str) -> Result<BootDatum> {
        use duct::cmd;
        
        // Use fzf for interactive input
        println!("Creating repository configuration for: {}", name);
        
        // Get repository URL
        let url_prompt = format!("Repository URL for {}:", name);
        let url_result = cmd!("bash", "-c", &format!(
            "echo '' | fzf --print-query --prompt='{} ' --header='Enter git repository URL (https://github.com/org/repo.git)'",
            url_prompt
        )).read();
        let url = url_result.unwrap_or_default().trim().to_string();
        
        // Get branch (default to main)
        let branch_result = cmd!("bash", "-c", 
            "echo -e 'main\\ndevelop\\nmaster' | fzf --prompt='Branch: ' --header='Select or enter branch name'"
        ).read();
        let branch = branch_result.unwrap_or_else(|_| "main".to_string()).trim().to_string();
        
        // Get clone path
        let clone_path_prompt = format!("Clone path for {}:", name);
        let default_path = format!("~/repos/{}", name);
        let clone_path_result = cmd!("bash", "-c", &format!(
            "echo '{}' | fzf --print-query --prompt='{} ' --header='Enter local clone path'",
            default_path, clone_path_prompt
        )).read();
        let clone_path = clone_path_result.unwrap_or(default_path).trim().to_string();
        
        // Get hint
        let hint_result = cmd!("bash", "-c", &format!(
            "echo 'Git repository: {}' | fzf --print-query --prompt='Description: ' --header='Enter description/hint'",
            name
        )).read();
        let hint = hint_result.unwrap_or_else(|_| format!("Git repository: {}", name)).trim().to_string();
        
        Ok(BootDatum {
            name: name.to_string(),
            datum_type: Some(crate::DatumType::Repo),
            desires: Some(branch.clone()),
            hint,
            install: None,
            update: None,
            version: None,
            version_regex: None,
            command: None,
            args: None,
            vsix_id: None,
            script: None,
            image: None,
            docker_args: None,
            package_name: None,
            env: None,
            require: None,
            aliases: None,
            url: Some(url),
            branch: Some(branch),
            clone_path: Some(clone_path),
            dsn: None,
        })
    }
    
    fn file_extension() -> &'static str {
        ".repo.toml"
    }
    
    fn type_name() -> &'static str {
        "repository"
    }
}