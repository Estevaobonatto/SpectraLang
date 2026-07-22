use crate::discovery;
use crate::release_channel::{cli_compatibility_level, ReleaseMetadata};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::{value, DocumentMut, Item, Table};

const MANIFEST_NAMES: &[&str] = &["spectra.toml", "Spectra.toml"];
const LOCKFILE_NAME: &str = "spectra.lock";

#[derive(Clone, Debug)]
pub enum PackageCommand {
    Lock,
    Build,
    Check,
    Run,
    Test(PackageTestOptions),
    Bench,
    Doc,
    Add {
        name: String,
        version: Option<String>,
        path: Option<PathBuf>,
        registry: Option<PathBuf>,
        git: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        branch: Option<String>,
        catalog: Option<PathBuf>,
    },
    Update,
    Fetch {
        offline: bool,
    },
    Search {
        query: String,
        catalog: Option<PathBuf>,
    },
    Info {
        name: String,
        catalog: Option<PathBuf>,
    },
    Versions {
        name: String,
        catalog: Option<PathBuf>,
    },
    Tree,
    Register {
        git: String,
        tag: Option<String>,
        rev: Option<String>,
        branch: Option<String>,
        catalog: PathBuf,
    },
    PublishMetadata {
        out: PathBuf,
        git: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        branch: Option<String>,
    },
    Catalog(CatalogCommand),
    Publish {
        registry: PathBuf,
    },
}

#[derive(Clone, Debug)]
pub enum CatalogCommand {
    Add { name: String, source: String },
    List,
    Sync,
    Remove { name: String },
}

#[derive(Clone, Debug, Default)]
pub struct PackageTestOptions {
    pub filter: Option<String>,
    pub list: bool,
    pub json: bool,
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
    pub source: PackageSource,
    pub checksum: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResolvedDependency {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub source: PackageSource,
}

#[derive(Clone, Debug, Serialize)]
pub enum PackageSource {
    Path {
        path: PathBuf,
    },
    Registry {
        path: PathBuf,
    },
    Git {
        url: String,
        requested: String,
        resolved: String,
    },
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
    EditParse {
        path: PathBuf,
        message: String,
    },
    Serialize(toml::ser::Error),
    MissingManifest(PathBuf),
    MissingPackage(String),
    DuplicatePackage {
        name: String,
        first: PathBuf,
        second: PathBuf,
    },
    IncompatiblePackage {
        name: String,
        required: String,
        found: String,
        source: String,
    },
    ConflictingCatalogPackage {
        name: String,
        version: String,
        first: String,
        second: String,
    },
    DependencyCycle(Vec<String>),
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
            PackageError::EditParse { path, message } => {
                write!(f, "failed to edit '{}': {}", path.display(), message)
            }
            PackageError::Serialize(error) => write!(f, "failed to serialize lockfile: {}", error),
            PackageError::MissingManifest(path) => {
                write!(f, "no spectra.toml manifest found at '{}'", path.display())
            }
            PackageError::MissingPackage(name) => write!(f, "package '{}' was not found", name),
            PackageError::DuplicatePackage {
                name,
                first,
                second,
            } => write!(
                f,
                "package '{}' appears more than once in the workspace: '{}' and '{}'",
                name,
                first.display(),
                second.display()
            ),
            PackageError::IncompatiblePackage {
                name,
                required,
                found,
                source,
            } => write!(
                f,
                "package '{}' is incompatible with CLI compatibility '{}': found '{}' at {}",
                name, required, found, source
            ),
            PackageError::ConflictingCatalogPackage {
                name,
                version,
                first,
                second,
            } => write!(
                f,
                "catalog entries for '{}' version '{}' conflict: '{}' and '{}'",
                name, version, first, second
            ),
            PackageError::DependencyCycle(chain) => {
                write!(
                    f,
                    "cyclic package dependency detected: {}",
                    chain.join(" -> ")
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
    package: PackageSection,
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

#[derive(Debug, Default, Deserialize)]
struct PackageSection {
    #[serde(default)]
    catalogs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum DependencySpec {
    Version(String),
    Detailed {
        version: Option<String>,
        path: Option<String>,
        registry: Option<String>,
        git: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        branch: Option<String>,
        checksum: Option<String>,
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
    source_kind: String,
    checksum: String,
    git_url: Option<String>,
    git_ref: Option<String>,
    resolved_rev: Option<String>,
    manifest_hash: String,
    dependencies: Vec<LockDependency>,
}

#[derive(Serialize)]
struct LockDependency {
    name: String,
    version: String,
    source: String,
    source_kind: String,
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
    #[serde(default)]
    source_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CatalogIndex {
    #[serde(default = "catalog_schema")]
    schema: String,
    #[serde(default)]
    packages: Vec<CatalogPackage>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CatalogPackage {
    name: String,
    version: String,
    git: String,
    #[serde(default)]
    tag: Option<String>,
    #[serde(default)]
    rev: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    resolved_rev: Option<String>,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    compatibility: String,
    #[serde(default)]
    license: String,
    #[serde(default)]
    modules: Vec<String>,
    #[serde(default)]
    owner: String,
    #[serde(skip)]
    origin: String,
}

struct InstalledGitPackage {
    name: String,
    version: String,
    path: PathBuf,
    resolved: String,
    checksum: String,
}

struct InstalledRegistryPackage {
    canonical_name: String,
    version: String,
    path: PathBuf,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn catalog_schema() -> String {
    "spectra-package-catalog-v1".to_string()
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
    let mut active = Vec::new();

    for package_root in package_roots {
        collect_package(
            &root,
            &package_root,
            PackageSource::Path {
                path: package_root.clone(),
            },
            &mut visited,
            &mut by_name,
            &mut ordered,
            &mut active,
        )?;
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
        version: 2,
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
                source_kind: package.source.kind().to_string(),
                checksum: package.checksum.clone(),
                git_url: package.source.git_url(),
                git_ref: package.source.git_ref(),
                resolved_rev: package.source.git_resolved(),
                manifest_hash: package.manifest_hash.clone(),
                dependencies: package
                    .dependencies
                    .values()
                    .map(|dep| LockDependency {
                        name: dep.name.clone(),
                        version: dep.version.clone(),
                        source: path_source(&workspace.root, &dep.path),
                        source_kind: dep.source.kind().to_string(),
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

impl PackageSource {
    fn kind(&self) -> &'static str {
        match self {
            PackageSource::Path { .. } => "path",
            PackageSource::Registry { .. } => "registry",
            PackageSource::Git { .. } => "git",
        }
    }

    fn git_url(&self) -> Option<String> {
        match self {
            PackageSource::Git { url, .. } => Some(url.clone()),
            _ => None,
        }
    }

    fn git_ref(&self) -> Option<String> {
        match self {
            PackageSource::Git { requested, .. } => Some(requested.clone()),
            _ => None,
        }
    }

    fn git_resolved(&self) -> Option<String> {
        match self {
            PackageSource::Git { resolved, .. } => Some(resolved.clone()),
            _ => None,
        }
    }
}

pub fn add_dependency(
    root: &Path,
    name: &str,
    version: Option<&str>,
    path: Option<&Path>,
    registry: Option<&Path>,
    git: Option<&str>,
    tag: Option<&str>,
    rev: Option<&str>,
    branch: Option<&str>,
    catalog: Option<&Path>,
) -> Result<PathBuf, PackageError> {
    let root = canonicalize_existing(root)?;
    let manifest_path = find_manifest(&root)?;
    let parsed = parse_package_request(name, version)?;
    let (dependency_name, dependency_version, _dependency_path, dependency_source) =
        if let Some(path) = path {
            let version = version.unwrap_or("0.1.0");
            (
                parsed.name,
                version.to_string(),
                relative_or_absolute(&root, path),
                DependencyManifestSource::Path(relative_or_absolute(&root, path)),
            )
        } else if let Some(registry) = registry {
            let version = parsed.version.as_deref().unwrap_or("0.1.0");
            let installed = install_from_registry(&root, registry, &parsed.name, version)?;
            (
                installed.canonical_name,
                installed.version,
                installed.path.clone(),
                DependencyManifestSource::Path(relative_or_absolute(&root, &installed.path)),
            )
        } else if let Some(git) = git {
            let installed = install_from_git(
                &root,
                &parsed.name,
                parsed.version.as_deref(),
                git,
                tag,
                rev,
                branch,
                false,
            )?;
            (
                installed.name,
                installed.version,
                installed.path.clone(),
                DependencyManifestSource::Git {
                    git: git.to_string(),
                    tag: tag.map(str::to_string),
                    rev: rev.map(str::to_string),
                    branch: branch.map(str::to_string),
                    checksum: Some(installed.checksum),
                },
            )
        } else {
            let entry =
                resolve_catalog_entry(&root, &parsed.name, parsed.version.as_deref(), catalog)?;
            let installed = install_from_git(
                &root,
                &entry.name,
                Some(&entry.version),
                &entry.git,
                entry.tag.as_deref(),
                entry.rev.as_deref(),
                entry.branch.as_deref(),
                false,
            )?;
            (
                installed.name,
                installed.version,
                installed.path.clone(),
                DependencyManifestSource::Git {
                    git: entry.git,
                    tag: entry.tag,
                    rev: entry.rev,
                    branch: entry.branch,
                    checksum: Some(installed.checksum),
                },
            )
        };

    if !is_valid_semver(&dependency_version) {
        return Err(PackageError::InvalidManifest {
            path: manifest_path.clone(),
            message: format!(
                "dependency '{}' has invalid semver '{}'",
                dependency_name, dependency_version
            ),
        });
    }
    write_dependency_to_manifest(
        &manifest_path,
        &dependency_name,
        &dependency_version,
        &dependency_source,
    )?;

    let workspace = resolve(&root)?;
    write_lockfile(&workspace)
}

#[derive(Clone, Debug)]
struct PackageRequest {
    name: String,
    version: Option<String>,
}

#[derive(Clone, Debug)]
enum DependencyManifestSource {
    Path(PathBuf),
    Git {
        git: String,
        tag: Option<String>,
        rev: Option<String>,
        branch: Option<String>,
        checksum: Option<String>,
    },
}

fn parse_package_request(
    name: &str,
    version: Option<&str>,
) -> Result<PackageRequest, PackageError> {
    if let Some((left, right)) = name.rsplit_once('@') {
        if !left.is_empty() && !right.is_empty() {
            return Ok(PackageRequest {
                name: left.to_string(),
                version: Some(right.to_string()),
            });
        }
    }
    Ok(PackageRequest {
        name: name.to_string(),
        version: version.map(str::to_string),
    })
}

fn write_dependency_to_manifest(
    manifest_path: &Path,
    dependency_name: &str,
    dependency_version: &str,
    source: &DependencyManifestSource,
) -> Result<(), PackageError> {
    let text = fs::read_to_string(manifest_path).map_err(|error| PackageError::Io {
        path: manifest_path.to_path_buf(),
        error,
    })?;
    let mut doc = text
        .parse::<DocumentMut>()
        .map_err(|error| PackageError::EditParse {
            path: manifest_path.to_path_buf(),
            message: error.to_string(),
        })?;

    if !doc.as_table().contains_key("dependencies") {
        doc["dependencies"] = Item::Table(Table::new());
    }

    let mut table = Table::new();
    table["version"] = value(dependency_version);
    match source {
        DependencyManifestSource::Path(path) => {
            table["path"] = value(path.to_string_lossy().replace('\\', "/"));
        }
        DependencyManifestSource::Git {
            git,
            tag,
            rev,
            branch,
            checksum,
        } => {
            table["git"] = value(git.as_str());
            if let Some(tag) = tag {
                table["tag"] = value(tag.as_str());
            }
            if let Some(rev) = rev {
                table["rev"] = value(rev.as_str());
            }
            if let Some(branch) = branch {
                table["branch"] = value(branch.as_str());
            }
            if let Some(checksum) = checksum {
                table["checksum"] = value(checksum.as_str());
            }
        }
    }
    doc["dependencies"][dependency_name] = Item::Table(table);
    fs::write(manifest_path, doc.to_string()).map_err(|error| PackageError::Io {
        path: manifest_path.to_path_buf(),
        error,
    })
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
        source_path: package.root.to_string_lossy().replace('\\', "/"),
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
    package_catalogs: BTreeMap<String, String>,
    dependency_specs: BTreeMap<String, DependencySpec>,
    manifest_hash: String,
}

fn collect_package(
    workspace_root: &Path,
    root: &Path,
    source: PackageSource,
    visited: &mut HashSet<PathBuf>,
    by_name: &mut BTreeMap<String, PathBuf>,
    ordered: &mut Vec<ResolvedPackage>,
    active: &mut Vec<String>,
) -> Result<(), PackageError> {
    let root = canonicalize_existing(root)?;
    let manifest_path = find_manifest(&root)?;
    let loaded = load_package(&manifest_path)?;
    validate_package_compatibility(&loaded.name, &loaded.release, &loaded.root)?;

    if let Some(index) = active.iter().position(|name| name == &loaded.name) {
        let mut chain = active[index..].to_vec();
        chain.push(loaded.name);
        return Err(PackageError::DependencyCycle(chain));
    }
    if !visited.insert(root.clone()) {
        return Ok(());
    }

    if let Some(first) = by_name.insert(loaded.name.clone(), root.clone()) {
        return Err(PackageError::DuplicatePackage {
            name: loaded.name,
            first,
            second: root,
        });
    }
    active.push(loaded.name.clone());

    let mut dependencies = BTreeMap::new();
    for (dep_name, spec) in &loaded.dependency_specs {
        let (dep_root, dep_source, expected_checksum) = match spec {
            DependencySpec::Detailed {
                path: Some(path), ..
            } => {
                let dep_root = canonicalize_existing(&root.join(path))?;
                (
                    dep_root.clone(),
                    PackageSource::Path { path: dep_root },
                    None,
                )
            }
            DependencySpec::Detailed {
                registry: Some(path),
                version,
                checksum,
                ..
            } => {
                let version = version.as_deref().unwrap_or("0.1.0");
                let installed =
                    install_from_registry(workspace_root, &root.join(path), dep_name, version)?;
                (
                    installed.path.clone(),
                    PackageSource::Registry {
                        path: installed.path,
                    },
                    checksum.clone(),
                )
            }
            DependencySpec::Detailed {
                git: Some(git),
                version,
                tag,
                rev,
                branch,
                checksum,
                ..
            } => {
                let installed = install_from_git(
                    workspace_root,
                    dep_name,
                    version.as_deref(),
                    git,
                    tag.as_deref(),
                    rev.as_deref(),
                    branch.as_deref(),
                    false,
                )?;
                (
                    installed.path.clone(),
                    PackageSource::Git {
                        url: git.clone(),
                        requested: git_requested_ref(
                            tag.as_deref(),
                            rev.as_deref(),
                            branch.as_deref(),
                        ),
                        resolved: installed.resolved,
                    },
                    checksum.clone().or(Some(installed.checksum)),
                )
            }
            DependencySpec::Version(_)
            | DependencySpec::Detailed {
                path: None,
                git: None,
                registry: None,
                ..
            } => {
                return Err(PackageError::InvalidManifest {
                    path: loaded.manifest.clone(),
                    message: format!(
                        "dependency '{}' must declare a local path, registry path, or git source",
                        dep_name
                    ),
                });
            }
        };
        collect_package(
            workspace_root,
            &dep_root,
            dep_source.clone(),
            visited,
            by_name,
            ordered,
            active,
        )?;
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
                source: dep_source,
            },
        );
        if let Some(expected) = expected_checksum {
            let actual = directory_checksum(&dependencies[dep_name].path)?;
            if actual != expected {
                return Err(PackageError::Registry(format!(
                    "checksum mismatch for dependency '{}'",
                    dep_name
                )));
            }
        }
    }
    let checksum = match &source {
        PackageSource::Path { .. } => loaded.manifest_hash.clone(),
        PackageSource::Registry { .. } | PackageSource::Git { .. } => {
            directory_checksum(&loaded.root)?
        }
    };

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
        source,
        checksum,
    });

    active.pop();

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
            let mut chain = remaining.keys().cloned().collect::<Vec<_>>();
            if let Some(first) = chain.first().cloned() {
                chain.push(first);
            }
            return Err(PackageError::DependencyCycle(chain));
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
        package_catalogs: manifest.package.catalogs,
        dependency_specs: manifest.dependencies,
        manifest_hash: stable_hash_hex(text.as_bytes()),
    })
}

fn validate_package_compatibility(
    name: &str,
    release: &ReleaseMetadata,
    root: &Path,
) -> Result<(), PackageError> {
    let required = cli_compatibility_level();
    if release.compatibility != required {
        return Err(PackageError::IncompatiblePackage {
            name: name.to_string(),
            required: required.to_string(),
            found: release.compatibility.clone(),
            source: root.display().to_string(),
        });
    }
    Ok(())
}

fn install_from_registry(
    root: &Path,
    registry: &Path,
    name: &str,
    version: &str,
) -> Result<InstalledRegistryPackage, PackageError> {
    let registry_package = registry_package_dir(registry, name, version);
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
    if metadata.version != version {
        return Err(PackageError::Registry(format!(
            "registry metadata version '{}' does not match requested version '{}'",
            metadata.version, version
        )));
    }
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
    Ok(InstalledRegistryPackage {
        canonical_name: metadata.name,
        version: metadata.version,
        path: vendor_dir,
    })
}

fn install_from_git(
    workspace_root: &Path,
    name: &str,
    requested_version: Option<&str>,
    git: &str,
    tag: Option<&str>,
    rev: Option<&str>,
    branch: Option<&str>,
    offline: bool,
) -> Result<InstalledGitPackage, PackageError> {
    let cache_root = workspace_root.join(".spectra").join("git");
    let clone_dir = cache_root.join(sanitize_package_component(name));
    if !clone_dir.join(".git").is_dir() {
        if offline {
            return Err(PackageError::Registry(format!(
                "package '{}' is not in git cache and --offline was used",
                name
            )));
        }
        fs::create_dir_all(&cache_root).map_err(|error| PackageError::Io {
            path: cache_root.clone(),
            error,
        })?;
        let clone_target = git_path_arg(&clone_dir);
        run_git(
            &["clone", "--quiet", git, clone_target.as_str()],
            workspace_root,
        )?;
    } else if !offline {
        run_git(&["fetch", "--quiet", "--tags", "--force"], &clone_dir)?;
    }

    if let Some(rev) = rev {
        run_git(&["checkout", "--quiet", rev], &clone_dir)?;
    } else if let Some(tag) = tag {
        run_git(&["checkout", "--quiet", tag], &clone_dir)?;
    } else if let Some(branch) = branch {
        run_git(&["checkout", "--quiet", branch], &clone_dir)?;
    }

    let resolved = git_output(&["rev-parse", "HEAD"], &clone_dir)?;
    let manifest_path = find_manifest(&clone_dir)?;
    let loaded = load_package(&manifest_path)?;
    validate_package_compatibility(&loaded.name, &loaded.release, &clone_dir)?;
    let version = requested_version
        .map(str::to_string)
        .unwrap_or_else(|| loaded.version.clone());
    if loaded.name != name {
        return Err(PackageError::Registry(format!(
            "git package manifest name '{}' does not match requested '{}'",
            loaded.name, name
        )));
    }
    if loaded.version != version {
        return Err(PackageError::Registry(format!(
            "git package '{}' version '{}' does not match requested '{}'",
            name, loaded.version, version
        )));
    }

    let vendor_dir = workspace_root
        .join(".spectra")
        .join("packages")
        .join(format!("{}-{}", sanitize_package_component(name), version));
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
    copy_package_payload(&clone_dir, &vendor_dir)?;
    let checksum = directory_checksum(&vendor_dir)?;
    Ok(InstalledGitPackage {
        name: loaded.name,
        version,
        path: vendor_dir,
        resolved,
        checksum,
    })
}

fn git_path_arg(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("//?/").unwrap_or(&text).to_string()
}

fn run_git(args: &[&str], cwd: &Path) -> Result<(), PackageError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| PackageError::Registry(format!("failed to launch git: {}", error)))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(PackageError::Registry(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn git_output(args: &[&str], cwd: &Path) -> Result<String, PackageError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| PackageError::Registry(format!("failed to launch git: {}", error)))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(PackageError::Registry(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn git_requested_ref(tag: Option<&str>, rev: Option<&str>, branch: Option<&str>) -> String {
    if let Some(rev) = rev {
        format!("rev:{}", rev)
    } else if let Some(tag) = tag {
        format!("tag:{}", tag)
    } else if let Some(branch) = branch {
        format!("branch:{}", branch)
    } else {
        "HEAD".to_string()
    }
}

fn ensure_catalog_publication_ref(
    tag: Option<&str>,
    rev: Option<&str>,
    branch: Option<&str>,
) -> Result<(), PackageError> {
    let ref_count = [tag, rev, branch]
        .iter()
        .filter(|value| value.is_some())
        .count();
    if ref_count != 1 {
        return Err(PackageError::Registry(
            "catalog publication requires exactly one of --tag or --rev; branch-only refs are not accepted"
                .to_string(),
        ));
    }
    if branch.is_some() {
        return Err(PackageError::Registry(
            "catalog publication requires --tag or --rev; branch refs are mutable".to_string(),
        ));
    }
    Ok(())
}

fn resolve_catalog_publication_rev(
    root: &Path,
    tag: Option<&str>,
    rev: Option<&str>,
) -> Result<String, PackageError> {
    let requested = tag.or(rev).ok_or_else(|| {
        PackageError::Registry("catalog publication requires --tag or --rev".to_string())
    })?;
    let resolved = git_output(&["rev-parse", &format!("{}^{{commit}}", requested)], root)?;
    let head = git_output(&["rev-parse", "HEAD"], root)?;
    if resolved != head {
        return Err(PackageError::Registry(format!(
            "catalog ref '{}' resolves to {}, but package root is checked out at {}",
            requested, resolved, head
        )));
    }
    Ok(resolved)
}

fn validate_catalog_package(package: &CatalogPackage, publication: bool) -> Result<(), String> {
    if !is_valid_package_name(&package.name) {
        return Err(format!("package name '{}' is invalid", package.name));
    }
    if !is_valid_semver(&package.version) {
        return Err(format!(
            "package '{}' has invalid semver '{}'",
            package.name, package.version
        ));
    }
    validate_clean_text("git", &package.git)?;
    if package.git.trim().is_empty() {
        return Err(format!("package '{}' has empty git URL", package.name));
    }
    if !is_allowed_git_locator(&package.git) {
        return Err(format!(
            "package '{}' uses unsupported git locator '{}'",
            package.name, package.git
        ));
    }

    let ref_count = [&package.tag, &package.rev, &package.branch]
        .iter()
        .filter(|value| value.is_some())
        .count();
    if ref_count > 1 {
        return Err(format!(
            "package '{}' declares more than one git ref",
            package.name
        ));
    }
    if publication && package.tag.is_none() && package.rev.is_none() {
        return Err(format!(
            "package '{}' must publish with immutable tag or rev",
            package.name
        ));
    }
    if publication && package.branch.is_some() {
        return Err(format!(
            "package '{}' cannot publish mutable branch refs",
            package.name
        ));
    }
    if let Some(tag) = &package.tag {
        validate_clean_text("tag", tag)?;
        let plain = package.version.as_str();
        let prefixed = format!("v{}", package.version);
        if publication && tag != plain && tag != &prefixed {
            return Err(format!(
                "package '{}' tag '{}' must match version '{}' or 'v{}'",
                package.name, tag, package.version, package.version
            ));
        }
    }
    if let Some(rev) = &package.rev {
        validate_git_ref_text("rev", rev)?;
        if publication && !is_hex_sha(rev, 7) {
            return Err(format!(
                "package '{}' publication rev must be a commit SHA",
                package.name
            ));
        }
    }
    if let Some(branch) = &package.branch {
        validate_git_ref_text("branch", branch)?;
    }
    if let Some(resolved) = &package.resolved_rev {
        if !is_hex_sha(resolved, 40) {
            return Err(format!(
                "package '{}' resolved_rev must be a commit SHA",
                package.name
            ));
        }
    } else if publication {
        return Err(format!(
            "package '{}' publication metadata missing resolved_rev",
            package.name
        ));
    }
    if let Some(checksum) = &package.checksum {
        if !is_hex_sha(checksum, 64) {
            return Err(format!(
                "package '{}' checksum must be a SHA-256 hex digest",
                package.name
            ));
        }
    } else if publication {
        return Err(format!(
            "package '{}' metadata missing checksum",
            package.name
        ));
    }

    validate_clean_text("description", &package.description)?;
    validate_clean_text("compatibility", &package.compatibility)?;
    validate_clean_text("license", &package.license)?;
    validate_clean_text("owner", &package.owner)?;
    for keyword in &package.keywords {
        validate_clean_text("keyword", keyword)?;
    }
    if publication && package.modules.is_empty() {
        return Err(format!(
            "package '{}' must export at least one module",
            package.name
        ));
    }
    for module in &package.modules {
        if !is_valid_package_name(module) {
            return Err(format!(
                "package '{}' exports invalid module '{}'",
                package.name, module
            ));
        }
        if module != &package.name && !module.starts_with(&format!("{}.", package.name)) {
            return Err(format!(
                "package '{}' exports module '{}' outside its namespace",
                package.name, module
            ));
        }
    }
    Ok(())
}

fn validate_clean_text(field: &str, value: &str) -> Result<(), String> {
    if value.chars().any(|ch| ch.is_control()) {
        Err(format!("{} contains control characters", field))
    } else {
        Ok(())
    }
}

fn validate_git_ref_text(field: &str, value: &str) -> Result<(), String> {
    validate_clean_text(field, value)?;
    if value.is_empty()
        || value.starts_with('-')
        || value.contains("..")
        || value.contains("@{")
        || value.ends_with(".lock")
        || value.ends_with('/')
        || value
            .chars()
            .any(|ch| matches!(ch, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\'))
    {
        Err(format!("{} '{}' is not a safe git ref", field, value))
    } else {
        Ok(())
    }
}

fn is_allowed_git_locator(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    if lowered.starts_with("https://")
        || lowered.starts_with("ssh://")
        || (value.starts_with("git@") && value.contains(':'))
    {
        return true;
    }
    if Path::new(value)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return false;
    }
    Path::new(value).is_absolute()
        || value.starts_with("./")
        || value.starts_with(".\\")
        || value.contains('/')
        || value.contains('\\')
}

fn is_hex_sha(value: &str, min_len: usize) -> bool {
    value.len() >= min_len && value.len() <= 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn is_valid_package_name(value: &str) -> bool {
    !value.is_empty()
        && value.split('.').all(|part| {
            let mut chars = part.chars();
            matches!(chars.next(), Some(ch) if ch.is_ascii_alphabetic() || ch == '_')
                && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        })
}

fn catalog_entries_match(left: &CatalogPackage, right: &CatalogPackage) -> bool {
    left.git == right.git
        && left.tag == right.tag
        && left.rev == right.rev
        && left.branch == right.branch
        && (left.resolved_rev == right.resolved_rev
            || left.resolved_rev.is_none()
            || right.resolved_rev.is_none())
        && left.checksum == right.checksum
        && left.modules == right.modules
        && left.compatibility == right.compatibility
}

fn sanitize_package_component(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn catalog_paths(root: &Path, explicit: Option<&Path>) -> Result<Vec<PathBuf>, PackageError> {
    if let Some(explicit) = explicit {
        return Ok(vec![catalog_index_path(explicit)]);
    }
    let mut paths = Vec::new();
    let manifest = find_manifest(root).ok();
    if let Some(manifest_path) = manifest {
        let loaded = load_package(&manifest_path)?;
        for value in loaded.package_catalogs.values() {
            paths.push(catalog_index_path(&root.join(value)));
        }
    }
    paths.push(
        root.join(".spectra")
            .join("catalogs")
            .join("package.index.toml"),
    );
    if let Some(home) = env::var_os("USERPROFILE").or_else(|| env::var_os("HOME")) {
        paths.push(
            PathBuf::from(home)
                .join(".spectra")
                .join("catalogs")
                .join("spectralang-official")
                .join("package.index.toml"),
        );
    }
    Ok(paths)
}

fn catalog_index_path(path: &Path) -> PathBuf {
    if path.is_dir() || path.extension().is_none() {
        path.join("package.index.toml")
    } else {
        path.to_path_buf()
    }
}

fn load_catalogs(
    root: &Path,
    explicit: Option<&Path>,
) -> Result<Vec<CatalogPackage>, PackageError> {
    let mut packages = Vec::new();
    for path in catalog_paths(root, explicit)? {
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|error| PackageError::Io {
            path: path.clone(),
            error,
        })?;
        let catalog: CatalogIndex = toml::from_str(&text).map_err(|error| PackageError::Parse {
            path: path.clone(),
            error,
        })?;
        for package in &catalog.packages {
            validate_catalog_package(package, false).map_err(|message| {
                PackageError::Registry(format!(
                    "catalog '{}' rejected: {}",
                    path.display(),
                    message
                ))
            })?;
        }
        packages.extend(catalog.packages.into_iter().map(|mut package| {
            package.origin = path.display().to_string();
            package
        }));
    }
    Ok(packages)
}

fn resolve_catalog_entry(
    root: &Path,
    name: &str,
    version: Option<&str>,
    explicit: Option<&Path>,
) -> Result<CatalogPackage, PackageError> {
    let packages = load_catalogs(root, explicit)?;
    let matches = packages
        .into_iter()
        .filter(|package| package.name == name)
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err(PackageError::MissingPackage(name.to_string()));
    }

    let mut unique = BTreeMap::<String, CatalogPackage>::new();
    for package in matches {
        if let Some(existing) = unique.get(&package.version) {
            if !catalog_entries_match(existing, &package) {
                return Err(PackageError::ConflictingCatalogPackage {
                    name: name.to_string(),
                    version: package.version,
                    first: existing.origin.clone(),
                    second: package.origin,
                });
            }
            continue;
        }
        unique.insert(package.version.clone(), package);
    }

    let requested = version.map(str::to_string);
    if let Some(requested_version) = requested.as_deref() {
        if !unique.contains_key(requested_version) {
            return Err(PackageError::MissingPackage(format!(
                "{}@{}",
                name, requested_version
            )));
        }
    }
    let mut compatible = unique
        .into_values()
        .filter(|package| {
            requested
                .as_deref()
                .map_or(true, |requested| package.version == requested)
        })
        .filter(|package| package.compatibility == cli_compatibility_level())
        .collect::<Vec<_>>();

    if compatible.is_empty() {
        let requested_version = requested.as_deref().unwrap_or("any");
        let found = if requested_version == "any" {
            "no compatible catalog entry".to_string()
        } else {
            load_catalogs(root, explicit)?
                .into_iter()
                .find(|package| package.name == name && package.version == requested_version)
                .map(|package| package.compatibility)
                .unwrap_or_else(|| "no compatible catalog entry".to_string())
        };
        return Err(PackageError::IncompatiblePackage {
            name: name.to_string(),
            required: cli_compatibility_level().to_string(),
            found,
            source: requested_version.to_string(),
        });
    }

    compatible.sort_by(|left, right| compare_versions(&left.version, &right.version));
    Ok(compatible
        .pop()
        .expect("non-empty compatible catalog matches"))
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    match (Version::parse(left), Version::parse(right)) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

pub fn search(
    root: &Path,
    query: &str,
    catalog: Option<&Path>,
) -> Result<Vec<String>, PackageError> {
    let root = canonicalize_existing(root)?;
    let query = query.to_ascii_lowercase();
    let mut rows = Vec::new();
    for package in load_catalogs(&root, catalog)? {
        let haystack = format!(
            "{} {} {} {}",
            package.name,
            package.description,
            package.keywords.join(" "),
            package.owner
        )
        .to_ascii_lowercase();
        if haystack.contains(&query) {
            rows.push(format!(
                "{} {} {}",
                package.name, package.version, package.description
            ));
        }
    }
    rows.sort();
    rows.dedup();
    Ok(rows)
}

pub fn info(root: &Path, name: &str, catalog: Option<&Path>) -> Result<Vec<String>, PackageError> {
    let root = canonicalize_existing(root)?;
    let packages = load_catalogs(&root, catalog)?
        .into_iter()
        .filter(|package| package.name == name)
        .collect::<Vec<_>>();
    if packages.is_empty() {
        return Err(PackageError::MissingPackage(name.to_string()));
    }
    let mut rows = Vec::new();
    for package in packages {
        rows.push(format!(
            "{} {}\ngit: {}\nref: {}\nresolved: {}\ncompatibility: {}\nlicense: {}\nkeywords: {}\nmodules: {}",
            package.name,
            package.version,
            package.git,
            git_requested_ref(package.tag.as_deref(), package.rev.as_deref(), package.branch.as_deref()),
            package.resolved_rev.as_deref().unwrap_or("<unresolved>"),
            package.compatibility,
            package.license,
            package.keywords.join(", "),
            package.modules.join(", ")
        ));
    }
    Ok(rows)
}

pub fn versions(
    root: &Path,
    name: &str,
    catalog: Option<&Path>,
) -> Result<Vec<String>, PackageError> {
    let root = canonicalize_existing(root)?;
    let mut rows = load_catalogs(&root, catalog)?
        .into_iter()
        .filter(|package| package.name == name)
        .map(|package| package.version)
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| compare_versions(left, right));
    rows.dedup();
    if rows.is_empty() {
        return Err(PackageError::MissingPackage(name.to_string()));
    }
    Ok(rows)
}

pub fn write_metadata(
    root: &Path,
    out: &Path,
    git: Option<&str>,
    tag: Option<&str>,
    rev: Option<&str>,
    branch: Option<&str>,
) -> Result<PathBuf, PackageError> {
    let root = canonicalize_existing(root)?;
    let manifest = load_package(&find_manifest(&root)?)?;
    ensure_catalog_publication_ref(tag, rev, branch)?;
    let resolved_rev = resolve_catalog_publication_rev(&root, tag, rev)?;
    let entry = CatalogPackage {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        git: git
            .ok_or_else(|| {
                PackageError::Registry(
                    "publish-metadata requires --git for catalog publication".to_string(),
                )
            })?
            .to_string(),
        tag: tag.map(str::to_string),
        rev: rev.map(str::to_string),
        branch: branch.map(str::to_string),
        resolved_rev: Some(resolved_rev),
        checksum: Some(directory_checksum(&root)?),
        description: String::new(),
        keywords: Vec::new(),
        compatibility: manifest.release.compatibility,
        license: String::new(),
        modules: exported_modules(&manifest.src_dirs)?,
        owner: String::new(),
        origin: String::new(),
    };
    validate_catalog_package(&entry, true).map_err(|message| {
        PackageError::Registry(format!("refusing to publish package metadata: {}", message))
    })?;
    let index = CatalogIndex {
        schema: catalog_schema(),
        packages: vec![entry],
    };
    let text = toml::to_string_pretty(&index).map_err(PackageError::Serialize)?;
    let path = out.to_path_buf();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| PackageError::Io {
            path: parent.to_path_buf(),
            error,
        })?;
    }
    fs::write(&path, text).map_err(|error| PackageError::Io {
        path: path.clone(),
        error,
    })?;
    Ok(path)
}

pub fn register(
    root: &Path,
    catalog: &Path,
    git: &str,
    tag: Option<&str>,
    rev: Option<&str>,
    branch: Option<&str>,
) -> Result<PathBuf, PackageError> {
    let root = canonicalize_existing(root)?;
    let manifest = load_package(&find_manifest(&root)?)?;
    ensure_catalog_publication_ref(tag, rev, branch)?;
    let resolved_rev = resolve_catalog_publication_rev(&root, tag, rev)?;
    let catalog_path = catalog_index_path(catalog);
    let mut index = if catalog_path.is_file() {
        let text = fs::read_to_string(&catalog_path).map_err(|error| PackageError::Io {
            path: catalog_path.clone(),
            error,
        })?;
        toml::from_str::<CatalogIndex>(&text).map_err(|error| PackageError::Parse {
            path: catalog_path.clone(),
            error,
        })?
    } else {
        CatalogIndex {
            schema: catalog_schema(),
            packages: Vec::new(),
        }
    };
    for package in &index.packages {
        validate_catalog_package(package, false).map_err(|message| {
            PackageError::Registry(format!(
                "catalog '{}' rejected: {}",
                catalog_path.display(),
                message
            ))
        })?;
    }
    let entry = CatalogPackage {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        git: git.to_string(),
        tag: tag.map(str::to_string),
        rev: rev.map(str::to_string),
        branch: branch.map(str::to_string),
        resolved_rev: Some(resolved_rev),
        checksum: Some(directory_checksum(&root)?),
        description: String::new(),
        keywords: Vec::new(),
        compatibility: manifest.release.compatibility,
        license: String::new(),
        modules: exported_modules(&manifest.src_dirs)?,
        owner: String::new(),
        origin: String::new(),
    };
    validate_catalog_package(&entry, true).map_err(|message| {
        PackageError::Registry(format!("refusing to register package: {}", message))
    })?;
    for existing in &index.packages {
        if existing.name == entry.name
            && existing.version == entry.version
            && !catalog_entries_match(existing, &entry)
        {
            return Err(PackageError::Registry(format!(
                "refusing to overwrite existing catalog entry '{} {}' with different source metadata",
                entry.name, entry.version
            )));
        }
    }
    index
        .packages
        .retain(|pkg| !(pkg.name == entry.name && pkg.version == entry.version));
    index.packages.push(entry);
    index.packages.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| compare_versions(&left.version, &right.version))
    });
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent).map_err(|error| PackageError::Io {
            path: parent.to_path_buf(),
            error,
        })?;
    }
    let text = toml::to_string_pretty(&index).map_err(PackageError::Serialize)?;
    fs::write(&catalog_path, text).map_err(|error| PackageError::Io {
        path: catalog_path.clone(),
        error,
    })?;
    Ok(catalog_path)
}

fn exported_modules(src_dirs: &[PathBuf]) -> Result<Vec<String>, PackageError> {
    let mut modules = Vec::new();
    for src in src_dirs {
        if !src.is_dir() {
            continue;
        }
        for source in discovery::discover_sources(&[src.clone()]) {
            let text = fs::read_to_string(&source).map_err(|error| PackageError::Io {
                path: source.clone(),
                error,
            })?;
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("module ") {
                    let module = rest.trim_end_matches(';').trim();
                    if !module.is_empty() {
                        modules.push(module.to_string());
                    }
                    break;
                }
            }
        }
    }
    modules.sort();
    modules.dedup();
    Ok(modules)
}

pub fn fetch(root: &Path, offline: bool) -> Result<PathBuf, PackageError> {
    let workspace = resolve(root)?;
    if offline {
        for package in &workspace.packages {
            if matches!(package.source, PackageSource::Git { .. }) && !package.root.is_dir() {
                return Err(PackageError::Registry(format!(
                    "package '{}' missing from offline cache",
                    package.name
                )));
            }
        }
    }
    write_lockfile(&workspace)
}

pub fn dependency_tree(root: &Path) -> Result<Vec<String>, PackageError> {
    let workspace = resolve(root)?;
    let mut rows = Vec::new();
    for package in workspace.packages {
        if package.dependencies.is_empty() {
            rows.push(format!(
                "{} {} ({})",
                package.name,
                package.version,
                package.source.kind()
            ));
        } else {
            for dep in package.dependencies.values() {
                rows.push(format!(
                    "{} {} -> {} {} ({})",
                    package.name,
                    package.version,
                    dep.name,
                    dep.version,
                    dep.source.kind()
                ));
            }
        }
    }
    Ok(rows)
}

fn registry_package_dir(registry: &Path, name: &str, version: &str) -> PathBuf {
    let exact = registry.join(name).join(version);
    if exact.join("package.toml").is_file() {
        return exact;
    }

    if name.contains('-') {
        let dotted = name.replace('-', ".");
        let alias = registry.join(dotted).join(version);
        if alias.join("package.toml").is_file() {
            return alias;
        }
    }

    exact
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

#[cfg(test)]
fn toml_key(name: &str) -> String {
    if name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        name.to_string()
    } else {
        format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""))
    }
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
        if matches!(
            name_text.as_ref(),
            "target" | ".git" | ".spectra" | LOCKFILE_NAME
        ) {
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
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if matches!(name_text.as_ref(), "target" | ".git" | ".spectra") {
            continue;
        }
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
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
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
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn toml_key_quotes_dotted_package_names() {
        assert_eq!(toml_key("spectra-api"), "spectra-api");
        assert_eq!(toml_key("spectra.api"), "\"spectra.api\"");
    }

    fn temp_catalog(contents: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("spectralang-r905-{}.toml", nonce));
        fs::write(&path, contents).expect("catalog write");
        path
    }

    #[test]
    fn catalog_resolution_deduplicates_and_selects_highest_compatible_version() {
        let catalog = temp_catalog(
            r#"
schema = "spectra-package-catalog-v1"

[[packages]]
name = "demo"
version = "1.0.0"
git = "C:\\packages\\demo-1"
compatibility = "spectralang-0.1"

[[packages]]
name = "demo"
version = "1.1.0"
git = "C:\\packages\\demo-11"
compatibility = "spectralang-0.1"

[[packages]]
name = "demo"
version = "1.1.0"
git = "C:\\packages\\demo-11"
compatibility = "spectralang-0.1"

[[packages]]
name = "demo"
version = "2.0.0"
git = "C:\\packages\\demo-2"
compatibility = "spectralang-0.2"
"#,
        );
        let resolved = resolve_catalog_entry(Path::new("."), "demo", None, Some(&catalog))
            .expect("compatible catalog entry");
        assert_eq!(resolved.version, "1.1.0");
        assert_eq!(resolved.git, "C:\\packages\\demo-11");
        let _ = fs::remove_file(catalog);
    }

    #[test]
    fn catalog_resolution_reports_conflicting_same_version_entries() {
        let catalog = temp_catalog(
            r#"
[[packages]]
name = "demo"
version = "1.0.0"
git = "C:\\packages\\one"
compatibility = "spectralang-0.1"

[[packages]]
name = "demo"
version = "1.0.0"
git = "C:\\packages\\two"
compatibility = "spectralang-0.1"
"#,
        );
        let error = resolve_catalog_entry(Path::new("."), "demo", None, Some(&catalog))
            .expect_err("conflicting entries must fail");
        assert!(error.to_string().contains("version '1.0.0' conflict"));
        let _ = fs::remove_file(catalog);
    }

    #[test]
    fn package_diagnostics_include_origins_and_cycle_chain() {
        let duplicate = PackageError::DuplicatePackage {
            name: "demo".to_string(),
            first: PathBuf::from("one"),
            second: PathBuf::from("two"),
        };
        assert!(duplicate.to_string().contains("'one' and 'two'"));
        let cycle =
            PackageError::DependencyCycle(vec!["a".to_string(), "b".to_string(), "a".to_string()]);
        assert!(cycle.to_string().contains("a -> b -> a"));
    }
}
