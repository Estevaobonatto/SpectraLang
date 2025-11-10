use spectra_compiler::{
    analyze_graph, CompilationOptions, CompilerError, ModuleGraph, ModuleResolutionError,
    ModuleResolver, ModuleResolverOptions,
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
    pub exports: Vec<String>,
}

pub struct ProjectPlan {
    modules: Vec<ResolvedModule>,
    graphs: Vec<ModuleGraph>,
}

impl ProjectPlan {
    pub fn build(
        entries: Vec<PathBuf>,
        options: &CompilationOptions,
    ) -> Result<Self, ProjectError> {
        if entries.is_empty() {
            return Ok(Self {
                modules: Vec::new(),
                graphs: Vec::new(),
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

        for lib in &options.library_paths {
            let canonical = normalize_path(lib).map_err(|error| ProjectError::Io {
                path: lib.clone(),
                error,
            })?;
            let metadata = fs::metadata(&canonical).map_err(|error| ProjectError::Io {
                path: lib.clone(),
                error,
            })?;
            if !metadata.is_dir() {
                return Err(ProjectError::LibraryRootNotDirectory { path: lib.clone() });
            }
            search_roots.push(canonical);
        }

        search_roots.sort();
        search_roots.dedup();

        let mut resolver = ModuleResolver::new(ModuleResolverOptions {
            roots: search_roots,
            experimental_features: options.experimental_features.clone(),
        });

        let mut files: Vec<PathBuf> = discovered.into_iter().collect();
        files.sort();

        let mut modules = Vec::new();
        let mut graphs = Vec::new();
        let mut seen_modules: HashMap<String, PathBuf> = HashMap::new();

        for entry in files {
            let graph =
                resolver
                    .resolve(&entry)
                    .map_err(|error| ProjectError::ModuleResolution {
                        entry: entry.clone(),
                        error,
                    })?;

            let mut added_any = false;
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

                let exports = module
                    .exports
                    .iter()
                    .map(|export| export.name.clone())
                    .collect();

                modules.push(ResolvedModule {
                    name: module.name.clone(),
                    path: module.path.clone(),
                    imports,
                    exports,
                });
                added_any = true;
            }

            if added_any {
                graphs.push(graph);
            }
        }

        Ok(Self { modules, graphs })
    }

    pub fn modules(&self) -> &[ResolvedModule] {
        &self.modules
    }

    pub fn analyze_semantics(&mut self) -> Result<(), Vec<CompilerError>> {
        let mut errors = Vec::new();

        for graph in &mut self.graphs {
            if let Err(semantic_errors) = analyze_graph(graph) {
                errors.extend(semantic_errors.into_iter().map(CompilerError::Semantic));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
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
    LibraryRootNotDirectory {
        path: PathBuf,
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
            ProjectError::LibraryRootNotDirectory { path } => {
                write!(
                    f,
                    "library search root '{}' is not a directory",
                    path.display()
                )
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
        ModuleResolutionError::ModuleNotFound {
            module,
            searched,
            sources,
        } => {
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
            if !sources.is_empty() {
                writeln!(f, "")?;
                writeln!(f, "    referenced by:")?;
                for source in sources {
                    let location = source.span.start_location;
                    let alias_display = match &source.alias {
                        None => source.import_path.clone(),
                        Some(alias) if alias == &source.import_path => alias.clone(),
                        Some(alias) => format!("{} as {}", source.import_path, alias),
                    };
                    writeln!(
                        f,
                        "      • {}:{}:{} (module '{}', import `{}`)",
                        source.origin_path.display(),
                        location.line,
                        location.column,
                        source.origin_module,
                        alias_display
                    )?;
                }
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
                write!(
                    f,
                    "failed to parse '{}': unknown parser error",
                    path.display()
                )
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

#[cfg(test)]
mod tests {
    use super::*;
    use spectra_compiler::CompilationOptions;
    use tempfile::tempdir;

    #[test]
    fn project_plan_handles_prelude_aliases() {
        let dir = tempdir().expect("failed to create temp dir");
        let entry_path = dir.path().join("app.spectra");

        std::fs::write(
            &entry_path,
            r#"module app;

pub fn demo(a: int, b: int) -> int {
    let total = math.add(a, b);
    io.print(total);
    return total;
}
"#,
        )
        .expect("failed to write entry module");

        let mut plan = ProjectPlan::build(vec![entry_path.clone()], &CompilationOptions::default())
            .expect("project plan should build successfully");

        assert!(plan
            .modules()
            .iter()
            .any(|module| module.name == "app"), "entry module should appear in project plan");

        plan.analyze_semantics()
            .expect("semantic analysis should succeed with prelude aliases");

        dir.close().expect("temp dir close should succeed");
    }
}
