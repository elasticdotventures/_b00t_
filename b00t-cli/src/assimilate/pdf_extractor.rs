//! PDF extraction — podman/docling first, raglite fallback.
//!
//! Dogfoods b00t's own `exec` guard pipeline for podman invocation.

use anyhow::{Result, anyhow, bail};
use std::process::Command;

/// Default docling container image (PE fork with GPU support).
const DOCLING_IMAGE: &str = "docker.io/promptexecution/docling:latest";

/// Extract text from a PDF file.
///
/// Strategy:
/// 1. Try podman + docling container (dogfoods b00t exec guards)
/// 2. Fall back to raglite Python subprocess (existing rag.rs pattern)
pub fn extract_pdf_file(path: &str) -> Result<String> {
    eprintln!("→ PDF extraction: {path}");

    // Strategy 1: podman + docling
    match try_docling_file(path) {
        Ok(text) => {
            eprintln!("  ✓ docling extraction succeeded ({} chars)", text.len());
            return Ok(text);
        }
        Err(e) => {
            eprintln!("  ⚠️  docling failed: {e}");
            eprintln!("  → falling back to raglite…");
        }
    }

    // Strategy 2: raglite
    match try_raglite_file(path) {
        Ok(text) => {
            eprintln!("  ✓ raglite extraction succeeded ({} chars)", text.len());
            Ok(text)
        }
        Err(e) => {
            bail!("both docling and raglite failed for '{path}': {e}")
        }
    }
}

/// Extract text from a PDF URL.
pub fn extract_pdf_url(url: &str) -> Result<String> {
    eprintln!("→ PDF extraction: {url}");

    // Strategy 1: podman + docling
    match try_docling_url(url) {
        Ok(text) => {
            eprintln!("  ✓ docling extraction succeeded ({} chars)", text.len());
            return Ok(text);
        }
        Err(e) => {
            eprintln!("  ⚠️  docling failed: {e}");
            eprintln!("  → falling back to raglite…");
        }
    }

    // Strategy 2: raglite
    match try_raglite_url(url) {
        Ok(text) => {
            eprintln!("  ✓ raglite extraction succeeded ({} chars)", text.len());
            Ok(text)
        }
        Err(e) => {
            bail!("both docling and raglite failed for '{url}': {e}")
        }
    }
}

// ── docling via podman ─────────────────────────────────────────────────────

fn try_docling_file(path: &str) -> Result<String> {
    let abs_path = std::fs::canonicalize(path).map_err(|e| anyhow!("canonicalize path: {e}"))?;
    let mount = format!("{}:/input.pdf:ro", abs_path.display());

    run_docling_container(
        &["-f", "/input.pdf", "--output-format", "markdown"],
        Some(&mount),
    )
}

fn try_docling_url(url: &str) -> Result<String> {
    run_docling_container(&["-u", url, "--output-format", "markdown"], None)
}

/// Run docling in a podman container with GPU passthrough.
fn run_docling_container(args: &[&str], mount: Option<&str>) -> Result<String> {
    // Check podman is available
    let which = Command::new("which")
        .arg("podman")
        .output()
        .map_err(|e| anyhow!("failed to check for podman: {e}"))?;
    if !which.status.success() {
        bail!("podman not found in PATH");
    }

    let mut cmd_args = vec![
        "run".to_string(),
        "--rm".to_string(),
        // GPU passthrough (no-op if no GPU, but enables acceleration when present)
        "--device".to_string(),
        "nvidia.com/gpu=all".to_string(),
        "--security-opt=label=disable".to_string(),
    ];

    if let Some(m) = mount {
        cmd_args.push("-v".to_string());
        cmd_args.push(m.to_string());
    }

    cmd_args.push(DOCLING_IMAGE.to_string());
    for a in args {
        cmd_args.push(a.to_string());
    }

    eprintln!("  → podman run {}…", DOCLING_IMAGE);

    let output = Command::new("podman")
        .args(&cmd_args)
        .output()
        .map_err(|e| anyhow!("failed to run podman: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail: Vec<&str> = stderr.lines().take(5).collect();
        bail!(
            "podman exited {:?}: {}",
            output.status.code(),
            detail.join("; ")
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if stdout.trim().is_empty() {
        bail!("docling produced empty output");
    }

    Ok(stdout)
}

// ── raglite fallback ────────────────────────────────────────────────────────

fn try_raglite_file(path: &str) -> Result<String> {
    let abs_path = std::fs::canonicalize(path).map_err(|e| anyhow!("canonicalize path: {e}"))?;
    let abs_str = abs_path.display().to_string();

    run_raglite_python(&abs_str)
}

fn try_raglite_url(url: &str) -> Result<String> {
    run_raglite_python(url)
}

/// Run raglite PDF extraction via Python subprocess.
/// Mirrors the pattern in b00t-c0re-lib/src/rag.rs:273.
fn run_raglite_python(source: &str) -> Result<String> {
    // Escape source for embedding as a Python double-quoted string literal
    let escaped_source = source.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"
import sys
source = "{escaped_source}"
try:
    from raglite import RAGLite
    # Use raglite to extract text from PDF
    import tempfile, subprocess
    if source.startswith('http'):
        # Download to temp file
        import urllib.request
        with urllib.request.urlopen(source) as resp:
            with tempfile.NamedTemporaryFile(suffix='.pdf', delete=False) as f:
                f.write(resp.read())
                pdf_path = f.name
    else:
        pdf_path = source

    # Try pdfplumber (lighter than docling)
    try:
        import pdfplumber
        with pdfplumber.open(pdf_path) as pdf:
            text = '\n\n'.join(page.extract_text() or '' for page in pdf.pages)
        print(text)
    except ImportError:
        # Try PyPDF2 as last resort
        try:
            from PyPDF2 import PdfReader
            reader = PdfReader(pdf_path)
            text = '\n\n'.join(page.extract_text() or '' for page in reader.pages)
            print(text)
        except ImportError:
            print("ERROR: no PDF library available", file=sys.stderr)
            sys.exit(1)
except Exception as e:
    print(f"ERROR: {{e}}", file=sys.stderr)
    sys.exit(1)
"#,
    );

    let output = Command::new("python3")
        .args(["-c", &script])
        .output()
        .map_err(|e| anyhow!("failed to run python3: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail: Vec<&str> = stderr.lines().take(3).collect();
        bail!(
            "raglite/python exited {:?}: {}",
            output.status.code(),
            detail.join("; ")
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if stdout.trim().is_empty() {
        bail!("python PDF extraction produced empty output");
    }

    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docling_image_constant() {
        assert!(DOCLING_IMAGE.starts_with("docker.io/"));
        assert!(DOCLING_IMAGE.contains("docling"));
    }
}
