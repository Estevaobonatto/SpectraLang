use crate::ast::{Import, Item, Module, Visibility};
use crate::error::CompilerError;
use crate::parser::workspace::{ModuleLoader, ModuleParseError};
use crate::span::Span;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ModuleResolverOptions {
    /// Additional search roots to probe when locating modules.
    pub roots: Vec<PathBuf>,
    /// Enabled experimental features forwarded to the parser.
    pub experimental_features: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleGraph {
    modules: Vec<ResolvedModule>,
    index_by_name: HashMap<String, usize>,
    entry: String,
}

impl ModuleGraph {
    pub fn modules(&self) -> &[ResolvedModule] {
        &self.modules
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedModule> {
        self.index_by_name
            .get(name)
            .and_then(|index| self.modules.get(*index))
    }

    pub fn entry(&self) -> &ResolvedModule {
        let index = self
            .index_by_name
            .get(&self.entry)
            .copied()
            .expect("entry module missing from graph");
        &self.modules[index]
    }

    pub fn entry_name(&self) -> &str {
        &self.entry
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedModule {
    pub name: String,
    pub path: PathBuf,
    pub ast: Module,
    pub imports: Vec<ResolvedImport>,
}

#[derive(Debug, Clone)]
pub struct ResolvedImport {
    pub module: String,
    pub alias: String,
    pub visibility: Visibility,
    pub span: Span,
    pub is_builtin: bool,
    pub target: Option<usize>,
}

pub struct ModuleResolver {
    loader: ModuleLoader,
    options: ModuleResolverOptions,
}

impl ModuleResolver {
    pub fn new(options: ModuleResolverOptions) -> Self {
        Self {
            loader: ModuleLoader::new(),
            options,
        }
    }

    pub fn resolve(&mut self, entry: &Path) -> Result<ModuleGraph, ModuleResolutionError> {
        let entry_path = canonicalize(entry).map_err(|error| ModuleResolutionError::Io {
            path: entry.to_path_buf(),
            error,
        })?;

        let mut search_roots = Vec::new();
        if let Some(parent) = entry_path.parent() {
            search_roots.push(parent.to_path_buf());
        }

        for root in &self.options.roots {
            let canonical = canonicalize(root).unwrap_or_else(|_| root.clone());
            if !search_roots.contains(&canonical) {
                search_roots.push(canonical);
            }
        }

        let mut module_paths: HashMap<String, PathBuf> = HashMap::new();
        let mut modules: HashMap<String, ModuleNode> = HashMap::new();
        let entry_module = self.parse_from_path(&entry_path)?;
        let entry_name = entry_module.name.clone();

        module_paths.insert(entry_name.clone(), entry_path.clone());
        let entry_node = ModuleNode::new(entry_module, entry_path.clone());
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut scheduled: HashSet<String> = HashSet::new();
        for dependency in &entry_node.dependencies {
            if scheduled.insert(dependency.clone()) {
                queue.push_back(dependency.clone());
            }
        }
        modules.insert(entry_name.clone(), entry_node);

        while let Some(module_name) = queue.pop_front() {
            if modules.contains_key(&module_name) {
                continue;
            }

            let path = locate_module(&module_name, &search_roots)?;
            if let Some(existing) = module_paths.get(&module_name) {
                if !paths_equal(existing, &path) {
                    return Err(ModuleResolutionError::DuplicateModule {
                        module: module_name,
                        existing: existing.clone(),
                        duplicate: path,
                    });
                }
                continue;
            }

            let parsed = self.parse_from_path(&path)?;
            if parsed.name != module_name {
                return Err(ModuleResolutionError::ModuleHeaderMismatch {
                    expected: module_name,
                    found: parsed.name,
                    path,
                });
            }

            module_paths.insert(parsed.name.clone(), path.clone());
            let node = ModuleNode::new(parsed, path);
            for dependency in &node.dependencies {
                if modules.contains_key(dependency) {
                    continue;
                }
                if scheduled.insert(dependency.clone()) {
                    queue.push_back(dependency.clone());
                }
            }
            modules.insert(module_name, node);
        }

        let order = topological_order(&modules)?;
        let mut resolved = Vec::with_capacity(order.len());
        let mut index_by_name = HashMap::with_capacity(order.len());
        for (idx, name) in order.iter().enumerate() {
            let node = modules
                .remove(name)
                .expect("module missing during graph construction");
            let mut imports = node.imports;
            for import in &mut imports {
                if import.is_builtin {
                    continue;
                }
                if let Some(target_idx) = index_by_name.get(&import.module) {
                    import.target = Some(*target_idx);
                }
            }
            index_by_name.insert(name.clone(), idx);
            resolved.push(ResolvedModule {
                name: name.clone(),
                path: node.path,
                ast: node.ast,
                imports,
            });
        }

        Ok(ModuleGraph {
            modules: resolved,
            index_by_name,
            entry: entry_name,
        })
    }

    fn parse_from_path(&mut self, path: &Path) -> Result<Module, ModuleResolutionError> {
        let source = fs::read_to_string(path).map_err(|error| ModuleResolutionError::Io {
            path: path.to_path_buf(),
            error,
        })?;

        let key = path.to_string_lossy().to_string();
        let result = self
            .loader
            .parse_module(&key, &source, &self.options.experimental_features)
            .map_err(|error| map_parse_error(path, error))?;

        Ok(result.module)
    }
}

#[derive(Debug)]
struct ModuleNode {
    path: PathBuf,
    ast: Module,
    imports: Vec<ResolvedImport>,
    dependencies: Vec<String>,
}

impl ModuleNode {
    fn new(module: Module, path: PathBuf) -> Self {
        let imports = collect_imports(&module);
        let dependencies = imports
            .iter()
            .filter(|import| !import.is_builtin)
            .map(|import| import.module.clone())
            .collect();

        Self {
            path,
            ast: module,
            imports,
            dependencies,
        }
    }
}

fn collect_imports(module: &Module) -> Vec<ResolvedImport> {
    let mut imports = Vec::new();

    for item in &module.items {
        if let Item::Import(Import {
            path,
            alias,
            visibility,
            span,
            ..
        }) = item
        {
            if path.is_empty() {
                continue;
            }

            let module_name = path.join(".");
            let alias_name = alias
                .clone()
                .or_else(|| path.last().cloned())
                .unwrap_or_else(|| module_name.clone());
            imports.push(ResolvedImport {
                module: module_name,
                alias: alias_name,
                visibility: *visibility,
                span: *span,
                is_builtin: is_builtin_module(path),
                target: None,
            });
        }
    }

    imports
}

fn is_builtin_module(path: &[String]) -> bool {
    if path.is_empty() {
        return false;
    }

    let name = path.join(".");
    name == "std"
        || name.starts_with("std.")
        || name == "spectra.std"
        || name.starts_with("spectra.std.")
}

fn locate_module(module: &str, roots: &[PathBuf]) -> Result<PathBuf, ModuleResolutionError> {
    let mut relative = PathBuf::new();
    for segment in module.split('.') {
        relative.push(segment);
    }
    let mut candidates = Vec::new();

    for root in roots {
        let mut path = root.join(&relative);
        path.set_extension("spectra");
        candidates.push(path.clone());
        if path.exists() {
            return canonicalize(&path).map_err(|error| ModuleResolutionError::Io { path, error });
        }

        path.set_extension("spc");
        candidates.push(path.clone());
        if path.exists() {
            return canonicalize(&path).map_err(|error| ModuleResolutionError::Io { path, error });
        }
    }

    Err(ModuleResolutionError::ModuleNotFound {
        module: module.to_string(),
        searched: candidates,
    })
}

fn canonicalize(path: &Path) -> Result<PathBuf, io::Error> {
    fs::canonicalize(path)
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    if let (Ok(left), Ok(right)) = (canonicalize(left), canonicalize(right)) {
        left == right
    } else {
        left == right
    }
}

fn map_parse_error(path: &Path, error: ModuleParseError) -> ModuleResolutionError {
    let errors = match error {
        ModuleParseError::Lexical(errors) => errors
            .into_iter()
            .map(CompilerError::Lexical)
            .collect::<Vec<_>>(),
        ModuleParseError::Parse(errors) => errors
            .into_iter()
            .map(CompilerError::Parse)
            .collect::<Vec<_>>(),
    };

    ModuleResolutionError::ParseFailure {
        path: path.to_path_buf(),
        errors,
    }
}

#[derive(Copy, Clone, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Visited,
}

fn topological_order(
    modules: &HashMap<String, ModuleNode>,
) -> Result<Vec<String>, ModuleResolutionError> {
    let mut state: HashMap<String, VisitState> = modules
        .keys()
        .map(|name| (name.clone(), VisitState::Unvisited))
        .collect();
    let mut order = Vec::with_capacity(modules.len());
    let mut stack: Vec<String> = Vec::new();

    for name in modules.keys() {
        if matches!(state.get(name.as_str()), Some(VisitState::Unvisited)) {
            dfs(name, modules, &mut state, &mut stack, &mut order)?;
        }
    }

    Ok(order)
}

fn dfs(
    name: &str,
    modules: &HashMap<String, ModuleNode>,
    state: &mut HashMap<String, VisitState>,
    stack: &mut Vec<String>,
    order: &mut Vec<String>,
) -> Result<(), ModuleResolutionError> {
    match state.get(name) {
        Some(VisitState::Visited) => return Ok(()),
        Some(VisitState::Visiting) => {
            stack.push(name.to_string());
            let cycle = cycle_slice(&stack, name);
            return Err(ModuleResolutionError::Cycle { cycle });
        }
        _ => {}
    }

    if let Some(entry) = state.get_mut(name) {
        *entry = VisitState::Visiting;
    }
    stack.push(name.to_string());

    let node = modules
        .get(name)
        .expect("module missing during DFS traversal");

    for dependency in &node.dependencies {
        let dep_name = dependency.as_str();
        if !modules.contains_key(dep_name) {
            continue;
        }
        dfs(dep_name, modules, state, stack, order)?;
    }

    if let Some(entry) = state.get_mut(name) {
        *entry = VisitState::Visited;
    }
    stack.pop();
    order.push(name.to_string());
    Ok(())
}

fn cycle_slice(stack: &[String], start: &str) -> Vec<String> {
    if let Some(position) = stack.iter().position(|name| name == start) {
        stack[position..].to_vec()
    } else {
        stack.to_vec()
    }
}

#[derive(Debug)]
pub enum ModuleResolutionError {
    Io {
        path: PathBuf,
        error: io::Error,
    },
    ModuleNotFound {
        module: String,
        searched: Vec<PathBuf>,
    },
    ModuleHeaderMismatch {
        expected: String,
        found: String,
        path: PathBuf,
    },
    DuplicateModule {
        module: String,
        existing: PathBuf,
        duplicate: PathBuf,
    },
    ParseFailure {
        path: PathBuf,
        errors: Vec<CompilerError>,
    },
    Cycle {
        cycle: Vec<String>,
    },
}
