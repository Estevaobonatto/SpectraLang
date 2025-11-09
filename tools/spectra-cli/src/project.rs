use spectra_compiler::{
    CompilationOptions,
    ModuleResolutionError,
    ModuleResolver,
    ModuleResolverOptions,
};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct ResolvedModule {
    pub name: String,
    pub path: PathBuf,
    pub imports: Vec<String>,
}

#[derive(Debug)]
pub struct ProjectPlan {
    modules: Vec<ResolvedModule>,
}

impl ProjectPlan {
    pub fn build(entries: Vec<PathBuf>, options: &CompilationOptions) -> Result<Self, ProjectError> {
        if entries.is_empty() {
            return Ok(Self {
                modules: Vec::new(),
            });
        }

        let mut discovered: HashSet<PathBuf> = HashSet::new();
        let mut entry_paths: Vec<PathBuf> = Vec::new();

        for entry in entries {
            let normalized =
                normalize_path(&entry).map_err(|error| ProjectError::Io { path: entry, error })?;
            entry_paths.push(normalized.clone());
            collect_sources(&normalized, &mut discovered)?;
        }

        if discovered.is_empty() {
            return Err(ProjectError::NoSourcesFound(entry_paths));
        }

        let mut search_roots: Vec<PathBuf> = discovered
            .iter()
            .filter_map(|path| path.parent().map(|parent| parent.to_path_buf()))
            .collect();
        search_roots.sort();
        search_roots.dedup();

        let mut resolver = ModuleResolver::new(ModuleResolverOptions {
            roots: search_roots,
            experimental_features: options.experimental_features.clone(),
        });

        let mut files: Vec<PathBuf> = discovered.into_iter().collect();
        files.sort();

        let mut modules = Vec::new();
        let mut seen_modules: HashMap<String, PathBuf> = HashMap::new();

        for entry in files {
            let graph = resolver
                .resolve(&entry)
                .map_err(|error| ProjectError::ModuleResolution {
                    entry: entry.clone(),
                    error,
                })?;

            for module in graph.modules() {
                if let Some(existing_path) = seen_modules.get(&module.name) {
                    if existing_path != &module.path {
                        return Err(ProjectError::ModuleResolution {
                            entry: entry.clone(),
                            error: ModuleResolutionError::DuplicateModule {
                                module: module.name.clone(),
                                existing: existing_path.clone(),
                                duplicate: module.path.clone(),
                            },
                        });
                    }
                    continue;
                }

                seen_modules.insert(module.name.clone(), module.path.clone());

                let imports = module
                    .imports
                    .iter()
                    .filter(|import| !import.is_builtin)
                    .map(|import| import.module.clone())
                    .collect();

                modules.push(ResolvedModule {
                    name: module.name.clone(),
                    path: module.path.clone(),
                    imports,
                });
            }
        }

        Ok(Self { modules })
    }

    pub fn modules(&self) -> &[ResolvedModule] {
        &self.modules
    }
}

#[derive(Debug)]
pub enum ProjectError {
    Io {
        path: PathBuf,
        error: io::Error,
    },
    NoSourcesFound(Vec<PathBuf>),
    ModuleResolution {
        entry: PathBuf,
        error: ModuleResolutionError,
    },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Io { path, error } => {
                write!(f, "failed to read '{}': {}", path.display(), error)
            }
            ProjectError::NoSourcesFound(paths) => {
                writeln!(f, "no Spectra source files found in the given paths:")?;
                for path in paths {
                    writeln!(f, "  • {}", path.display())?;
                }
                Ok(())
            }
            ProjectError::ModuleResolution { entry, error } => {
                write_resolution_error(f, entry, error)
            }
        }
    }
}

impl std::error::Error for ProjectError {}

fn collect_sources(path: &Path, out: &mut HashSet<PathBuf>) -> Result<(), ProjectError> {
    let metadata = fs::metadata(path).map_err(|error| ProjectError::Io {
        path: path.to_path_buf(),
        error,
    })?;

    if metadata.is_dir() {
        if should_skip_directory(path) {
            return Ok(());
        }
        for entry in fs::read_dir(path).map_err(|error| ProjectError::Io {
            path: path.to_path_buf(),
            error,
        })? {
            let entry = entry.map_err(|error| ProjectError::Io {
                path: path.to_path_buf(),
                error,
            })?;
            let child_path = entry.path();
            collect_sources(&child_path, out)?;
        }
    } else if metadata.is_file() {
        if is_source_file(path) {
            let normalized = normalize_path(path).map_err(|error| ProjectError::Io {
                path: path.to_path_buf(),
                error,
            })?;
            out.insert(normalized);
        }
    }

    Ok(())
}

fn is_source_file(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("spectra") => true,
        Some(ext) if ext.eq_ignore_ascii_case("spc") => true,
        _ => false,
    }
}

fn should_skip_directory(path: &Path) -> bool {
    match path.file_name().and_then(|name| name.to_str()) {
        Some(name) if name.starts_with('.') => true,
        Some(name) if matches!(name, "target" | "build" | "dist" | "out") => true,
        _ => false,
    }
}

fn normalize_path(path: &Path) -> Result<PathBuf, io::Error> {
    fs::canonicalize(path)
}

fn write_resolution_error(
    f: &mut fmt::Formatter<'_>,
    entry: &Path,
    error: &ModuleResolutionError,
) -> fmt::Result {
    match error {
        ModuleResolutionError::Io { path, error } => {
            write!(
                f,
                "failed to read '{}' while resolving module graph for '{}': {}",
                path.display(),
                entry.display(),
                error
            )
        }
        ModuleResolutionError::ModuleNotFound { module, searched } => {
            write!(
                f,
                "module '{}' imported while resolving '{}' was not found",
                module,
                entry.display()
            )?;
            if !searched.is_empty() {
                let mut candidates: Vec<String> = searched
                    .iter()
                    .map(|candidate| candidate.to_string_lossy().into_owned())
                    .collect();
                candidates.sort();
                candidates.dedup();
                write!(f, " (searched: {})", candidates.join(", "))?;
            }
            Ok(())
        }
        ModuleResolutionError::ModuleHeaderMismatch {
            expected,
            found,
            path,
        } => write!(
            f,
            "module header mismatch in '{}': expected '{}', found '{}'",
            path.display(),
            expected,
            found
        ),
        ModuleResolutionError::DuplicateModule {
            module,
            existing,
            duplicate,
        } => write!(
            f,
            "module '{}' resolves to both '{}' and '{}'",
            module,
            existing.display(),
            duplicate.display()
        ),
        ModuleResolutionError::ParseFailure { path, errors } => {
            if errors.is_empty() {
                write!(f, "failed to parse '{}': unknown parser error", path.display())
            } else {
                write!(f, "failed to parse '{}': {}", path.display(), errors[0])?;
                if errors.len() > 1 {
                    let remaining = errors.len() - 1;
                    write!(
                        f,
                        " ({} additional error{})",
                        remaining,
                        if remaining == 1 { "" } else { "s" }
                    )?;
                }
                Ok(())
            }
        }
        ModuleResolutionError::Cycle { cycle } => {
            write!(
                f,
                "detected module dependency cycle while resolving '{}': {}",
                entry.display(),
                cycle.join(" -> ")
            )
        }
    }
}
