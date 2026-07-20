//! b00t sandbox engine — Linux namespace isolation for runtime wrapper profiles.
//!
//! Implements the declarative [IsolationConfig] from a runtime datum as a real
//! Linux sandbox using unshare(2), mount(2), prctl(2), and execve(2).
//!
//! Construction:
//!   1. Parent pre-resolves binary, argv, CStrings (heap allocation)
//!   2. fork() — child enters sandbox (syscalls only — async-signal-safe)
//!   3. Child: unshare → mount → drop caps → chdir → exec(binary)
//!   4. Parent: waitpid, surface child exit status
//!
//! Safety: All heap operations happen in the parent before fork().
//! The child uses only async-signal-safe syscalls. Errors are communicated
//! via a single-byte code written to the pre-fork pipe.

use crate::{IsolationConfig, RuntimeConfig};
use anyhow::{Result, anyhow, bail};
use std::ffi::CString;

/// Fork, launch sandboxed child, wait, run post-hook.
/// Returns the child's exit status.
pub fn spawn_sandboxed(config: &RuntimeConfig, passthrough_args: &[String]) -> Result<i32> {
    // ═══════ Pre-resolve everything in the PARENT (before fork) ═══════
    let binary = resolve_binary(&config.binary)?;
    let c_binary = CString::new(binary.clone()).map_err(|e| anyhow!("binary: {e}"))?;

    let mut c_args: Vec<CString> = Vec::new();
    c_args.push(c_binary.clone());
    if let Some(ref args) = config.args {
        for a in args {
            c_args.push(CString::new(a.as_str()).map_err(|e| anyhow!("arg: {e}"))?);
        }
    }
    for a in passthrough_args {
        c_args.push(CString::new(a.as_str()).map_err(|e| anyhow!("arg: {e}"))?);
    }

    let iso = config.isolation.clone();
    let cwd = iso
        .as_ref()
        .and_then(|i| i.cwd.clone())
        .map(|s| CString::new(s).expect("cwd has null"));
    let hostname = iso
        .as_ref()
        .and_then(|i| i.hostname.clone())
        .map(|s| CString::new(s).expect("hostname has null"));

    // Build NULL-terminated argv pointer array for execvp (CString is 16B, char* is 8B)
    let mut c_argv_ptrs: Vec<*const libc::c_char> = c_args.iter().map(|s| s.as_ptr()).collect();
    c_argv_ptrs.push(std::ptr::null());

    // Error pipe — child writes a single byte on failure
    let mut pipe_fds = [0i32; 2];
    if unsafe { libc::pipe2(pipe_fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        bail!("pipe2: {}", std::io::Error::last_os_error());
    }

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        bail!("fork: {}", std::io::Error::last_os_error());
    }

    if pid == 0 {
        // ═══════ CHILD — syscalls only, no heap allocation ═══════
        unsafe { libc::close(pipe_fds[0]) };

        let result = child_sandbox(
            &iso,
            cwd.as_deref(),
            hostname.as_deref(),
            &c_binary,
            c_argv_ptrs.as_ptr(),
        );

        let err_byte: u8 = match result {
            Ok(()) => 0, // execvp failed (it doesn't return on success)
            Err(code) => code,
        };
        unsafe {
            libc::write(
                pipe_fds[1],
                &err_byte as *const u8 as *const libc::c_void,
                1,
            )
        };
        let exit = if err_byte == 0 { 126 } else { err_byte as i32 };
        unsafe { libc::_exit(exit) };
    }

    // ═══════ PARENT ═══════
    unsafe { libc::close(pipe_fds[1]) };

    let mut status = 0i32;
    unsafe { libc::waitpid(pid, &mut status, 0) };

    let mut err_byte = [0u8; 1];
    let n = unsafe { libc::read(pipe_fds[0], err_byte.as_mut_ptr() as *mut libc::c_void, 1) };
    unsafe { libc::close(pipe_fds[0]) };

    let exit_code = if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        libc::WTERMSIG(status) + 128
    } else {
        1i32
    };

    if n > 0 && err_byte[0] != 0 {
        eprintln!("[sandbox] child setup error code {}", err_byte[0]);
    }

    Ok(exit_code)
}

// ── Child-only functions — async-signal-safe (no heap, no format, no alloc) ──

fn child_sandbox(
    iso: &Option<IsolationConfig>,
    cwd: Option<&std::ffi::CStr>,
    hostname: Option<&std::ffi::CStr>,
    binary: &CString,
    argv_ptrs: *const *const libc::c_char,
) -> Result<(), u8> {
    if let Some(iso) = iso {
        child_enter_namespaces(iso)?;
        child_mount_root()?;
        if let Some(ref mounts) = iso.mounts {
            child_apply_mounts(mounts)?;
        }
        if let Some(h) = hostname {
            unsafe { libc::sethostname(h.as_ptr(), h.to_bytes().len()) };
        }
        if iso.new_session.unwrap_or(false) {
            unsafe { libc::setsid() };
        }
        child_drop_capabilities(iso);
    }

    if let Some(c) = cwd {
        if unsafe { libc::chdir(c.as_ptr()) } != 0 {
            return Err(3);
        }
    }

    unsafe {
        libc::execvp(binary.as_ptr(), argv_ptrs);
    }

    Ok(())
}

fn child_enter_namespaces(iso: &IsolationConfig) -> Result<(), u8> {
    let mut ns_flags = libc::CLONE_NEWNS;

    if !iso.share_pid.unwrap_or(false) {
        ns_flags |= libc::CLONE_NEWPID;
    }
    if !iso.share_net.unwrap_or(true) {
        ns_flags |= libc::CLONE_NEWNET;
    }
    if !iso.share_ipc.unwrap_or(false) {
        ns_flags |= libc::CLONE_NEWIPC;
    }
    if !iso.share_uts.unwrap_or(false) {
        ns_flags |= libc::CLONE_NEWUTS;
    }

    if unsafe { libc::unshare(ns_flags) } != 0 {
        return Err(1);
    }
    Ok(())
}

fn child_mount_root() -> Result<(), u8> {
    if unsafe {
        libc::mount(
            std::ptr::null(),
            b"/\0".as_ptr() as *const libc::c_char,
            std::ptr::null(),
            libc::MS_REC | libc::MS_PRIVATE,
            std::ptr::null(),
        )
    } != 0
    {
        return Err(2);
    }
    Ok(())
}

fn child_apply_mounts(mounts: &[crate::MountEntry]) -> Result<(), u8> {
    for mnt in mounts {
        let src = CString::new(mnt.src.as_str()).map_err(|_| 4u8)?;
        let dest = CString::new(mnt.dest.as_str()).map_err(|_| 5u8)?;

        match mnt.mount_type.as_str() {
            "tmpfs" => child_bind_mount(b"tmpfs\0", &dest, 0, 6)?,
            "proc" => child_bind_mount(b"proc\0", &dest, 0, 7)?,
            "dev" => child_bind_mount(b"devtmpfs\0", &dest, 0, 8)?,
            "ro-bind" => {
                child_bind_mount(
                    src.as_bytes_with_nul(),
                    &dest,
                    libc::MS_BIND | libc::MS_REC,
                    9,
                )?;
                let ro_flags = libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY;
                if unsafe {
                    libc::mount(
                        std::ptr::null(),
                        dest.as_ptr(),
                        std::ptr::null(),
                        ro_flags,
                        std::ptr::null(),
                    )
                } != 0
                {
                    return Err(9);
                }
            }
            _ => child_bind_mount(
                src.as_bytes_with_nul(),
                &dest,
                libc::MS_BIND | libc::MS_REC,
                10,
            )?,
        }
    }
    Ok(())
}

fn child_bind_mount(
    src: &[u8],
    dest: &CString,
    flags: libc::c_ulong,
    err_code: u8,
) -> Result<(), u8> {
    if unsafe {
        libc::mount(
            src.as_ptr() as *const libc::c_char,
            dest.as_ptr(),
            std::ptr::null(),
            flags,
            std::ptr::null(),
        )
    } != 0
    {
        return Err(err_code);
    }
    Ok(())
}

// ── Linux capability constants ──────────────────────────────────────────

const CAP_CHOWN: u32 = 0;
const CAP_DAC_OVERRIDE: u32 = 1;
const CAP_DAC_READ_SEARCH: u32 = 2;
const CAP_FOWNER: u32 = 3;
const CAP_FSETID: u32 = 4;
const CAP_KILL: u32 = 5;
const CAP_SETGID: u32 = 6;
const CAP_SETUID: u32 = 7;
const CAP_SETPCAP: u32 = 8;
const CAP_NET_BIND_SERVICE: u32 = 10;
const CAP_NET_BROADCAST: u32 = 11;
const CAP_NET_ADMIN: u32 = 12;
const CAP_NET_RAW: u32 = 13;
const CAP_IPC_LOCK: u32 = 14;
const CAP_IPC_OWNER: u32 = 15;
const CAP_SYS_MODULE: u32 = 16;
const CAP_SYS_RAWIO: u32 = 17;
const CAP_SYS_CHROOT: u32 = 18;
const CAP_SYS_PTRACE: u32 = 19;
const CAP_SYS_PACCT: u32 = 20;
const CAP_SYS_ADMIN: u32 = 21;
const CAP_SYS_BOOT: u32 = 22;
const CAP_SYS_NICE: u32 = 23;
const CAP_SYS_RESOURCE: u32 = 24;
const CAP_SYS_TIME: u32 = 25;
const CAP_SYS_TTY_CONFIG: u32 = 26;
const CAP_MKNOD: u32 = 27;
const CAP_LEASE: u32 = 28;
const CAP_AUDIT_WRITE: u32 = 29;
const CAP_AUDIT_CONTROL: u32 = 30;
const CAP_SETFCAP: u32 = 31;
const CAP_MAC_OVERRIDE: u32 = 32;
const CAP_MAC_ADMIN: u32 = 33;
const CAP_SYSLOG: u32 = 34;
const CAP_WAKE_ALARM: u32 = 35;
const CAP_BLOCK_SUSPEND: u32 = 36;
const CAP_AUDIT_READ: u32 = 37;
const CAP_PERFMON: u32 = 38;
const CAP_BPF: u32 = 39;
const CAP_CHECKPOINT_RESTORE: u32 = 40;

static ALL_CAPS: &[(&str, u32)] = &[
    ("CAP_CHOWN", CAP_CHOWN),
    ("CAP_DAC_OVERRIDE", CAP_DAC_OVERRIDE),
    ("CAP_DAC_READ_SEARCH", CAP_DAC_READ_SEARCH),
    ("CAP_FOWNER", CAP_FOWNER),
    ("CAP_FSETID", CAP_FSETID),
    ("CAP_KILL", CAP_KILL),
    ("CAP_SETGID", CAP_SETGID),
    ("CAP_SETUID", CAP_SETUID),
    ("CAP_SETPCAP", CAP_SETPCAP),
    ("CAP_NET_BIND_SERVICE", CAP_NET_BIND_SERVICE),
    ("CAP_NET_BROADCAST", CAP_NET_BROADCAST),
    ("CAP_NET_ADMIN", CAP_NET_ADMIN),
    ("CAP_NET_RAW", CAP_NET_RAW),
    ("CAP_IPC_LOCK", CAP_IPC_LOCK),
    ("CAP_IPC_OWNER", CAP_IPC_OWNER),
    ("CAP_SYS_MODULE", CAP_SYS_MODULE),
    ("CAP_SYS_RAWIO", CAP_SYS_RAWIO),
    ("CAP_SYS_CHROOT", CAP_SYS_CHROOT),
    ("CAP_SYS_PTRACE", CAP_SYS_PTRACE),
    ("CAP_SYS_PACCT", CAP_SYS_PACCT),
    ("CAP_SYS_ADMIN", CAP_SYS_ADMIN),
    ("CAP_SYS_BOOT", CAP_SYS_BOOT),
    ("CAP_SYS_NICE", CAP_SYS_NICE),
    ("CAP_SYS_RESOURCE", CAP_SYS_RESOURCE),
    ("CAP_SYS_TIME", CAP_SYS_TIME),
    ("CAP_SYS_TTY_CONFIG", CAP_SYS_TTY_CONFIG),
    ("CAP_MKNOD", CAP_MKNOD),
    ("CAP_LEASE", CAP_LEASE),
    ("CAP_AUDIT_WRITE", CAP_AUDIT_WRITE),
    ("CAP_AUDIT_CONTROL", CAP_AUDIT_CONTROL),
    ("CAP_SETFCAP", CAP_SETFCAP),
    ("CAP_MAC_OVERRIDE", CAP_MAC_OVERRIDE),
    ("CAP_MAC_ADMIN", CAP_MAC_ADMIN),
    ("CAP_SYSLOG", CAP_SYSLOG),
    ("CAP_WAKE_ALARM", CAP_WAKE_ALARM),
    ("CAP_BLOCK_SUSPEND", CAP_BLOCK_SUSPEND),
    ("CAP_AUDIT_READ", CAP_AUDIT_READ),
    ("CAP_PERFMON", CAP_PERFMON),
    ("CAP_BPF", CAP_BPF),
    ("CAP_CHECKPOINT_RESTORE", CAP_CHECKPOINT_RESTORE),
];

fn child_drop_capabilities(iso: &IsolationConfig) {
    let retain: std::collections::HashSet<&str> = iso
        .caps_retain
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();

    for &(_name, cap_val) in ALL_CAPS {
        if !retain.contains(_name) {
            unsafe {
                libc::prctl(libc::PR_CAPBSET_DROP, cap_val as libc::c_ulong, 0, 0, 0);
            }
        }
    }
}

/// Resolve a binary name to an absolute path (searches PATH).
fn resolve_binary(name: &str) -> Result<String> {
    if name.starts_with('/') || name.starts_with("./") {
        if std::path::Path::new(name).exists() {
            return Ok(name.to_string());
        }
        bail!("binary not found: {name}");
    }
    if let Ok(path) = which::which(name) {
        return Ok(path.to_string_lossy().to_string());
    }
    bail!("binary '{name}' not found in PATH")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_runtime_config(binary: &str) -> RuntimeConfig {
        RuntimeConfig {
            binary: binary.to_string(),
            args: None,
            workdir: None,
            isolation: None,
            hook_pre: None,
            hook_post: None,
        }
    }

    #[test]
    fn test_resolve_binary_absolute_path() {
        let result = resolve_binary("/bin/sh");
        assert!(
            result.is_ok(),
            "resolve_binary(\"/bin/sh\") failed: {:?}",
            result
        );
    }

    #[test]
    fn test_resolve_binary_not_found() {
        let result = resolve_binary("__nonexistent_binary_xyz__");
        assert!(result.is_err(), "expected error for nonexistent binary");
    }

    #[test]
    fn test_resolve_binary_relative() {
        let result = resolve_binary("./nonexistent");
        assert!(
            result.is_err(),
            "expected error for relative nonexistent binary"
        );
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found"),
            "error should mention 'not found': {err}"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_spawn_sandboxed_no_isolation() {
        let config = empty_runtime_config("/bin/true");
        let exit_code = spawn_sandboxed(&config, &[]).expect("spawn_sandboxed should succeed");
        assert_eq!(exit_code, 0, "/bin/true should exit 0, got {exit_code}");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_spawn_sandboxed_binary_not_found() {
        let config = empty_runtime_config("__nonexistent_binary_xyz__");
        let result = spawn_sandboxed(&config, &[]);
        assert!(
            result.is_err(),
            "expected error for nonexistent binary, got: {result:?}"
        );
    }
}
