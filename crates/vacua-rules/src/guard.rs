use crate::rule::{GuardType, RuleGuard};
use std::path::Path;
use sysinfo::{ProcessesToUpdate, System};

pub struct GuardEvaluator {
    system: System,
    processes_refreshed: bool,
}

impl Default for GuardEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl GuardEvaluator {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            processes_refreshed: false,
        }
    }

    fn ensure_processes_refreshed(&mut self) {
        if !self.processes_refreshed {
            self.system.refresh_processes(ProcessesToUpdate::All, true);
            self.processes_refreshed = true;
        }
    }

    /// Returns true if all guards pass; false if any guard is violated.
    pub fn evaluate_guards(
        &mut self,
        guards: &[RuleGuard],
        _target_path: &Path,
        target_mtime_sec: i64,
    ) -> bool {
        for guard in guards {
            match guard.guard_type {
                GuardType::ProcessNotRunning => {
                    self.ensure_processes_refreshed();
                    let target_proc = guard.value.trim().to_lowercase();
                    let is_running = self.system.processes().values().any(|proc| {
                        let name = proc.name().to_string_lossy().to_lowercase();
                        name == target_proc || name.contains(&target_proc)
                    });

                    if is_running {
                        // Guard failed: process is actively running!
                        return false;
                    }
                }
                GuardType::MinAgeDays => {
                    if let Ok(min_days) = guard.value.parse::<i64>() {
                        let now_sec = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0);

                        let age_days = (now_sec - target_mtime_sec) / 86400;
                        if age_days < min_days {
                            return false;
                        }
                    }
                }
                GuardType::PathExists => {
                    let expanded = expand_tilde(&guard.value);
                    if !Path::new(&expanded).exists() {
                        return false;
                    }
                }
            }
        }

        true
    }
}

pub fn expand_tilde(path_str: &str) -> String {
    if path_str.starts_with("~/") || path_str == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            let home_str = home.to_string_lossy();
            return path_str.replacen('~', &home_str, 1);
        }
    }
    path_str.to_string()
}
