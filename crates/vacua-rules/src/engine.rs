use glob::Pattern;
use std::path::Path;
use vacua_core::candidate::CandidateCategory;
use vacua_core::risk::RiskLevel;

use crate::guard::{expand_tilde, GuardEvaluator};
use crate::rule::{GuardType, Rule, RuleGuard, RuleMatch};

pub struct RulesEngine {
    rules: Vec<Rule>,
    guard_evaluator: GuardEvaluator,
}

impl Default for RulesEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RulesEngine {
    pub fn new() -> Self {
        let mut engine = Self {
            rules: Vec::new(),
            guard_evaluator: GuardEvaluator::new(),
        };
        engine.load_builtin_rules();
        engine
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn load_from_dir(&mut self, dir: &Path) -> std::io::Result<usize> {
        if !dir.is_dir() {
            return Ok(0);
        }

        let mut count = 0;
        for entry in walkdir::WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("toml") {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Ok(rule) = toml::from_str::<Rule>(&content) {
                        self.rules.push(rule);
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    /// Evaluates if a given path matches any registered rule and passes all guards.
    pub fn match_path(
        &mut self,
        target_path: &Path,
        target_mtime_sec: i64,
        is_dir: bool,
    ) -> Option<&Rule> {
        let path_str = target_path.to_string_lossy();

        for rule in &self.rules {
            for m in &rule.matches {
                let expanded_pattern = expand_tilde(&m.path);
                if let Ok(pattern) = Pattern::new(&expanded_pattern) {
                    if pattern.matches(&path_str) {
                        // Check file type constraint if specified
                        if let Some(ref ft) = m.file_type {
                            if ft == "directory" && !is_dir {
                                continue;
                            }
                            if ft == "file" && is_dir {
                                continue;
                            }
                        }

                        // Evaluate all guards
                        if self.guard_evaluator.evaluate_guards(
                            &rule.guards,
                            target_path,
                            target_mtime_sec,
                        ) {
                            return Some(rule);
                        }
                    }
                }
            }
        }

        None
    }

    fn load_builtin_rules(&mut self) {
        // 1. Xcode DerivedData
        self.rules.push(Rule {
            schema_version: 1,
            id: "apple.xcode.derived-data".into(),
            name: "Xcode DerivedData".into(),
            description: "Xcode intermediate build products, module caches, and index data.".into(),
            category: CandidateCategory::BuildArtifact,
            risk: RiskLevel::Safe,
            reconstructable: true,
            rebuild_consequence: Some(
                "Xcode will regenerate module caches and index on next build.".into(),
            ),
            matches: vec![RuleMatch {
                path: "~/Library/Developer/Xcode/DerivedData/*".into(),
                file_type: Some("directory".into()),
            }],
            guards: vec![
                RuleGuard {
                    guard_type: GuardType::ProcessNotRunning,
                    value: "xcodebuild".into(),
                },
                RuleGuard {
                    guard_type: GuardType::ProcessNotRunning,
                    value: "Xcode".into(),
                },
            ],
        });

        // 2. Cargo target directory
        self.rules.push(Rule {
            schema_version: 1,
            id: "rust.cargo.target".into(),
            name: "Cargo Target Directory".into(),
            description: "Rust compiler build artifacts and incremental cache.".into(),
            category: CandidateCategory::BuildArtifact,
            risk: RiskLevel::Review,
            reconstructable: true,
            rebuild_consequence: Some(
                "Cargo will rebuild target binary on next cargo build.".into(),
            ),
            matches: vec![
                RuleMatch {
                    path: "*/target".into(),
                    file_type: Some("directory".into()),
                },
                RuleMatch {
                    path: "*/target/*".into(),
                    file_type: None,
                },
            ],
            guards: vec![
                RuleGuard {
                    guard_type: GuardType::ProcessNotRunning,
                    value: "cargo".into(),
                },
                RuleGuard {
                    guard_type: GuardType::ProcessNotRunning,
                    value: "rustc".into(),
                },
            ],
        });

        // 3. Homebrew Download Cache
        self.rules.push(Rule {
            schema_version: 1,
            id: "homebrew.download-cache".into(),
            name: "Homebrew Download Cache".into(),
            description: "Downloaded bottles and tarballs cached by Homebrew.".into(),
            category: CandidateCategory::PackageManagerCache,
            risk: RiskLevel::Safe,
            reconstructable: true,
            rebuild_consequence: Some(
                "Homebrew can re-download packages on demand if ever needed.".into(),
            ),
            matches: vec![RuleMatch {
                path: "~/Library/Caches/Homebrew/downloads/*".into(),
                file_type: None,
            }],
            guards: vec![RuleGuard {
                guard_type: GuardType::ProcessNotRunning,
                value: "brew".into(),
            }],
        });
    }
}
