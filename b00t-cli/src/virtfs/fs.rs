//! `fuser::Filesystem` glue for the virtfs Phase 1 skeleton.
//! Thin adapter over `super::tree`'s pure directory-tree logic — this
//! module only translates that logic into fuser's Request/Reply protocol
//! and handles the actual mount.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyDirectory, ReplyEntry, Request,
};
use libc::ENOENT;

use super::tree::{self, ROOT_INO};

const TTL: Duration = Duration::from_secs(1);

pub struct B00tFS;

impl B00tFS {
    fn attr_for(ino: u64) -> Option<FileAttr> {
        if !tree::is_directory(ino) {
            return None;
        }
        let now = SystemTime::now();
        Some(FileAttr {
            ino,
            size: 0,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::Directory,
            perm: 0o555,
            nlink: 2,
            // SAFETY: getuid/getgid are always-succeeding syscalls with no
            // preconditions — this is the same pattern the fuser crate's
            // own examples use to report mount-point ownership.
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0,
            blksize: 512,
            flags: 0,
        })
    }
}

impl Filesystem for B00tFS {
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let Some(name) = name.to_str() else {
            reply.error(ENOENT);
            return;
        };
        match tree::lookup_child(parent, name).and_then(Self::attr_for) {
            Some(attr) => reply.entry(&TTL, &attr, 0),
            None => reply.error(ENOENT),
        }
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        match Self::attr_for(ino) {
            Some(attr) => reply.attr(&TTL, &attr),
            None => reply.error(ENOENT),
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let Some(entries) = tree::entries_for(ino) else {
            reply.error(ENOENT);
            return;
        };

        // Phase 1's tree is exactly two levels deep, so every directory's
        // parent (for "..") is root — root's own parent is itself.
        let mut listing: Vec<(u64, FileType, String)> = vec![
            (ino, FileType::Directory, ".".to_string()),
            (ROOT_INO, FileType::Directory, "..".to_string()),
        ];
        for entry in entries {
            listing.push((entry.ino, FileType::Directory, entry.name.to_string()));
        }

        for (index, (child_ino, kind, name)) in listing.into_iter().enumerate().skip(offset as usize) {
            // reply.add returns true when the kernel's buffer is full —
            // stop early rather than keep filling a reply it'll discard.
            if reply.add(child_ino, (index + 1) as i64, kind, name) {
                break;
            }
        }
        reply.ok();
    }
}

/// Mount the virtfs skeleton at `mount_point` (default `~/.claude/b00t`),
/// blocking the calling thread until unmounted (Ctrl-C, or
/// `fusermount3 -u <mount_point>` from another shell).
pub fn mount(mount_point: Option<String>) -> Result<()> {
    let mount_point = match mount_point {
        Some(p) => PathBuf::from(p),
        None => dirs::home_dir()
            .context("could not determine home directory")?
            .join(".claude/b00t"),
    };

    std::fs::create_dir_all(&mount_point)
        .with_context(|| format!("failed to create mount point {}", mount_point.display()))?;

    let options = [
        MountOption::RO,
        MountOption::FSName("b00t-virtfs".to_string()),
    ];

    println!("mounting b00t virtfs at {}", mount_point.display());
    fuser::mount2(B00tFS, &mount_point, &options)
        .with_context(|| format!("failed to mount virtfs at {}", mount_point.display()))
}
