use anyhow::{Context, Result};
use fs2::FileExt;
use std::fs::{read_dir, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use uuid::Uuid;

/// Minimal queue trait for sm0l workers that need a local IPC hop.
pub trait SmolQueue {
    fn name(&self) -> &'static str;
    fn send(&self, payload: &str) -> Result<()>;
    fn try_recv(&self) -> Result<Option<String>>;
}

/// File-backed queue that appends lines to a single file.
/// 
/// Uses exclusive file locking (via fs2::FileExt) to prevent race conditions
/// in concurrent producer/consumer scenarios. Both send() and try_recv() use
/// try_lock_exclusive(), which fails immediately if the lock cannot be acquired,
/// allowing callers to implement their own retry logic as needed.
pub struct BashLineQueue {
    path: PathBuf,
}

impl BashLineQueue {
    pub fn new<P: Into<PathBuf>>(path: P) -> Result<Self> {
        let path = path.into();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).context("create bash-line queue dir")?;
        }
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .context("init bash-line queue file")?;
        Ok(Self { path })
    }
}

impl SmolQueue for BashLineQueue {
    fn name(&self) -> &'static str {
        "bash-line"
    }

    fn send(&self, payload: &str) -> Result<()> {
        let mut fh = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .context("open bash-line queue for append")?;
        
        // Acquire exclusive lock to prevent race conditions during append
        fh.try_lock_exclusive()
            .context("failed to acquire exclusive lock on bash-line queue")?;
        
        writeln!(fh, "{}", payload).context("write bash-line payload")?;
        fh.flush().context("flush bash-line payload")?;
        
        // Lock is released when fh is dropped
        Ok(())
    }

    fn try_recv(&self) -> Result<Option<String>> {
        // Open with read+write to allow locking and modification
        let mut fh = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&self.path)
            .context("open bash-line queue for read+write")?;
        
        // Acquire exclusive lock to prevent race conditions
        fh.try_lock_exclusive()
            .context("failed to acquire exclusive lock on bash-line queue")?;
        
        // Read the first line and remaining content
        let (first, remaining) = {
            let mut reader = BufReader::new(&fh);
            let mut first = String::new();
            let count = reader.read_line(&mut first).context("read bash-line message")?;
            if count == 0 {
                // Release lock automatically when fh is dropped
                return Ok(None);
            }
            
            // Read remainder to keep queue behaviour
            let mut remaining = String::new();
            reader.read_to_string(&mut remaining).context("read remaining bash-line queue")?;
            (first, remaining)
        }; // BufReader is dropped here, releasing the borrow
        
        // Truncate and rewrite with the lock still held
        fh.set_len(0).context("truncate bash-line queue")?;
        // set_len doesn't change file position, so seek to start before writing
        fh.seek(std::io::SeekFrom::Start(0)).context("seek to start of bash-line queue")?;
        fh.write_all(remaining.as_bytes()).context("write remaining bash-line queue")?;
        fh.flush().context("flush bash-line queue")?;
        
        // Lock is released when fh is dropped
        Ok(Some(first.trim_end_matches('\n').to_string()))
    }
}

/// Tempfile chain queue: each message is a separate file.
pub struct TempfileChainQueue {
    dir: PathBuf,
}

impl TempfileChainQueue {
    pub fn new<P: Into<PathBuf>>(dir: P) -> Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).context("create tempfile-chain dir")?;
        Ok(Self { dir })
    }
}

impl SmolQueue for TempfileChainQueue {
    fn name(&self) -> &'static str {
        "tempfile-chain"
    }

    fn send(&self, payload: &str) -> Result<()> {
        let file = self
            .dir
            .join(format!("{}.msg", Uuid::new_v4().as_hyphenated()));
        let mut fh = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&file)
            .with_context(|| format!("open tempfile-chain file {:?}", file))?;
        fh.write_all(payload.as_bytes())
            .context("write tempfile-chain payload")?;
        fh.flush().context("flush tempfile-chain payload")?;
        Ok(())
    }

    fn try_recv(&self) -> Result<Option<String>> {
        let entries = read_dir(&self.dir).with_context(|| format!("scan {:?}", self.dir))?;
        let mut oldest: Option<PathBuf> = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                oldest = match oldest {
                    None => Some(path),
                    Some(current) => {
                        let cur_meta = std::fs::metadata(&current).ok();
                        let new_meta = std::fs::metadata(&path).ok();
                        match (cur_meta.and_then(|m| m.modified().ok()), new_meta.and_then(|m| m.modified().ok())) {
                            (Some(cur_time), Some(new_time)) if new_time < cur_time => Some(path),
                            _ => Some(current),
                        }
                    }
                };
            }
        }
        let Some(target) = oldest else {
            return Ok(None);
        };
        let content = std::fs::read_to_string(&target)
            .with_context(|| format!("read {:?}", target))?;
        let _ = std::fs::remove_file(&target);
        Ok(Some(content))
    }
}

/// socat-based queue for hosts with sockets enabled.
pub struct SocatQueue {
    target: String,
}


impl SocatQueue {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
        }
    }

    fn socat_available() -> bool {
        Command::new("socat")
            .arg("-V")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

impl SmolQueue for SocatQueue {
    fn name(&self) -> &'static str {
        "socat"
    }

    fn send(&self, payload: &str) -> Result<()> {
        if !Self::socat_available() {
            anyhow::bail!("socat not available");
        }
        let mut child = Command::new("socat")
            .arg("-u")
            .arg("-")
            .arg(&self.target)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .with_context(|| format!("launch socat to {}", self.target))?;
        let stdin = child.stdin.as_mut().context("stdin not available")?;
        stdin
            .write_all(payload.as_bytes())
            .context("write payload to socat stdin")?;
        let status = child.wait().context("wait for socat")?;
        if !status.success() {
            anyhow::bail!("socat exited with {}", status);
        }
        Ok(())
    }

    fn try_recv(&self) -> Result<Option<String>> {
        if !Self::socat_available() {
            return Ok(None);
        }
        let output = Command::new("socat")
            .arg("-u")
            .arg(&self.target)
            .arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("launch socat read {}", self.target))?
            .wait_with_output()
            .context("wait for socat read")?;
        if output.stdout.is_empty() {
            return Ok(None);
        }
        let line = String::from_utf8_lossy(&output.stdout);
        Ok(Some(line.trim().to_string()))
    }
}

/// Wrapper that picks the best available backend on the current host.
pub enum QueueBackend {
    Socat(SocatQueue),
    Bash(BashLineQueue),
    Tempfile(TempfileChainQueue),
}

impl QueueBackend {
    pub fn auto(default_dir: &Path) -> Result<Self> {
        if SocatQueue::socat_available() {
            return Ok(Self::Socat(SocatQueue::new("TCP:127.0.0.1:4222")));
        }
        if default_dir.exists() {
            return Ok(Self::Tempfile(TempfileChainQueue::new(default_dir)?));
        }
        Ok(Self::Bash(BashLineQueue::new(default_dir.join("sm0l.queue.log"))?))
    }
}

impl SmolQueue for QueueBackend {
    fn name(&self) -> &'static str {
        match self {
            QueueBackend::Socat(_) => "socat",
            QueueBackend::Bash(_) => "bash-line",
            QueueBackend::Tempfile(_) => "tempfile-chain",
        }
    }

    fn send(&self, payload: &str) -> Result<()> {
        match self {
            QueueBackend::Socat(q) => q.send(payload),
            QueueBackend::Bash(q) => q.send(payload),
            QueueBackend::Tempfile(q) => q.send(payload),
        }
    }

    fn try_recv(&self) -> Result<Option<String>> {
        match self {
            QueueBackend::Socat(q) => q.try_recv(),
            QueueBackend::Bash(q) => q.try_recv(),
            QueueBackend::Tempfile(q) => q.try_recv(),
        }
    }
}
