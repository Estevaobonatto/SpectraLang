use std::collections::{hash_map::DefaultHasher, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use crate::{
    ast::Module,
    error::{LexError, ParseError},
    lexer::Lexer,
};

use super::Parser;

#[derive(Debug)]
struct CachedModule {
    hash: u64,
    feature_key: Vec<String>,
    outcome: CachedOutcome,
}

#[derive(Debug, Clone)]
enum CachedOutcome {
    Success(Module),
    Lexical(Vec<LexError>),
    Parse(Vec<ParseError>),
}

#[derive(Debug, Clone)]
pub struct ModuleParseSuccess {
    pub module: Module,
    pub reused: bool,
    pub lexing_duration: Duration,
    pub parsing_duration: Duration,
}

#[derive(Debug, Clone)]
pub enum ModuleParseError {
    Lexical(Vec<LexError>),
    Parse(Vec<ParseError>),
}

pub struct ModuleLoader {
    cache: HashMap<String, CachedModule>,
}

impl ModuleLoader {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn invalidate(&mut self, module_id: &str) {
        self.cache.remove(module_id);
    }

    pub fn parse_module(
        &mut self,
        module_id: &str,
        source: &str,
        features: &HashSet<String>,
    ) -> Result<ModuleParseSuccess, ModuleParseError> {
        let feature_key = Self::feature_key(features);
        let hash = Self::compute_hash(source, &feature_key);

        if let Some(entry) = self.cache.get(module_id) {
            if entry.hash == hash && entry.feature_key == feature_key {
                return match &entry.outcome {
                    CachedOutcome::Success(module) => Ok(ModuleParseSuccess {
                        module: module.clone(),
                        reused: true,
                        lexing_duration: Duration::default(),
                        parsing_duration: Duration::default(),
                    }),
                    CachedOutcome::Lexical(errors) => {
                        Err(ModuleParseError::Lexical(errors.clone()))
                    }
                    CachedOutcome::Parse(errors) => {
                        Err(ModuleParseError::Parse(errors.clone()))
                    }
                };
            }
        }

        let lex_start = Instant::now();
        let tokens = match Lexer::new(source).tokenize() {
            Ok(tokens) => tokens,
            Err(errors) => {
                let cloned = errors.clone();
                self.cache.insert(
                    module_id.to_string(),
                    CachedModule {
                        hash,
                        feature_key,
                        outcome: CachedOutcome::Lexical(cloned),
                    },
                );
                return Err(ModuleParseError::Lexical(errors));
            }
        };
        let lex_duration = lex_start.elapsed();

        let parse_start = Instant::now();
        let result = Parser::new(tokens, features.clone()).parse();
        let parse_duration = parse_start.elapsed();

        match result {
            Ok(module) => {
                let stored_module = module.clone();
                self.cache.insert(
                    module_id.to_string(),
                    CachedModule {
                        hash,
                        feature_key,
                        outcome: CachedOutcome::Success(stored_module),
                    },
                );

                Ok(ModuleParseSuccess {
                    module,
                    reused: false,
                    lexing_duration: lex_duration,
                    parsing_duration: parse_duration,
                })
            }
            Err(errors) => {
                let cloned = errors.clone();
                self.cache.insert(
                    module_id.to_string(),
                    CachedModule {
                        hash,
                        feature_key,
                        outcome: CachedOutcome::Parse(cloned),
                    },
                );
                Err(ModuleParseError::Parse(errors))
            }
        }
    }

    fn compute_hash(source: &str, features: &[String]) -> u64 {
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        for feature in features {
            feature.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn feature_key(features: &HashSet<String>) -> Vec<String> {
        let mut list: Vec<_> = features.iter().cloned().collect();
        list.sort();
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn caches_successful_parse() {
        let mut loader = ModuleLoader::new();
        let features = HashSet::new();
        let source = "\n            module demo;\n            fn main() {}\n        ";

        let first = loader
            .parse_module("demo", source, &features)
            .expect("first parse should succeed");
        assert!(!first.reused);

        let second = loader
            .parse_module("demo", source, &features)
            .expect("second parse should reuse cache");
        assert!(second.reused);
    }

    #[test]
    fn reparses_when_source_changes() {
        let mut loader = ModuleLoader::new();
        let features = HashSet::new();

        let original = "\n            module demo;\n            fn main() {}\n        ";

        loader
            .parse_module("demo", original, &features)
            .expect("initial parse should succeed");

        let modified = "\n            module demo;\n            fn main() { let x = 1; }\n        ";

        let result = loader
            .parse_module("demo", modified, &features)
            .expect("modified source should reparse successfully");
        assert!(!result.reused, "modified source must trigger reparse");
    }

    #[test]
    fn feature_set_changes_trigger_reparse() {
        let mut loader = ModuleLoader::new();
        let mut features = HashSet::new();
        features.insert("unless".to_string());

        let source = "\n            module demo;\n            fn main() { let value = unless false { 1 }; }\n        ";

        loader
            .parse_module("demo", source, &features)
            .expect("feature-enabled parse should succeed");

        let disabled_features = HashSet::new();
        let result = loader.parse_module("demo", source, &disabled_features);
        assert!(matches!(result, Err(ModuleParseError::Parse(_))));
    }
}
