use std::{
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
};

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::HANDLE,
    Storage::FileSystem::{GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, VOLUME_NAME_DOS},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathAuthorizationError {
    NoRoots,
    RootMissing(PathBuf),
    RootNotDirectory(PathBuf),
    RelativePath,
    NetworkPath,
    DevicePath,
    AlternateDataStream,
    CannotResolve(PathBuf),
    OutsideRoots,
    TargetChanged,
}

impl fmt::Display for PathAuthorizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRoots => formatter.write_str("at least one authorized root is required"),
            Self::RootMissing(path) => write!(
                formatter,
                "authorized root does not exist: {}",
                path.display()
            ),
            Self::RootNotDirectory(path) => write!(
                formatter,
                "authorized root is not a directory: {}",
                path.display()
            ),
            Self::RelativePath => formatter.write_str("relative paths are not authorized"),
            Self::NetworkPath => formatter.write_str("network paths are not authorized"),
            Self::DevicePath => formatter.write_str("device namespace paths are not authorized"),
            Self::AlternateDataStream => {
                formatter.write_str("alternate data streams are not authorized")
            }
            Self::CannotResolve(path) => {
                write!(formatter, "cannot resolve path: {}", path.display())
            }
            Self::OutsideRoots => formatter.write_str("path is outside all authorized roots"),
            Self::TargetChanged => {
                formatter.write_str("opened target no longer matches the authorized target")
            }
        }
    }
}

impl std::error::Error for PathAuthorizationError {}

#[derive(Debug, Clone)]
pub struct AuthorizedRoots {
    roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedPath {
    canonical: PathBuf,
}

impl AuthorizedPath {
    pub fn canonical_path(&self) -> &Path {
        &self.canonical
    }

    /// A local DOS path derived only from this authorized filesystem result.
    /// Raw caller-supplied verbatim paths still fail `validate_raw_path`.
    pub fn local_path(&self) -> PathBuf {
        #[cfg(windows)]
        {
            use std::path::{Component, Prefix};
            let mut components = self.canonical.components();
            if let Some(Component::Prefix(prefix)) = components.next() {
                if let Prefix::VerbatimDisk(drive) = prefix.kind() {
                    let mut local = PathBuf::from(format!("{}:\\", drive as char));
                    for component in components {
                        if let Component::Normal(name) = component {
                            local.push(name);
                        }
                    }
                    return local;
                }
            }
        }
        self.canonical.clone()
    }
}

impl AuthorizedRoots {
    pub fn new(roots: Vec<PathBuf>) -> Result<Self, PathAuthorizationError> {
        if roots.is_empty() {
            return Err(PathAuthorizationError::NoRoots);
        }
        let mut canonical_roots: Vec<PathBuf> = Vec::with_capacity(roots.len());
        for root in roots {
            validate_raw_path(&root)?;
            let canonical = fs::canonicalize(&root)
                .map_err(|_| PathAuthorizationError::RootMissing(root.clone()))?;
            validate_resolved_path(&canonical)?;
            if !canonical.is_dir() {
                return Err(PathAuthorizationError::RootNotDirectory(canonical));
            }
            if !canonical_roots
                .iter()
                .any(|existing| same_path(existing, &canonical))
            {
                canonical_roots.push(canonical);
            }
        }
        Ok(Self {
            roots: canonical_roots,
        })
    }

    pub fn authorize(&self, path: &Path) -> Result<AuthorizedPath, PathAuthorizationError> {
        validate_raw_path(path)?;
        let canonical = resolve_with_missing_tail(path)?;
        if !self.roots.iter().any(|root| is_within(root, &canonical)) {
            return Err(PathAuthorizationError::OutsideRoots);
        }
        Ok(AuthorizedPath { canonical })
    }

    pub fn verify_target(
        &self,
        authorized: &AuthorizedPath,
        current: &Path,
    ) -> Result<(), PathAuthorizationError> {
        let canonical = resolve_with_missing_tail(current)
            .map_err(|_| PathAuthorizationError::TargetChanged)?;
        if same_path(&authorized.canonical, &canonical)
            && self.roots.iter().any(|root| is_within(root, &canonical))
        {
            Ok(())
        } else {
            Err(PathAuthorizationError::TargetChanged)
        }
    }

    #[cfg(windows)]
    pub fn verify_opened_file(
        &self,
        authorized: &AuthorizedPath,
        file: &fs::File,
    ) -> Result<(), PathAuthorizationError> {
        let opened = final_path_from_handle(file)?;
        if same_path(&authorized.canonical, &opened)
            && self.roots.iter().any(|root| is_within(root, &opened))
        {
            Ok(())
        } else {
            Err(PathAuthorizationError::TargetChanged)
        }
    }
}

pub fn authorize_existing_path(path: &Path) -> Result<AuthorizedPath, PathAuthorizationError> {
    let roots = sibling_roots(path)?;
    let authorized = roots.authorize(path)?;
    if !authorized.canonical_path().exists() {
        return Err(PathAuthorizationError::CannotResolve(path.to_path_buf()));
    }
    Ok(authorized)
}

pub fn authorize_existing_file(path: &Path) -> Result<AuthorizedPath, PathAuthorizationError> {
    let authorized = authorize_existing_path(path)?;
    if !authorized.canonical_path().is_file() {
        return Err(PathAuthorizationError::CannotResolve(path.to_path_buf()));
    }
    Ok(authorized)
}

pub fn authorize_existing_file_with_handle(
    path: &Path,
) -> Result<(AuthorizedPath, fs::File), PathAuthorizationError> {
    let roots = sibling_roots(path)?;
    let authorized = roots.authorize(path)?;
    let file = fs::File::open(authorized.canonical_path())
        .map_err(|_| PathAuthorizationError::CannotResolve(path.to_path_buf()))?;
    if !file
        .metadata()
        .map_err(|_| PathAuthorizationError::CannotResolve(path.to_path_buf()))?
        .is_file()
    {
        return Err(PathAuthorizationError::CannotResolve(path.to_path_buf()));
    }
    #[cfg(windows)]
    roots.verify_opened_file(&authorized, &file)?;
    Ok((authorized, file))
}

pub fn authorize_output_path(path: &Path) -> Result<AuthorizedPath, PathAuthorizationError> {
    sibling_roots(path)?.authorize(path)
}

fn sibling_roots(path: &Path) -> Result<AuthorizedRoots, PathAuthorizationError> {
    validate_raw_path(path)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| PathAuthorizationError::CannotResolve(path.to_path_buf()))?;
    AuthorizedRoots::new(vec![parent.to_path_buf()])
}

fn validate_raw_path(path: &Path) -> Result<(), PathAuthorizationError> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(PathAuthorizationError::RelativePath);
    }
    let text = path.to_string_lossy();
    #[cfg(windows)]
    let text = text.replace('/', "\\");
    let lowered = text.to_ascii_lowercase();
    if lowered.starts_with("\\\\.\\")
        || lowered.starts_with("\\\\?\\")
        || lowered.starts_with("\\device\\")
        || lowered.starts_with("\\\\device\\")
    {
        return Err(PathAuthorizationError::DevicePath);
    }
    if lowered.starts_with("\\\\") {
        return Err(PathAuthorizationError::NetworkPath);
    }
    if text
        .chars()
        .enumerate()
        .any(|(index, character)| character == ':' && index != 1)
    {
        return Err(PathAuthorizationError::AlternateDataStream);
    }
    Ok(())
}

// These paths come from canonicalize/GetFinalPathNameByHandleW, never from an
// unchecked request. Even filesystem results must resolve to a local disk.
fn validate_resolved_path(path: &Path) -> Result<(), PathAuthorizationError> {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        match path.components().next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::Disk(_) | Prefix::VerbatimDisk(_) => {}
                Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _) => {
                    return Err(PathAuthorizationError::NetworkPath);
                }
                _ => return Err(PathAuthorizationError::DevicePath),
            },
            _ => return Err(PathAuthorizationError::RelativePath),
        }
        for component in path.components() {
            if let Component::Normal(name) = component {
                if name.to_string_lossy().contains(':') {
                    return Err(PathAuthorizationError::AlternateDataStream);
                }
            }
            if matches!(component, Component::ParentDir | Component::CurDir) {
                return Err(PathAuthorizationError::CannotResolve(path.to_path_buf()));
            }
        }
    }
    #[cfg(not(windows))]
    if !path.is_absolute() {
        return Err(PathAuthorizationError::RelativePath);
    }
    Ok(())
}

fn resolve_with_missing_tail(path: &Path) -> Result<PathBuf, PathAuthorizationError> {
    if let Ok(canonical) = fs::canonicalize(path) {
        validate_resolved_path(&canonical)?;
        return Ok(canonical);
    }
    let mut cursor = path.to_path_buf();
    let mut missing = Vec::<OsString>::new();
    loop {
        if let Ok(canonical) = fs::canonicalize(&cursor) {
            let mut resolved = canonical;
            for component in missing.iter().rev() {
                resolved.push(component);
            }
            validate_resolved_path(&resolved)?;
            return Ok(resolved);
        }
        let Some(name) = cursor.file_name() else {
            return Err(PathAuthorizationError::CannotResolve(path.to_path_buf()));
        };
        missing.push(name.to_os_string());
        let Some(parent) = cursor.parent() else {
            return Err(PathAuthorizationError::CannotResolve(path.to_path_buf()));
        };
        if parent == cursor {
            return Err(PathAuthorizationError::CannotResolve(path.to_path_buf()));
        }
        cursor = parent.to_path_buf();
    }
}

fn is_within(root: &Path, candidate: &Path) -> bool {
    let root = normalized(root);
    let candidate = normalized(candidate);
    candidate.starts_with(&root)
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalized(left) == normalized(right)
}

fn normalized(path: &Path) -> Vec<OsString> {
    path.components()
        .map(|component| {
            #[cfg(windows)]
            {
                use std::path::{Component, Prefix};
                if let Component::Prefix(prefix) = component {
                    if let Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) = prefix.kind() {
                        return OsString::from(format!(
                            "{}:",
                            (drive as char).to_ascii_lowercase()
                        ));
                    }
                }
                OsString::from(component.as_os_str().to_string_lossy().to_ascii_lowercase())
            }
            #[cfg(not(windows))]
            component.as_os_str().to_owned()
        })
        .collect()
}

#[cfg(windows)]
fn final_path_from_handle(file: &fs::File) -> Result<PathBuf, PathAuthorizationError> {
    let handle = file.as_raw_handle() as HANDLE;
    let mut buffer = vec![0u16; 512];
    loop {
        let length = unsafe {
            GetFinalPathNameByHandleW(
                handle,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        };
        if length == 0 {
            return Err(PathAuthorizationError::TargetChanged);
        }
        let length = length as usize;
        if length < buffer.len() {
            let path = String::from_utf16(&buffer[..length])
                .map(PathBuf::from)
                .map_err(|_| PathAuthorizationError::TargetChanged)?;
            validate_resolved_path(&path)?;
            return Ok(path);
        }
        buffer.resize(length + 1, 0);
    }
}
