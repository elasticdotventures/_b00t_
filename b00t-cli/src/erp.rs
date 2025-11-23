use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::path::PathBuf;

/// SERPAH TOGAF process building block -> t00n-ready representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErpProcess {
    pub name: String,
    pub phase: String,          // TOGAF ADM phase
    pub description: String,
    pub actors: Vec<String>,
    pub roles: Vec<String>,
    pub skills: Vec<String>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub flows: Vec<String>,     // logical flows/ports
}

impl ErpProcess {
    /// Serialize to minimal t00n text (token-lean, human-friendly)
    pub fn to_toon(&self) -> String {
        let mut toon = String::new();
        toon.push_str("format = \"t00n\"\n\n");
        toon.push_str("[meta]\n");
        toon.push_str(&format!("name = \"{}\"\n", self.name));
        toon.push_str(&format!("phase = \"{}\"\n", self.phase));
        toon.push_str("layer = \"logical\"\n");
        toon.push_str("\n[process]\n");
        toon.push_str(&format!("description = \"{}\"\n", self.description));
        toon.push_str(&format!("actors = {:?}\n", self.actors));
        toon.push_str(&format!("roles = {:?}\n", self.roles));
        toon.push_str(&format!("skills = {:?}\n", self.skills));
        toon.push_str(&format!("inputs = {:?}\n", self.inputs));
        toon.push_str(&format!("outputs = {:?}\n", self.outputs));
        toon.push_str(&format!("flows = {:?}\n", self.flows));
        toon
    }

    /// Write t00n to a path (stub—no append/merge logic yet)
    pub fn write_toon<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        fs::write(path, self.to_toon())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_toon_roundtrip_stub() {
        let proc = ErpProcess {
            name: "capability-planning".to_string(),
            phase: "Phase B".to_string(),
            description: "Map enterprise capabilities to roles/skills for semantic workflow edges".to_string(),
            actors: vec!["executive".to_string()],
            roles: vec!["delegate".to_string()],
            skills: vec!["delegation".to_string(), "compliance".to_string()],
            inputs: vec!["business_goal".to_string()],
            outputs: vec!["semantic_workflow_spec".to_string()],
            flows: vec!["goal->workflow".to_string()],
        };

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("process.t00n");
        proc.write_toon(&path).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("format = \"t00n\""));
        assert!(contents.contains("capability-planning"));
        assert!(contents.contains("delegation"));
    }
}

/// Minimal interface for sm0l-compatible queues used by ERP processes.
pub trait SmolQueue {
    fn enqueue(&self, msg: &str) -> Result<()>;
    fn try_dequeue(&self) -> Result<Option<String>>;
    fn path(&self) -> &Path;
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn write_line(path: &Path, line: &str) -> Result<()> {
    ensure_parent(path)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}

fn pop_line(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let file = OpenOptions::new().read(true).open(path)?;
    let mut reader = BufReader::new(file);
    let mut first = String::new();
    let bytes = reader.read_line(&mut first)?;
    if bytes == 0 {
        return Ok(None);
    }
    let rest: Vec<String> = reader
        .lines()
        .filter_map(|l| l.ok())
        .collect();
    // Rewrite remaining lines
    let mut tmp = tempfile::NamedTempFile::new_in(
        path.parent().unwrap_or_else(|| Path::new(".")),
    )?;
    for line in rest {
        writeln!(tmp, "{}", line)?;
    }
    tmp.persist(path)?;
    Ok(Some(first.trim_end_matches('\n').to_string()))
}

/// File-backed queue for FIFO semantics (lightweight stub for sm0l minion ingestion).
pub struct FifoQueue {
    path: PathBuf,
}

impl FifoQueue {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self { path: path.into() }
    }
}

impl SmolQueue for FifoQueue {
    fn enqueue(&self, msg: &str) -> Result<()> {
        write_line(&self.path, msg)
    }

    fn try_dequeue(&self) -> Result<Option<String>> {
        pop_line(&self.path)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

/// Socat-style UNIX socket queue (stubbed with file backing for portability).
pub struct SocatQueue {
    path: PathBuf,
}

impl SocatQueue {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self { path: path.into() }
    }
}

impl SmolQueue for SocatQueue {
    fn enqueue(&self, msg: &str) -> Result<()> {
        write_line(&self.path, msg)
    }

    fn try_dequeue(&self) -> Result<Option<String>> {
        pop_line(&self.path)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

/// Tempfile/lock-based queue (fallback for environments without FIFOs or sockets).
pub struct TempfileQueue {
    path: PathBuf,
}

impl TempfileQueue {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        Self { path: path.into() }
    }
}

impl SmolQueue for TempfileQueue {
    fn enqueue(&self, msg: &str) -> Result<()> {
        write_line(&self.path, msg)
    }

    fn try_dequeue(&self) -> Result<Option<String>> {
        pop_line(&self.path)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;
    use tempfile::TempDir;

    fn exercise_queue<Q: SmolQueue>(q: &Q) {
        q.enqueue("a").unwrap();
        q.enqueue("b").unwrap();
        assert_eq!(q.try_dequeue().unwrap(), Some("a".to_string()));
        assert_eq!(q.try_dequeue().unwrap(), Some("b".to_string()));
        assert_eq!(q.try_dequeue().unwrap(), None);
    }

    #[test]
    fn test_fifo_queue() {
        let tmp = TempDir::new().unwrap();
        let q = FifoQueue::new(tmp.path().join("fifo.queue"));
        exercise_queue(&q);
    }

    #[test]
    fn test_socat_queue() {
        let tmp = TempDir::new().unwrap();
        let q = SocatQueue::new(tmp.path().join("socat.queue"));
        exercise_queue(&q);
    }

    #[test]
    fn test_tempfile_queue() {
        let tmp = TempDir::new().unwrap();
        let q = TempfileQueue::new(tmp.path().join("tmp.queue"));
        exercise_queue(&q);
    }
}
