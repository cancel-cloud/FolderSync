use crate::*;
use anyhow::{Context, Result, bail, ensure};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::{
    fs::{self, File},
    io::Read,
    path::{Component, Path},
};
use walkdir::WalkDir;

pub fn path_key(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}
pub fn overlap(a: &Path, b: &Path) -> bool {
    let a = path_key(&a.to_string_lossy());
    let b = path_key(&b.to_string_lossy());
    a == b || a.starts_with(&(b.clone() + "/")) || b.starts_with(&(a + "/"))
}
fn reparse(m: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        m.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        m.is_symlink()
    }
}
pub fn native_file_id(root: &Path) -> Result<String> {
    #[cfg(windows)]
    {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
        };
        let f = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(root)?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { GetFileInformationByHandle(f.as_raw_handle(), &mut info) } != 0,
            "File identity unavailable"
        );
        Ok(format!(
            "{}:{}:{}",
            info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
        ))
    }
    #[cfg(not(windows))]
    {
        Ok(fs::canonicalize(root)?.to_string_lossy().into_owned())
    }
}
pub fn root_identity(root: &Path) -> Result<String> {
    let m = fs::symlink_metadata(root)
        .with_context(|| format!("Root unavailable: {}", root.display()))?;
    ensure!(
        m.is_dir() && !reparse(&m),
        "Root must be a directory without a junction or symlink"
    );
    fs::read_dir(root)?.next().transpose()?;
    let canonical = fs::canonicalize(root)?;
    #[cfg(windows)]
    {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
        };
        let f = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(root)?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { GetFileInformationByHandle(f.as_raw_handle(), &mut info) } != 0,
            "Cannot identify root"
        );
        Ok(format!(
            "{}|{}|{}|{}",
            canonical.display(),
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow
        ))
    }
    #[cfg(not(windows))]
    {
        Ok(canonical.to_string_lossy().into_owned())
    }
}
pub fn validate_roots(task: &Task, storage: &Path) -> Result<(String, String)> {
    let identities = (root_identity(&task.a)?, root_identity(&task.b)?);
    let a = fs::canonicalize(&task.a)?;
    let b = fs::canonicalize(&task.b)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS;
        let check = |root: &Path| -> Result<()> {
            let _handle = fs::OpenOptions::new()
                .access_mode(7)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(root)
                .with_context(|| format!("Folder write access unavailable: {}", root.display()))?;
            Ok(())
        };
        check(&task.b)?;
        if matches!(task.mode, Mode::TwoWay | Mode::Move) {
            check(&task.a)?;
        }
    }
    ensure!(
        !overlap(&a, &b),
        "Folders must be distinct and cannot overlap"
    );
    let storage = if storage.exists() {
        fs::canonicalize(storage)?
    } else {
        storage.to_path_buf()
    };
    ensure!(
        !overlap(&a, &storage) && !overlap(&b, &storage),
        "Protected storage must be outside synchronization roots"
    );
    Ok(identities)
}
pub fn safe_path(root: &Path, relative: &str) -> Result<std::path::PathBuf> {
    ensure!(!relative.is_empty(), "Empty relative path");
    let relative = Path::new(relative);
    ensure!(
        relative
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "Unsafe relative path"
    );
    let mut p = root.to_path_buf();
    for c in relative.components() {
        p.push(c);
        match fs::symlink_metadata(&p) {
            Ok(m) => ensure!(
                !reparse(&m),
                "Reparse points are unsupported: {}",
                p.display()
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(p)
}
fn patterns(p: &[String]) -> Result<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for s in p.iter().filter(|s| !s.trim().is_empty()) {
        let s = s.trim().trim_start_matches('/').replace('\\', "/");
        b.add(
            GlobBuilder::new(&s)
                .case_insensitive(true)
                .literal_separator(false)
                .build()?,
        );
    }
    Ok(b.build()?)
}
pub struct Matcher {
    scope: Scope,
    inc: GlobSet,
    exc: GlobSet,
    finc: GlobSet,
    fexc: GlobSet,
}
impl Matcher {
    pub fn new(s: &Scope) -> Result<Self> {
        Ok(Self {
            scope: s.clone(),
            inc: patterns(&s.include)?,
            exc: patterns(&s.exclude)?,
            finc: patterns(&s.folder_include)?,
            fexc: patterns(&s.folder_exclude)?,
        })
    }
    pub fn allows(&self, path: &str, directory: bool) -> bool {
        if path
            .split('/')
            .any(|p| p.starts_with(".foldersync-stage-") || p.starts_with(".foldersync-displaced-"))
        {
            return false;
        }
        let parts: Vec<_> = path.split('/').collect();
        if !self.scope.selected_folders.is_empty()
            && (parts.len() > 1 || directory)
            && !self
                .scope
                .selected_folders
                .iter()
                .any(|p| p.eq_ignore_ascii_case(parts[0]))
        {
            return false;
        }
        if !directory && parts.len() == 1 && self.scope.exclude_root_files {
            return false;
        }
        for i in 1..=if directory {
            parts.len()
        } else {
            parts.len().saturating_sub(1)
        } {
            let folder = parts[..i].join("/");
            if self.fexc.is_match(&folder) || self.fexc.is_match(parts[i - 1]) {
                return false;
            }
        }
        if !self.finc.is_empty()
            && parts.len() > 1
            && !(1..parts.len()).any(|i| self.finc.is_match(parts[..i].join("/")))
        {
            return directory;
        }
        directory
            || ((!self.exc.is_match(path) && !self.exc.is_match(parts.last().unwrap()))
                && (self.inc.is_empty()
                    || self.inc.is_match(path)
                    || self.inc.is_match(parts.last().unwrap())))
    }
}
pub fn read_guard(path: &Path) -> Result<File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    Ok(options.open(path)?)
}
pub fn hash_file(path: &Path, metrics: &mut Metrics) -> Result<(String, u64)> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0; 256 * 1024];
    let mut size = 0;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        size += n as u64;
    }
    metrics.hashed_bytes += size;
    Ok((h.finalize().to_hex().to_string(), size))
}
pub fn excluded_attributes(root: &Path, relative: &str, scope: &Scope) -> Result<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if scope.skip_hidden || scope.skip_system {
            let mut ancestor = root.to_path_buf();
            for component in Path::new(relative).components() {
                ancestor.push(component);
                let attrs = match fs::symlink_metadata(&ancestor) {
                    Ok(m) => m.file_attributes(),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                    Err(e) => return Err(e.into()),
                };
                if (scope.skip_hidden && attrs & 2 != 0) || (scope.skip_system && attrs & 4 != 0) {
                    return Ok(true);
                }
            }
        }
    }
    #[cfg(not(windows))]
    let _ = (root, relative, scope);
    Ok(false)
}
pub fn read_entry(
    root: &Path,
    relative: &str,
    scope: &Scope,
    metrics: &mut Metrics,
) -> Result<Option<Entry>> {
    let path = safe_path(root, relative)?;
    if excluded_attributes(root, relative, scope)? {
        return Ok(None);
    }
    let m = match fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    ensure!(!reparse(&m), "Symlinks/junctions are unsupported");
    if !Matcher::new(scope)?.allows(relative, m.is_dir()) {
        return Ok(None);
    }
    metrics.scanned_entries += 1;
    let modified = m.modified()?;
    let file_id = native_file_id(&path).ok();
    let (digest, size) = if m.is_dir() {
        ("directory".into(), 0)
    } else {
        ensure!(m.is_file(), "Unsupported file type");
        hash_file(&path, metrics)?
    };
    let after = fs::metadata(&path)?;
    ensure!(
        after.len() == m.len() && after.modified()? == modified,
        "File changed while scanning: {relative}"
    );
    Ok(Some(Entry {
        path: relative.replace('\\', "/"),
        digest,
        directory: m.is_dir(),
        size,
        mtime_ns: modified
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
            .to_string(),
        file_id,
    }))
}
pub fn scan(root: &Path, scope: &Scope, metrics: &mut Metrics) -> Result<Tree> {
    root_identity(root)?;
    let matcher = Matcher::new(scope)?;
    let mut tree = Tree::new();
    let mut it = WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .into_iter();
    while let Some(next) = it.next() {
        let item = next?;
        let relative = item
            .path()
            .strip_prefix(root)?
            .to_str()
            .context("Non-Unicode path unsupported")?
            .replace('\\', "/");
        if !matcher.allows(&relative, item.file_type().is_dir()) {
            if item.file_type().is_dir() {
                it.skip_current_dir()
            }
            continue;
        }
        match read_entry(root, &relative, scope, metrics)? {
            Some(e) => {
                if tree.insert(path_key(&relative), e).is_some() {
                    bail!("Case-insensitive path collision: {relative}")
                }
            }
            None if item.file_type().is_dir() => it.skip_current_dir(),
            None => {}
        }
    }
    root_identity(root)?;
    Ok(tree)
}
