use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, get_config, check_command_available};
use crate::traits::*;

/// Database datum for managing database connections via DSN (Data Source Name)
pub struct DatabaseDatum {
    pub datum: BootDatum,
}

impl DatabaseDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(DatabaseDatum {
            datum: config.b00t,
        })
    }

    /// Parse DSN to extract database type
    fn get_db_type(&self) -> Option<String> {
        if let Some(dsn) = &self.datum.dsn {
            // Extract scheme from DSN (postgres://, mysql://, sqlite://, etc.)
            if let Some(scheme_end) = dsn.find("://") {
                return Some(dsn[..scheme_end].to_lowercase());
            }
        }
        None
    }

    /// Check if database is reachable
    fn is_reachable(&self) -> bool {
        if let Some(dsn) = &self.datum.dsn {
            match self.get_db_type().as_deref() {
                Some("postgres") | Some("postgresql") => {
                    // Use psql to test connection
                    let result = cmd!("psql", dsn, "-c", "SELECT 1;").run();
                    result.is_ok()
                }
                Some("mysql") => {
                    // Use mysql to test connection
                    let result = cmd!("mysql", "--execute", "SELECT 1;", dsn).run();
                    result.is_ok()
                }
                Some("sqlite") => {
                    // For SQLite, check if file exists and is readable
                    if let Some(path) = dsn.strip_prefix("sqlite://") {
                        std::path::Path::new(path).exists()
                    } else {
                        false
                    }
                }
                Some("redis") => {
                    // Use redis-cli to test connection
                    if let Ok(url) = url::Url::parse(dsn) {
                        let host = url.host_str().unwrap_or("localhost");
                        let port = url.port().unwrap_or(6379);
                        let result = cmd!("redis-cli", "-h", host, "-p", &port.to_string(), "ping").read();
                        result.map(|output| output.trim() == "PONG").unwrap_or(false)
                    } else {
                        false
                    }
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Get database version if possible
    fn get_db_version(&self) -> Option<String> {
        if let Some(dsn) = &self.datum.dsn {
            match self.get_db_type().as_deref() {
                Some("postgres") | Some("postgresql") => {
                    let result = cmd!("psql", dsn, "-t", "-c", "SELECT version();").read();
                    result.ok().map(|v| v.trim().to_string())
                }
                Some("mysql") => {
                    let result = cmd!("mysql", "--execute", "SELECT VERSION();", dsn).read();
                    result.ok().map(|v| v.trim().to_string())
                }
                Some("sqlite") => {
                    let result = cmd!("sqlite3", "--version").read();
                    result.ok().map(|v| v.split_whitespace().next().unwrap_or("unknown").to_string())
                }
                Some("redis") => {
                    if let Ok(url) = url::Url::parse(dsn) {
                        let host = url.host_str().unwrap_or("localhost");
                        let port = url.port().unwrap_or(6379);
                        let result = cmd!("redis-cli", "-h", host, "-p", &port.to_string(), "info", "server").read();
                        result.ok().and_then(|output| {
                            for line in output.lines() {
                                if line.starts_with("redis_version:") {
                                    return Some(line.split(':').nth(1)?.to_string());
                                }
                            }
                            None
                        })
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }

    /// Check if required client tools are available
    fn client_available(&self) -> bool {
        match self.get_db_type().as_deref() {
            Some("postgres") | Some("postgresql") => check_command_available("psql"),
            Some("mysql") => check_command_available("mysql"),
            Some("sqlite") => check_command_available("sqlite3"),
            Some("redis") => check_command_available("redis-cli"),
            _ => false,
        }
    }
}

impl TryFrom<(&str, &str)> for DatabaseDatum {
    type Error = anyhow::Error;
    
    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for DatabaseDatum {
    fn is_installed(&self) -> bool {
        // Database is "installed" if it's reachable and client tools are available
        self.client_available() && self.is_reachable()
    }
    
    fn current_version(&self) -> Option<String> {
        if self.is_reachable() {
            self.get_db_version()
        } else {
            None
        }
    }
    
    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }
    
    fn version_status(&self) -> VersionStatus {
        if !self.client_available() {
            return VersionStatus::Missing;
        }
        
        if !self.is_reachable() {
            return VersionStatus::Missing;
        }
        
        // For databases, we primarily care about reachability
        // Version comparison can be complex and database-specific
        VersionStatus::Unknown
    }
}

impl StatusProvider for DatabaseDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }
    
    fn subsystem(&self) -> &str {
        "database"
    }
    
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    
    fn is_disabled(&self) -> bool {
        // Database is disabled if no DSN is provided or client tools unavailable
        self.datum.dsn.is_none() || !self.client_available()
    }
}

impl FilterLogic for DatabaseDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }
    
    fn prerequisites_satisfied(&self) -> bool {
        // Check if require constraints are satisfied
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            // Default: DSN must be provided and client tools available
            self.datum.dsn.is_some() && self.client_available()
        }
    }
    
    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for DatabaseDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for DatabaseDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumCreator for DatabaseDatum {
    fn create_interactive(name: &str, _path: &str) -> Result<BootDatum> {
        use duct::cmd;
        
        println!("Creating database configuration for: {}", name);
        
        // Select database type
        let db_type_result = cmd!("bash", "-c", 
            "echo -e 'postgresql\\nmysql\\nsqlite\\nredis\\nmongodb' | fzf --prompt='Database Type: ' --header='Select database type'"
        ).read();
        let db_type = db_type_result.unwrap_or_else(|_| "postgresql".to_string()).trim().to_string();
        
        // Build DSN based on database type
        let dsn = match db_type.as_str() {
            "postgresql" => {
                let host_result = cmd!("bash", "-c", 
                    "echo 'localhost' | fzf --print-query --prompt='Host: ' --header='Database host'"
                ).read();
                let host = host_result.unwrap_or_else(|_| "localhost".to_string()).trim().to_string();
                
                let port_result = cmd!("bash", "-c", 
                    "echo '5432' | fzf --print-query --prompt='Port: ' --header='Database port'"
                ).read();
                let port = port_result.unwrap_or_else(|_| "5432".to_string()).trim().to_string();
                
                let user_result = cmd!("bash", "-c", 
                    "echo 'postgres' | fzf --print-query --prompt='Username: ' --header='Database username'"
                ).read();
                let user = user_result.unwrap_or_else(|_| "postgres".to_string()).trim().to_string();
                
                let pass_result = cmd!("bash", "-c", 
                    "echo '' | fzf --print-query --prompt='Password: ' --header='Database password (leave empty for env var)'"
                ).read();
                let pass = pass_result.unwrap_or_default().trim().to_string();
                
                let db_result = cmd!("bash", "-c", &format!(
                    "echo '{}' | fzf --print-query --prompt='Database: ' --header='Database name'",
                    name
                )).read();
                let database = db_result.unwrap_or_else(|_| name.to_string()).trim().to_string();
                
                if pass.is_empty() {
                    format!("postgres://{}@{}:{}/{}", user, host, port, database)
                } else {
                    format!("postgres://{}:{}@{}:{}/{}", user, pass, host, port, database)
                }
            },
            "mysql" => {
                let host_result = cmd!("bash", "-c", 
                    "echo 'localhost' | fzf --print-query --prompt='Host: '"
                ).read();
                let host = host_result.unwrap_or_else(|_| "localhost".to_string()).trim().to_string();
                
                let user_result = cmd!("bash", "-c", 
                    "echo 'root' | fzf --print-query --prompt='Username: '"
                ).read();
                let user = user_result.unwrap_or_else(|_| "root".to_string()).trim().to_string();
                
                format!("mysql://{}@{}/{}", user, host, name)
            },
            "sqlite" => {
                let path_result = cmd!("bash", "-c", &format!(
                    "echo './{}.db' | fzf --print-query --prompt='File Path: ' --header='SQLite database file path'",
                    name
                )).read();
                let path = path_result.unwrap_or_else(|_| format!("./{}.db", name)).trim().to_string();
                
                format!("sqlite://{}", path)
            },
            "redis" => {
                let host_result = cmd!("bash", "-c", 
                    "echo 'localhost' | fzf --print-query --prompt='Host: '"
                ).read();
                let host = host_result.unwrap_or_else(|_| "localhost".to_string()).trim().to_string();
                
                let port_result = cmd!("bash", "-c", 
                    "echo '6379' | fzf --print-query --prompt='Port: '"
                ).read();
                let port = port_result.unwrap_or_else(|_| "6379".to_string()).trim().to_string();
                
                format!("redis://{}:{}/0", host, port)
            },
            _ => {
                format!("{}://localhost/{}", db_type, name)
            }
        };
        
        // Get hint
        let hint_result = cmd!("bash", "-c", &format!(
            "echo '{} database: {}' | fzf --print-query --prompt='Description: ' --header='Enter description/hint'",
            db_type, name
        )).read();
        let hint = hint_result.unwrap_or_else(|_| format!("{} database: {}", db_type, name)).trim().to_string();
        
        Ok(BootDatum {
            name: name.to_string(),
            datum_type: Some(crate::DatumType::Database),
            desires: None,
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
            url: None,
            branch: None,
            clone_path: None,
            dsn: Some(dsn),
        })
    }
    
    fn file_extension() -> &'static str {
        ".db.toml"
    }
    
    fn type_name() -> &'static str {
        "database"
    }
}