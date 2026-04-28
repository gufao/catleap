use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::Mutex;

/// Tracks running game processes by their game ID.
///
/// Two flavours of tracking:
/// - `processes`: children we spawned directly (manual + macOS Steam games);
///   liveness via `Child::try_wait`.
/// - `external`: processes spawned by another app (Steam-Windows launching
///   games via `Steam.exe -applaunch`); liveness via `pgrep -f <install_dir>`.
pub struct ProcessMonitor {
    processes: Mutex<HashMap<String, Child>>,
    external: Mutex<HashMap<String, PathBuf>>,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            external: Mutex::new(HashMap::new()),
        }
    }

    /// Register a child process for a game we spawned ourselves.
    pub fn track(&self, game_id: String, child: Child) {
        let mut map = self.processes.lock().unwrap();
        map.insert(game_id, child);
    }

    /// Register an external process for a game (e.g. Steam-Windows launches).
    /// Liveness will be checked by pgrep -f <install_dir>.
    pub fn track_external(&self, game_id: String, install_dir: PathBuf) {
        let mut map = self.external.lock().unwrap();
        map.insert(game_id, install_dir);
    }

    pub fn untrack_external(&self, game_id: &str) {
        let mut map = self.external.lock().unwrap();
        map.remove(game_id);
    }

    pub fn has_external(&self, game_id: &str) -> bool {
        self.external.lock().unwrap().contains_key(game_id)
    }

    pub fn external_install_dir(&self, game_id: &str) -> Option<PathBuf> {
        self.external.lock().unwrap().get(game_id).cloned()
    }

    /// Stop a tracked game. Tries Child first, then external (pkill -f).
    pub fn stop(&self, game_id: &str) -> Result<(), String> {
        {
            let mut map = self.processes.lock().unwrap();
            if let Some(child) = map.get_mut(game_id) {
                child
                    .kill()
                    .map_err(|e| format!("Failed to kill process for {}: {}", game_id, e))?;
                let _ = child.wait();
                map.remove(game_id);
                return Ok(());
            }
        }
        let install_dir = {
            let map = self.external.lock().unwrap();
            map.get(game_id).cloned()
        };
        match install_dir {
            Some(dir) => {
                let _ = Command::new("/usr/bin/pkill")
                    .arg("-f")
                    .arg(dir.to_string_lossy().as_ref())
                    .status();
                self.untrack_external(game_id);
                Ok(())
            }
            None => Err(format!("No running process found for game: {}", game_id)),
        }
    }

    /// Return true if the game is currently running.
    pub fn is_running(&self, game_id: &str) -> bool {
        {
            let mut map = self.processes.lock().unwrap();
            if let Some(child) = map.get_mut(game_id) {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        map.remove(game_id);
                        return false;
                    }
                    Ok(None) => return true,
                    Err(_) => {
                        map.remove(game_id);
                        return false;
                    }
                }
            }
        }
        let install_dir = self.external.lock().unwrap().get(game_id).cloned();
        match install_dir {
            Some(dir) => Self::pgrep_alive(&dir),
            None => false,
        }
    }

    /// Return the IDs of all games currently running. Cleans up finished
    /// Child-tracked processes; external entries are kept until explicitly
    /// untracked (we don't auto-prune them since pgrep results are transient).
    pub fn running_game_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();

        // Child-tracked
        {
            let mut map = self.processes.lock().unwrap();
            let finished: Vec<String> = map
                .iter_mut()
                .filter_map(|(id, child)| match child.try_wait() {
                    Ok(Some(_)) | Err(_) => Some(id.clone()),
                    Ok(None) => None,
                })
                .collect();
            for id in &finished {
                map.remove(id);
            }
            ids.extend(map.keys().cloned());
        }

        // External: pgrep each
        let externals: Vec<(String, PathBuf)> = self
            .external
            .lock()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (id, dir) in externals {
            if Self::pgrep_alive(&dir) {
                ids.push(id);
            }
        }

        ids
    }

    fn pgrep_alive(install_dir: &Path) -> bool {
        Command::new("/usr/bin/pgrep")
            .arg("-f")
            .arg(install_dir.to_string_lossy().as_ref())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Spawn a process that stays alive for a short while (sleep 10) so we can test.
    fn spawn_sleep() -> Child {
        Command::new("sleep")
            .arg("10")
            .spawn()
            .expect("failed to spawn sleep")
    }

    #[test]
    fn test_monitor_track_and_check() {
        let monitor = ProcessMonitor::new();
        let child = spawn_sleep();
        monitor.track("game_1".to_string(), child);

        assert!(monitor.is_running("game_1"));
        assert!(!monitor.is_running("game_999")); // unknown id
    }

    #[test]
    fn test_monitor_running_game_ids() {
        let monitor = ProcessMonitor::new();
        let child_a = spawn_sleep();
        let child_b = spawn_sleep();
        monitor.track("game_a".to_string(), child_a);
        monitor.track("game_b".to_string(), child_b);

        let mut ids = monitor.running_game_ids();
        ids.sort();
        assert_eq!(ids, vec!["game_a", "game_b"]);

        // Cleanup: stop both processes
        let _ = monitor.stop("game_a");
        let _ = monitor.stop("game_b");
    }

    #[test]
    fn test_monitor_stop_nonexistent() {
        let monitor = ProcessMonitor::new();
        let result = monitor.stop("nonexistent_game");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No running process found"));
    }

    #[test]
    fn track_external_records_install_dir() {
        let mon = ProcessMonitor::new();
        mon.track_external("steam_wine_42".into(), "/tmp/games/Foo".into());
        assert!(mon.has_external("steam_wine_42"));
    }

    #[test]
    fn untrack_external_removes_record() {
        let mon = ProcessMonitor::new();
        mon.track_external("steam_wine_42".into(), "/tmp/games/Foo".into());
        mon.untrack_external("steam_wine_42");
        assert!(!mon.has_external("steam_wine_42"));
    }

    #[test]
    fn external_install_dir_lookup() {
        let mon = ProcessMonitor::new();
        mon.track_external("steam_wine_99".into(), "/tmp/games/Bar".into());
        assert_eq!(
            mon.external_install_dir("steam_wine_99").as_deref(),
            Some(std::path::Path::new("/tmp/games/Bar"))
        );
    }
}
