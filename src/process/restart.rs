use std::time::{Duration, SystemTime};

/// Restart policy configuration
#[derive(Debug, Clone)]
pub struct RestartPolicy {
    /// Whether automatic restart is enabled
    pub enabled: bool,
    /// Maximum number of restarts within the time window
    pub max_restarts: usize,
    /// Time window for counting restarts (in seconds)
    pub time_window_secs: u64,
    /// Initial delay before first restart (in seconds)
    pub initial_delay_secs: u64,
    /// Backoff strategy to use
    pub backoff_strategy: BackoffStrategy,
}

impl RestartPolicy {
    /// Create a new restart policy with default values
    pub fn new() -> Self {
        Self {
            enabled: true,
            max_restarts: 10,
            time_window_secs: 60,
            initial_delay_secs: 1,
            backoff_strategy: BackoffStrategy::Exponential { max_delay_secs: 60 },
        }
    }

    /// Create a restart policy from configuration values
    pub fn from_config(enabled: bool, max_restarts: usize, restart_delay_secs: u64) -> Self {
        Self {
            enabled,
            max_restarts,
            time_window_secs: 60,
            initial_delay_secs: restart_delay_secs,
            backoff_strategy: BackoffStrategy::Exponential { max_delay_secs: 60 },
        }
    }

    /// Check if restart should be attempted based on restart history
    pub fn should_restart(&self, tracker: &RestartTracker) -> bool {
        if !self.enabled {
            return false;
        }

        // Check if we've exceeded max restarts in the time window
        let recent_restarts = tracker.count_recent_restarts(self.time_window_secs);
        recent_restarts < self.max_restarts
    }

    /// Calculate the delay before the next restart attempt
    pub fn calculate_delay(&self, tracker: &RestartTracker) -> Duration {
        let restart_count = tracker.restart_count();
        self.backoff_strategy
            .calculate_delay(self.initial_delay_secs, restart_count)
    }
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self::new()
    }
}

/// Backoff strategy for restart delays
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackoffStrategy {
    /// Fixed delay between restarts
    Fixed,
    /// Exponential backoff with maximum delay
    Exponential { max_delay_secs: u64 },
}

impl BackoffStrategy {
    /// Calculate the delay for a given restart attempt
    pub fn calculate_delay(&self, initial_delay_secs: u64, restart_count: usize) -> Duration {
        match self {
            BackoffStrategy::Fixed => Duration::from_secs(initial_delay_secs),
            BackoffStrategy::Exponential { max_delay_secs } => {
                // Exponential backoff: delay = initial * 2^restart_count
                let delay_secs = initial_delay_secs
                    .saturating_mul(2_u64.saturating_pow(restart_count as u32))
                    .min(*max_delay_secs);
                Duration::from_secs(delay_secs)
            }
        }
    }
}

/// Tracks restart history for a process
#[derive(Debug, Clone)]
pub struct RestartTracker {
    /// Timestamps of all restart attempts
    restart_times: Vec<SystemTime>,
}

impl RestartTracker {
    /// Create a new restart tracker
    pub fn new() -> Self {
        Self {
            restart_times: Vec::new(),
        }
    }

    /// Record a restart attempt
    pub fn record_restart(&mut self) {
        self.restart_times.push(SystemTime::now());
    }

    /// Get the total number of restarts
    pub fn restart_count(&self) -> usize {
        self.restart_times.len()
    }

    /// Count restarts within the specified time window (in seconds)
    pub fn count_recent_restarts(&self, window_secs: u64) -> usize {
        let now = SystemTime::now();
        let window = Duration::from_secs(window_secs);

        self.restart_times
            .iter()
            .filter(|&&time| {
                now.duration_since(time)
                    .map(|d| d < window)
                    .unwrap_or(false)
            })
            .count()
    }

    /// Get the time of the last restart, if any
    pub fn last_restart_time(&self) -> Option<SystemTime> {
        self.restart_times.last().copied()
    }

    /// Clear restart history (useful when resetting after a successful run)
    pub fn clear(&mut self) {
        self.restart_times.clear();
    }

    /// Remove restart records older than the specified window
    pub fn prune_old_restarts(&mut self, window_secs: u64) {
        let now = SystemTime::now();
        let window = Duration::from_secs(window_secs);

        self.restart_times.retain(|&time| {
            now.duration_since(time)
                .map(|d| d < window)
                .unwrap_or(false)
        });
    }
}

impl Default for RestartTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_restart_policy_new() {
        let policy = RestartPolicy::new();
        assert!(policy.enabled);
        assert_eq!(policy.max_restarts, 10);
        assert_eq!(policy.time_window_secs, 60);
        assert_eq!(policy.initial_delay_secs, 1);
    }

    #[test]
    fn test_restart_policy_from_config() {
        let policy = RestartPolicy::from_config(true, 5, 2);
        assert!(policy.enabled);
        assert_eq!(policy.max_restarts, 5);
        assert_eq!(policy.initial_delay_secs, 2);
    }

    #[test]
    fn test_restart_policy_disabled() {
        let policy = RestartPolicy::from_config(false, 10, 1);
        let tracker = RestartTracker::new();
        assert!(!policy.should_restart(&tracker));
    }

    #[test]
    fn test_restart_policy_should_restart() {
        let policy = RestartPolicy::from_config(true, 3, 1);
        let mut tracker = RestartTracker::new();

        // Should allow restarts under the limit
        assert!(policy.should_restart(&tracker));

        tracker.record_restart();
        assert!(policy.should_restart(&tracker));

        tracker.record_restart();
        assert!(policy.should_restart(&tracker));

        tracker.record_restart();
        // Should not allow restart after hitting the limit
        assert!(!policy.should_restart(&tracker));
    }

    #[test]
    fn test_backoff_fixed() {
        let strategy = BackoffStrategy::Fixed;
        assert_eq!(strategy.calculate_delay(5, 0), Duration::from_secs(5));
        assert_eq!(strategy.calculate_delay(5, 1), Duration::from_secs(5));
        assert_eq!(strategy.calculate_delay(5, 10), Duration::from_secs(5));
    }

    #[test]
    fn test_backoff_exponential() {
        let strategy = BackoffStrategy::Exponential { max_delay_secs: 60 };

        // 1 * 2^0 = 1
        assert_eq!(strategy.calculate_delay(1, 0), Duration::from_secs(1));
        // 1 * 2^1 = 2
        assert_eq!(strategy.calculate_delay(1, 1), Duration::from_secs(2));
        // 1 * 2^2 = 4
        assert_eq!(strategy.calculate_delay(1, 2), Duration::from_secs(4));
        // 1 * 2^3 = 8
        assert_eq!(strategy.calculate_delay(1, 3), Duration::from_secs(8));
        // 1 * 2^6 = 64, but capped at 60
        assert_eq!(strategy.calculate_delay(1, 6), Duration::from_secs(60));
        // 1 * 2^10 = 1024, but capped at 60
        assert_eq!(strategy.calculate_delay(1, 10), Duration::from_secs(60));
    }

    #[test]
    fn test_restart_tracker_new() {
        let tracker = RestartTracker::new();
        assert_eq!(tracker.restart_count(), 0);
        assert!(tracker.last_restart_time().is_none());
    }

    #[test]
    fn test_restart_tracker_record() {
        let mut tracker = RestartTracker::new();

        tracker.record_restart();
        assert_eq!(tracker.restart_count(), 1);
        assert!(tracker.last_restart_time().is_some());

        tracker.record_restart();
        assert_eq!(tracker.restart_count(), 2);
    }

    #[test]
    fn test_restart_tracker_count_recent() {
        let mut tracker = RestartTracker::new();

        tracker.record_restart();
        thread::sleep(Duration::from_millis(100));
        tracker.record_restart();
        thread::sleep(Duration::from_millis(100));
        tracker.record_restart();

        // All restarts should be within 1 second
        assert_eq!(tracker.count_recent_restarts(1), 3);

        // All restarts should be within 10 seconds
        assert_eq!(tracker.count_recent_restarts(10), 3);
    }

    #[test]
    fn test_restart_tracker_clear() {
        let mut tracker = RestartTracker::new();

        tracker.record_restart();
        tracker.record_restart();
        assert_eq!(tracker.restart_count(), 2);

        tracker.clear();
        assert_eq!(tracker.restart_count(), 0);
        assert!(tracker.last_restart_time().is_none());
    }

    #[test]
    fn test_restart_tracker_prune() {
        let mut tracker = RestartTracker::new();

        // Add some restarts
        tracker.record_restart();
        thread::sleep(Duration::from_millis(100));
        tracker.record_restart();

        // Prune with a very short window (should remove old ones)
        tracker.prune_old_restarts(0);
        assert_eq!(tracker.restart_count(), 0);
    }

    #[test]
    fn test_calculate_delay_integration() {
        let policy = RestartPolicy::from_config(true, 10, 1);
        let mut tracker = RestartTracker::new();

        // First restart: 1 second
        assert_eq!(policy.calculate_delay(&tracker), Duration::from_secs(1));

        tracker.record_restart();
        // Second restart: 2 seconds
        assert_eq!(policy.calculate_delay(&tracker), Duration::from_secs(2));

        tracker.record_restart();
        // Third restart: 4 seconds
        assert_eq!(policy.calculate_delay(&tracker), Duration::from_secs(4));
    }
}
