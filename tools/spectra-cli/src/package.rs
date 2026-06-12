use crate::discovery;
use crate::release_channel::ReleaseMetadata;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MANIFEST_NAMES: &[&str] = &["spectra.toml", "Spectra.toml"];
const LOCKFILE_NAME: &str = "spectra.lock";

#[derive(Clone, Debug)]
pub enum PackageCommand {
    Lock,
    Build,
    Check,
    Run,
    Test,
    Bench,
    Doc,
    Add {
        name: String,
        version: Option<String>,
        path: Option<PathBuf>,
        registry: Option<PathBuf>,
    },
    Update,
    Publish {
        registry: PathBuf,
    },
}

#[derive(Clone, Debug)]
pub struct PackageInvocation {
    pub root: PathBuf,
    pub command: PackageCommand,
}

#[derive(Clone, Debug)]
pub struct ResolvedWorkspace {
    pub root: PathBuf,
    pub root_name: String,
    pub packages: Vec<ResolvedPackage>,
}

impl ResolvedWorkspace {
    pub fn source_entries(&self) -> Vec<PathBuf> {
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();

        for package in &self.packages {
            for src in &package.src_dirs {
                if seen.insert(src.clone()) {
                    entries.push(src.clone());
                }
            }
        }

        entries
    }

    pub fn root_package_name(&self) -> Option<String> {
        Some(self.root_name.clone())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedPackage {
    pub name: String,
    pub version: String,
    pub release: ReleaseMetadata,
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub src_dirs: Vec<PathBuf>,
    pub entry: Option<PathBuf>,
    pub dependencies: BTreeMap<String, ResolvedDependency>,
    pub manifest_hash: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedDependency {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum PackageError {
    Io {
        path: PathBuf,
        error: io::Error,
    },
    Parse {
        path: PathBuf,
        error: toml::de::Error,
    },
    Serialize(toml::ser::Error),
    MissingManifest(PathBuf),
    MissingPackage(String),
    DuplicatePackage(String),
    InvalidManifest {
        path: PathBuf,
        message: String,
    },
    Registry(String),
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::Io { path, error } => {
                write!(f, "failed to access '{}': {}", path.display(), error)
            }
            PackageError::Parse { path, error } => {
                write!(f, "failed to parse '{}': {}", path.display(), error)
            }
            PackageError::Serialize(error) => write!(f, "failed to serialize lockfile: {}", error),
            PackageError::MissingManifest(path) => {
                write!(f, "no spectra.toml manifest found at '{}'", path.display())
            }
            PackageError::MissingPackage(name) => write!(f, "package '{}' was not found", name),
            PackageError::DuplicatePackage(name) => {
                write!(
                    f,
                    "package '{}' appears more than once in the workspace",
                    name
                )
            }
            PackageError::InvalidManifest { path, message } => {
                write!(f, "invalid manifest '{}': {}", path.display(), message)
            }
            PackageError::Registry(message) => write!(f, "registry error: {}", message),
        }
    }
}

impl std::error::Error for PackageError {}

#[derive(Debug, Deserialize)]
struct Manifest {
    project: ProjectSection,
    #[serde(default)]
    release: ReleaseMetadata,
    #[serde(default)]
    workspace: WorkspaceSection,
    #[serde(default)]
    dependencies: BTreeMap<String, DependencySpec>,
}

#[derive(Debug, Deserialize)]
struct ProjectSection {
    name: String,
    #[serde(default = "default_version")]
    version: String,
    entry: Option<String>,
    #[serde(default)]
    src_dirs: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WorkspaceSection {
    #[serde(default)]
    members: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum DependencySpec {
    Version(String),
    Detailed {
        version: Option<String>,
        path: Option<String>,
    },
}

#[derive(Serialize)]
struct Lockfile {
    version: u32,
    root: String,
    packages: Vec<LockPackage>,
}

#[derive(Serialize)]
struct LockPackage {
    name: String,
    version: String,
    channel: String,
    compatibility: String,
    deprecated_since: Option<String>,
    migration: Option<String>,
    source: String,
    manifest_hash: String,
    dependencies: Vec<LockDependency>,
}

#[derive(Serialize)]
struct LockDependency {
    name: String,
    version: String,
    source: String,
}

#[derive(Serialize, Deserialize)]
struct RegistryMetadata {
    name: String,
    version: String,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    compatibility: String,
    #[serde(default)]
    deprecated_since: Option<String>,
    #[serde(default)]
    migration: Option<String>,
    checksum: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

pub fn resolve(root: &Path) -> Result<ResolvedWorkspace, PackageError> {
    let root = canonicalize_existing(root)?;
    let manifest_path = find_manifest(&root)?;
    let root_package = load_package(&manifest_path)?;
    let root_name = root_package.name.clone();
    let mut package_roots = Vec::new();
    package_roots.push(root_package.root.clone());

    for member in &root_package.workspace_members {
        package_roots.push(canonicalize_existing(&root_package.root.join(member))?);
    }

    let mut visited = HashSet::new();
    let mut by_name = BTreeMap::new();
    let mut ordered = Vec::new();

    for package_root in package_roots {
        collect_package(&package_root, &mut visited, &mut by_name, &mut ordered)?;
    }

    let packages = topo_sort(ordered)?;
    Ok(ResolvedWorkspace {
        root,
        root_name,
        packages,
    })
}

pub fn write_lockfile(workspace: &ResolvedWorkspace) -> Result<PathBuf, PackageError> {
    let lockfile = Lockfile {
        version: 1,
        root: workspace
            .root_package_name()
            .unwrap_or_else(|| "workspace".to_string()),
        packages: workspace
            .packages
            .iter()
            .map(|package| LockPackage {
                name: package.name.clone(),
                version: package.version.clone(),
                channel: package.release.channel.as_str().to_string(),
                compatibility: package.release.compatibility.clone(),
                deprecated_since: package.release.deprecated_since.clone(),
                migration: package.release.migration.clone(),
                source: path_source(&workspace.root, &package.root),
                manifest_hash: package.manifest_hash.clone(),
                dependencies: package
                    .dependencies
                    .values()
                    .map(|dep| LockDependency {
                        name: dep.name.clone(),
                        version: dep.version.clone(),
                        source: path_source(&workspace.root, &dep.path),
                    })
                    .collect(),
            })
            .collect(),
    };

    let text = toml::to_string_pretty(&lockfile).map_err(PackageError::Serialize)?;
    let path = workspace.root.join(LOCKFILE_NAME);
    fs::write(&path, text).map_err(|error| PackageError::Io {
        path: path.clone(),
        error,
    })?;
    Ok(path)
}

pub fn add_dependency(
    root: &Path,
    name: &str,
    version: Option<&str>,
    path: Option<&Path>,
    registry: Option<&Path>,
) -> Result<PathBuf, PackageError> {
    let root = canonicalize_existing(root)?;
    let manifest_path = find_manifest(&root)?;
    let dependency_path = if let Some(path) = path {
        relative_or_absolute(&root, path)
    } else if let Some(registry) = registry {
        let version = version.unwrap_or("0.1.0");
        install_from_registry(&root, registry, name, version)?
    } else {
        return Err(PackageError::InvalidManifest {
            path: manifest_path,
            message: "package add requires --path or --registry".to_string(),
        });
    };

    let version = version.unwrap_or("0.1.0");
    if !is_valid_semver(version) {
        return Err(PackageError::InvalidManifest {
            path: manifest_path.clone(),
            message: format!("dependency '{}' has invalid semver '{}'", name, version),
        });
    }
    let mut manifest = fs::read_to_string(&manifest_path).map_err(|error| PackageError::Io {
        path: manifest_path.clone(),
        error,
    })?;

    if !manifest.contains("[dependencies]") {
        manifest.push_str("\n[dependencies]\n");
    }
    manifest.push_str(&format!(
        "{} = {{ version = \"{}\", path = \"{}\" }}\n",
        name,
        version,
        dependency_path.to_string_lossy().replace('\\', "/")
    ));

    fs::write(&manifest_path, manifest).map_err(|error| PackageError::Io {
        path: manifest_path.clone(),
        error,
    })?;

    let workspace = resolve(&root)?;
    write_lockfile(&workspace)
}

pub fn publish(root: &Path, registry: &Path) -> Result<PathBuf, PackageError> {
    let workspace = resolve(root)?;
    let package = workspace
        .packages
        .first()
        .ok_or_else(|| PackageError::MissingPackage("root".to_string()))?;
    let package_dir = registry.join(&package.name).join(&package.version);
    let payload_dir = package_dir.join("package");

    if package_dir.exists() {
        fs::remove_dir_all(&package_dir).map_err(|error| PackageError::Io {
            path: package_dir.clone(),
            error,
        })?;
    }
    fs::create_dir_all(&payload_dir).map_err(|error| PackageError::Io {
        path: payload_dir.clone(),
        error,
    })?;

    copy_package_payload(&package.root, &payload_dir)?;
    let checksum = directory_checksum(&payload_dir)?;
    let metadata = RegistryMetadata {
        name: package.name.clone(),
        version: package.version.clone(),
        channel: package.release.channel.as_str().to_string(),
        compatibility: package.release.compatibility.clone(),
        deprecated_since: package.release.deprecated_since.clone(),
        migration: package.release.migration.clone(),
        checksum,
    };
    let metadata_text = toml::to_string_pretty(&metadata).map_err(PackageError::Serialize)?;
    let metadata_path = package_dir.join("package.toml");
    fs::write(&metadata_path, metadata_text).map_err(|error| PackageError::Io {
        path: metadata_path.clone(),
        error,
    })?;

    Ok(package_dir)
}

pub fn write_docs(workspace: &ResolvedWorkspace) -> Result<PathBuf, PackageError> {
    let docs_dir = workspace.root.join("target").join("spectra-docs");
    fs::create_dir_all(&docs_dir).map_err(|error| PackageError::Io {
        path: docs_dir.clone(),
        error,
    })?;
    let path = docs_dir.join("packages.md");
    let mut text = String::from("# Spectra Packages\n\n");
    for package in &workspace.packages {
        text.push_str(&format!("## {} {}\n\n", package.name, package.version));
        text.push_str(&format!("- channel: `{}`\n", package.release.channel));
        text.push_str(&format!(
            "- compatibility: `{}`\n",
            package.release.compatibility
        ));
        if let Some(warning) = package.release.deprecation_warning(&package.name) {
            text.push_str(&format!("- deprecation: `{}`\n", warning));
        }
        text.push_str(&format!("- root: `{}`\n", package.root.display()));
        text.push_str(&format!("- manifest: `{}`\n", package.manifest.display()));
        if package.dependencies.is_empty() {
            text.push_str("- dependencies: none\n\n");
        } else {
            text.push_str("- dependencies:\n");
            for dependency in package.dependencies.values() {
                text.push_str(&format!(
                    "  - {} {} from `{}`\n",
                    dependency.name,
                    dependency.version,
                    dependency.path.display()
                ));
            }
            text.push('\n');
        }
    }
    fs::write(&path, text).map_err(|error| PackageError::Io {
        path: path.clone(),
        error,
    })?;
    Ok(path)
}

#[derive(Debug)]
struct LoadedPackage {
    name: String,
    version: String,
    release: ReleaseMetadata,
    root: PathBuf,
    manifest: PathBuf,
    src_dirs: Vec<PathBuf>,
    entry: Option<PathBuf>,
    workspace_members: Vec<String>,
    dependency_specs: BTreeMap<String, DependencySpec>,
    manifest_hash: String,
}

fn collect_package(
    root: &Path,
    visited: &mut HashSet<PathBuf>,
    by_name: &mut BTreeMap<String, PathBuf>,
    ordered: &mut Vec<ResolvedPackage>,
) -> Result<(), PackageError> {
    let root = canonicalize_existing(root)?;
    if !visited.insert(root.clone()) {
        return Ok(());
    }

    let manifest_path = find_manifest(&root)?;
    let loaded = load_package(&manifest_path)?;
    if by_name.insert(loaded.name.clone(), root.clone()).is_some() {
        return Err(PackageError::DuplicatePackage(loaded.name));
    }

    let mut dependencies = BTreeMap::new();
    for (dep_name, spec) in &loaded.dependency_specs {
        let dep_root = match spec {
            DependencySpec::Detailed {
                path: Some(path), ..
            } => canonicalize_existing(&root.join(path))?,
            DependencySpec::Version(_) | DependencySpec::Detailed { path: None, .. } => {
                return Err(PackageError::InvalidManifest {
                    path: loaded.manifest.clone(),
                    message: format!(
                        "dependency '{}' must declare a local path for the Phase 9 MVP",
                        dep_name
                    ),
                });
            }
        };
        collect_package(&dep_root, visited, by_name, ordered)?;
        let dep_manifest = load_package(&find_manifest(&dep_root)?)?;
        let version = match spec {
            DependencySpec::Version(version) => version.clone(),
            DependencySpec::Detailed { version, .. } => {
                version.clone().unwrap_or(dep_manifest.version)
            }
        };
        if !is_valid_semver(&version) {
            return Err(PackageError::InvalidManifest {
                path: loaded.manifest.clone(),
                message: format!("dependency '{}' has invalid semver '{}'", dep_name, version),
            });
        }
        dependencies.insert(
            dep_name.clone(),
            ResolvedDependency {
                name: dep_name.clone(),
                version,
                path: dep_root,
            },
        );
    }

    ordered.push(ResolvedPackage {
        name: loaded.name,
        version: loaded.version,
        release: loaded.release,
        root: loaded.root,
        manifest: loaded.manifest,
        src_dirs: loaded.src_dirs,
        entry: loaded.entry,
        dependencies,
        manifest_hash: loaded.manifest_hash,
    });

    Ok(())
}

fn topo_sort(packages: Vec<ResolvedPackage>) -> Result<Vec<ResolvedPackage>, PackageError> {
    let mut remaining: BTreeMap<String, ResolvedPackage> = packages
        .into_iter()
        .map(|package| (package.name.clone(), package))
        .collect();
    let mut emitted = BTreeSet::new();
    let mut ordered = Vec::new();

    while !remaining.is_empty() {
        let ready_name = remaining
            .iter()
            .find(|(_, package)| {
                package
                    .dependencies
                    .keys()
                    .all(|dependency| emitted.contains(dependency))
            })
            .map(|(name, _)| name.clone());

        let Some(name) = ready_name else {
            return Err(PackageError::Registry(
                "cyclic package dependency detected".to_string(),
            ));
        };
        let package = remaining.remove(&name).expect("ready package exists");
        emitted.insert(name);
        ordered.push(package);
    }

    Ok(ordered)
}

fn load_package(manifest_path: &Path) -> Result<LoadedPackage, PackageError> {
    let manifest_path = canonicalize_existing(manifest_path)?;
    let root = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| PackageError::MissingManifest(manifest_path.clone()))?;
    let text = fs::read_to_string(&manifest_path).map_err(|error| PackageError::Io {
        path: manifest_path.clone(),
        error,
    })?;
    let manifest: Manifest = toml::from_str(&text).map_err(|error| PackageError::Parse {
        path: manifest_path.clone(),
        error,
    })?;
    if !is_valid_semver(&manifest.project.version) {
        return Err(PackageError::InvalidManifest {
            path: manifest_path.clone(),
            message: format!(
                "project.version '{}' is not valid semver MAJOR.MINOR.PATCH",
                manifest.project.version
            ),
        });
    }
    manifest
        .release
        .validate()
        .map_err(|message| PackageError::InvalidManifest {
            path: manifest_path.clone(),
            message,
        })?;
    let src_dirs = if manifest.project.src_dirs.is_empty() {
        vec![root.join("src")]
    } else {
        manifest
            .project
            .src_dirs
            .iter()
            .map(|src| root.join(src))
            .collect()
    };

    Ok(LoadedPackage {
        name: manifest.project.name,
        version: manifest.project.version,
        release: manifest.release,
        root,
        manifest: manifest_path,
        src_dirs,
        entry: manifest.project.entry.map(|entry| PathBuf::from(entry)),
        workspace_members: manifest.workspace.members,
        dependency_specs: manifest.dependencies,
        manifest_hash: stable_hash_hex(text.as_bytes()),
    })
}

fn install_from_registry(
    root: &Path,
    registry: &Path,
    name: &str,
    version: &str,
) -> Result<PathBuf, PackageError> {
    let registry_package = registry.join(name).join(version);
    let metadata_path = registry_package.join("package.toml");
    let metadata_text = fs::read_to_string(&metadata_path).map_err(|error| PackageError::Io {
        path: metadata_path.clone(),
        error,
    })?;
    let metadata: RegistryMetadata =
        toml::from_str(&metadata_text).map_err(|error| PackageError::Parse {
            path: metadata_path.clone(),
            error,
        })?;
    let payload = registry_package.join("package");
    let checksum = directory_checksum(&payload)?;
    if checksum != metadata.checksum {
        return Err(PackageError::Registry(format!(
            "checksum mismatch for {} {}",
            name, version
        )));
    }

    let vendor_dir = root.join(".spectra").join("packages").join(format!(
        "{}-{}",
        name.replace('/', "_"),
        version
    ));
    if vendor_dir.exists() {
        fs::remove_dir_all(&vendor_dir).map_err(|error| PackageError::Io {
            path: vendor_dir.clone(),
            error,
        })?;
    }
    fs::create_dir_all(&vendor_dir).map_err(|error| PackageError::Io {
        path: vendor_dir.clone(),
        error,
    })?;
    copy_package_payload(&payload, &vendor_dir)?;
    Ok(vendor_dir)
}

fn find_manifest(root: &Path) -> Result<PathBuf, PackageError> {
    for name in MANIFEST_NAMES {
        let candidate = root.join(name);
        if candidate.is_file() {
            return canonicalize_existing(&candidate);
        }
    }
    Err(PackageError::MissingManifest(root.to_path_buf()))
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, PackageError> {
    fs::canonicalize(path).map_err(|error| PackageError::Io {
        path: path.to_path_buf(),
        error,
    })
}

fn relative_or_absolute(root: &Path, path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    pathdiff(&absolute, root).unwrap_or(absolute)
}

fn path_source(root: &Path, path: &Path) -> String {
    let relative = pathdiff(path, root).unwrap_or_else(|| path.to_path_buf());
    format!("path+{}", relative.to_string_lossy().replace('\\', "/"))
}

fn pathdiff(path: &Path, base: &Path) -> Option<PathBuf> {
    let path = path.components().collect::<Vec<_>>();
    let base = base.components().collect::<Vec<_>>();
    let common = path
        .iter()
        .zip(base.iter())
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return None;
    }
    let mut result = PathBuf::new();
    for _ in common..base.len() {
        result.push("..");
    }
    for component in &path[common..] {
        result.push(component.as_os_str());
    }
    Some(result)
}

fn copy_package_payload(from: &Path, to: &Path) -> Result<(), PackageError> {
    for entry in fs::read_dir(from).map_err(|error| PackageError::Io {
        path: from.to_path_buf(),
        error,
    })? {
        let entry = entry.map_err(|error| PackageError::Io {
            path: from.to_path_buf(),
            error,
        })?;
        let path = entry.path();
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if matches!(name_text.as_ref(), "target" | ".git" | ".spectra") {
            continue;
        }
        let dest = to.join(&name);
        if path.is_dir() {
            fs::create_dir_all(&dest).map_err(|error| PackageError::Io {
                path: dest.clone(),
                error,
            })?;
            copy_package_payload(&path, &dest)?;
        } else if path.is_file() {
            fs::copy(&path, &dest).map_err(|error| PackageError::Io {
                path: path.clone(),
                error,
            })?;
        }
    }
    Ok(())
}

fn directory_checksum(path: &Path) -> Result<String, PackageError> {
    let mut files = Vec::new();
    collect_files(path, path, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut bytes = Vec::new();
    for (relative, full) in files {
        bytes.extend_from_slice(relative.to_string_lossy().replace('\\', "/").as_bytes());
        bytes.push(0);
        let content = fs::read(&full).map_err(|error| PackageError::Io { path: full, error })?;
        bytes.extend_from_slice(&content);
        bytes.push(0);
    }
    Ok(stable_hash_hex(&bytes))
}

fn collect_files(
    root: &Path,
    current: &Path,
    out: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), PackageError> {
    for entry in fs::read_dir(current).map_err(|error| PackageError::Io {
        path: current.to_path_buf(),
        error,
    })? {
        let entry = entry.map_err(|error| PackageError::Io {
            path: current.to_path_buf(),
            error,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else if path.is_file() {
            let relative = pathdiff(&path, root).unwrap_or_else(|| path.clone());
            out.push((relative, path));
        }
    }
    Ok(())
}

fn stable_hash_hex(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

fn is_valid_semver(version: &str) -> bool {
    let (core, suffix) = match version.split_once('-') {
        Some((core, suffix)) => (core, Some(suffix)),
        None => (version, None),
    };
    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return false;
    }
    if let Some(suffix) = suffix {
        !suffix.is_empty()
            && suffix
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
    } else {
        true
    }
}

pub fn discover_test_entries(workspace: &ResolvedWorkspace) -> Vec<PathBuf> {
    let mut entries = Vec::new();
    for package in &workspace.packages {
        let tests_dir = package.root.join("tests");
        if tests_dir.is_dir() {
            entries.extend(discovery::discover_sources(&[tests_dir]));
        }
    }
    entries
}

pub fn deprecation_warnings(workspace: &ResolvedWorkspace) -> Vec<String> {
    workspace
        .packages
        .iter()
        .filter_map(|package| package.release.deprecation_warning(&package.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_hash_is_deterministic() {
        assert_eq!(stable_hash_hex(b"abc"), stable_hash_hex(b"abc"));
        assert_ne!(stable_hash_hex(b"abc"), stable_hash_hex(b"abcd"));
    }

    #[test]
    fn semver_validation_accepts_exact_versions_and_prerelease() {
        assert!(is_valid_semver("1.2.3"));
        assert!(is_valid_semver("1.2.3-alpha.1"));
        assert!(!is_valid_semver("1.2"));
        assert!(!is_valid_semver("latest"));
    }
}
