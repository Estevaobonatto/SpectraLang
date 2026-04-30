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
    pub fn build(entries: Vec<PathBuf>) -> Result<Self, ProjectError> {
        if entries.is_empty() {
            return Ok(Self {
                modules: Vec::new(),
            });
        }

        let mut discovered: HashSet<PathBuf> = HashSet::new();
        let mut entry_set: HashSet<PathBuf> = HashSet::new();

        for entry in entries {
            let normalized =
                normalize_path(&entry).map_err(|error| ProjectError::Io { path: entry, error })?;
            entry_set.insert(normalized.clone());
            collect_sources(&normalized, &mut discovered)?;
        }

        if discovered.is_empty() {
            return Err(ProjectError::NoSourcesFound(
                entry_set.into_iter().collect(),
            ));
        }

        let mut modules = Vec::new();
        let mut module_map: HashMap<String, PathBuf> = HashMap::new();

        for path in discovered.into_iter() {
            let source = fs::read_to_string(&path).map_err(|error| ProjectError::Io {
                path: path.clone(),
                error,
            })?;
            let module = extract_module_name(&source).unwrap_or_else(|| {
                // No explicit `module <name>;` declaration — derive the name
                // from the file stem so that single-file scripts and simple
                // projects work without a boilerplate header.
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "main".to_string())
            });

            if let Some(existing) = module_map.get(&module) {
                return Err(ProjectError::DuplicateModule {
                    module,
                    existing: existing.clone(),
                    duplicate: path.clone(),
                });
            }

            let imports = extract_imports(&source);
            module_map.insert(module.clone(), path.clone());
            modules.push(ResolvedModule {
                name: module,
                path,
                imports,
            });
        }

        let missing = collect_missing_dependencies(&modules, &module_map);
        if !missing.is_empty() {
            return Err(ProjectError::MissingDependencies(missing));
        }

        let order = topological_order(&modules)?;
        let ordered_modules = order
            .into_iter()
            .map(|index| modules[index].clone())
            .collect();

        Ok(Self {
            modules: ordered_modules,
        })
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
    /// Kept for potential programmatic use; the CLI itself now derives a module
    /// name from the file stem when no `module` declaration is present.
    #[allow(dead_code)]
    MissingModuleHeader {
        path: PathBuf,
    },
    DuplicateModule {
        module: String,
        existing: PathBuf,
        duplicate: PathBuf,
    },
    MissingDependencies(Vec<MissingDependency>),
    CyclicDependency(Vec<String>),
    NoSourcesFound(Vec<PathBuf>),
}

#[derive(Debug)]
pub struct MissingDependency {
    pub module: String,
    pub missing: Vec<String>,
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Io { path, error } => {
                write!(f, "failed to read '{}': {}", path.display(), error)
            }
            ProjectError::MissingModuleHeader { path } => {
                write!(
                    f,
                    "file '{}' is missing a module declaration\n\
                     help: add 'module <name>;' as the first non-comment line of the file",
                    path.display()
                )
            }
            ProjectError::DuplicateModule {
                module,
                existing,
                duplicate,
            } => {
                write!(
                    f,
                    "module '{}' is declared by two different files:\n  \
                     first:  {}\n  \
                     second: {}\n\
                     help: each module name must be unique within a project",
                    module,
                    existing.display(),
                    duplicate.display()
                )
            }
            ProjectError::MissingDependencies(items) => {
                writeln!(f, "unresolved imports:")?;
                for item in items {
                    for missing in &item.missing {
                        writeln!(
                            f,
                            "  - module '{}' imports '{}', but no file declaring 'module {};' was found",
                            item.module, missing, missing
                        )?;
                    }
                }
                write!(
                    f,
                    "help: create a source file with 'module <name>;' for each missing module"
                )
            }
            ProjectError::CyclicDependency(cycle) => {
                write!(
                    f,
                    "cyclic dependency detected: {}\n\
                     help: restructure your modules to break the circular import chain",
                    cycle.join(" -> ")
                )
            }
            ProjectError::NoSourcesFound(paths) => {
                writeln!(f, "no Spectra source files found in the given path(s):")?;
                for path in paths {
                    writeln!(f, "  - {}", path.display())?;
                }
                write!(f, "help: source files must have a .spectra or .spc extension")
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

fn extract_module_name(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("module ") {
            let rest = rest.split("//").next().unwrap_or(rest).trim();
            let rest = rest.trim_end_matches(';').trim();
            if rest.is_empty() {
                return None;
            }
            return Some(rest.to_string());
        }

        // Stop scanning once we reach non-comment, non-module tokens.
        break;
    }
    None
}

fn extract_imports(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();

        // Strip `pub` prefix (re-exports: `pub import path`)
        let trimmed = trimmed
            .strip_prefix("pub ")
            .unwrap_or(trimmed);

        if !trimmed.starts_with("import ") {
            continue;
        }

        let rest = &trimmed["import ".len()..];
        let rest = rest.split("//").next().unwrap_or(rest).trim();
        let rest = rest.trim_end_matches(';').trim();
        if rest.is_empty() {
            continue;
        }

        // `import { a, b } from path.to.module`
        let module_name = if rest.starts_with('{') {
            if let Some(from_pos) = rest.find("} from ") {
                let after_from = rest[from_pos + "} from ".len()..].trim();
                after_from.split_whitespace().next().unwrap_or("").trim()
            } else {
                continue;
            }
        } else {
            // `import path.to.module` or `import path.to.module as alias`
            if let Some((module, _alias)) = rest.split_once(" as ") {
                module.trim()
            } else {
                rest
            }
        };

        if !module_name.is_empty() {
            imports.push(module_name.to_string());
        }
    }
    imports
}

fn collect_missing_dependencies(
    modules: &[ResolvedModule],
    module_map: &HashMap<String, PathBuf>,
) -> Vec<MissingDependency> {
    let mut missing = Vec::new();

    for module in modules {
        let unresolved: Vec<String> = module
            .imports
            .iter()
            .filter(|dep| !is_builtin_module(dep) && !module_map.contains_key(*dep))
            .cloned()
            .collect();

        if !unresolved.is_empty() {
            missing.push(MissingDependency {
                module: module.name.clone(),
                missing: unresolved,
            });
        }
    }

    missing
}

fn is_builtin_module(name: &str) -> bool {
    name == "std"
        || name.starts_with("std.")
        || name == "spectra.std"
        || name.starts_with("spectra.std.")
}

fn topological_order(modules: &[ResolvedModule]) -> Result<Vec<usize>, ProjectError> {
    #[derive(Copy, Clone, PartialEq)]
    enum VisitState {
        Unvisited,
        Visiting,
        Visited,
    }

    let mut state = vec![VisitState::Unvisited; modules.len()];
    let mut order = Vec::with_capacity(modules.len());
    let mut stack = Vec::new();
    let name_to_index: HashMap<&str, usize> = modules
        .iter()
        .enumerate()
        .map(|(index, module)| (module.name.as_str(), index))
        .collect();

    fn dfs(
        index: usize,
        modules: &[ResolvedModule],
        state: &mut [VisitState],
        order: &mut Vec<usize>,
        stack: &mut Vec<String>,
        name_to_index: &HashMap<&str, usize>,
    ) -> Result<(), ProjectError> {
        if state[index] == VisitState::Visiting {
            let module = &modules[index];
            stack.push(module.name.clone());
            return Err(ProjectError::CyclicDependency(stack.clone()));
        }
        if state[index] == VisitState::Visited {
            return Ok(());
        }

        state[index] = VisitState::Visiting;
        stack.push(modules[index].name.clone());

        for dep in &modules[index].imports {
            if is_builtin_module(dep) {
                continue;
            }
            if let Some(&dep_index) = name_to_index.get(dep.as_str()) {
                dfs(dep_index, modules, state, order, stack, name_to_index)?;
            }
        }

        stack.pop();
        state[index] = VisitState::Visited;
        order.push(index);
        Ok(())
    }

    for index in 0..modules.len() {
        dfs(
            index,
            modules,
            &mut state,
            &mut order,
            &mut stack,
            &name_to_index,
        )?;
    }

    // Post-order DFS already produces dependencies-first topological order.
    Ok(order)
}
