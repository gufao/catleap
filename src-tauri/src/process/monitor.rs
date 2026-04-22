use std::collections::HashMap;
use std::process::Child;
use std::sync::Mutex;

/// Tracks running game processes by their game ID.
pub struct ProcessMonitor {
    processes: Mutex<HashMap<String, Child>>,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
        }
    }

    /// Register a newly spawned child process for a game.
    pub fn track(&self, game_id: String, child: Child) {
        let mut map = self.processes.lock().unwrap();
        map.insert(game_id, child);
    }

    /// Send SIGKILL / TerminateProcess to a tracked game and remove it.
    pub fn stop(&self, game_id: &str) -> Result<(), String> {
        let mut map = self.processes.lock().unwrap();
        match map.get_mut(game_id) {
            Some(child) => {
                child
                    .kill()
                    .map_err(|e| format!("Failed to kill process for {}: {}", game_id, e))?;
                let _ = child.wait(); // reap zombie
                map.remove(game_id);
                Ok(())
            }
            None => Err(format!("No running process found for game: {}", game_id)),
        }
    }

    /// Return true if the game is currently running. Cleans up finished processes.
    pub fn is_running(&self, game_id: &str) -> bool {
        let mut map = self.processes.lock().unwrap();
        match map.get_mut(game_id) {
            Some(child) => match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited — remove it
                    map.remove(game_id);
                    false
                }
                Ok(None) => true, // still running
                Err(_) => {
                    map.remove(game_id);
                    false
                }
            },
            None => false,
        }
    }

    /// Return the IDs of all games that are currently running. Cleans up finished processes.
    pub fn running_game_ids(&self) -> Vec<String> {
        let mut map = self.processes.lock().unwrap();

        // Identify finished processes
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

        map.keys().cloned().collect()
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
}
