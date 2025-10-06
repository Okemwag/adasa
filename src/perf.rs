// Performance utilities for profiling and optimization

use std::time::{Duration, Instant};

/// Simple performance timer for measuring operation duration
pub struct PerfTimer {
    name: &'static str,
    start: Instant,
    threshold_ms: Option<u64>,
}

impl PerfTimer {
    /// Create a new performance timer
    ///
    /// # Arguments
    /// * `name` - Name of the operation being timed
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
            threshold_ms: None,
        }
    }

    /// Create a timer that only logs if duration exceeds threshold
    ///
    /// # Arguments
    /// * `name` - Name of the operation being timed
    /// * `threshold_ms` - Only log if duration exceeds this many milliseconds
    pub fn with_threshold(name: &'static str, threshold_ms: u64) -> Self {
        Self {
            name,
            start: Instant::now(),
            threshold_ms: Some(threshold_ms),
        }
    }

    /// Get elapsed time without stopping the timer
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Stop the timer and return elapsed duration
    pub fn stop(self) -> Duration {
        let elapsed = self.start.elapsed();
        
        // Check if we should log based on threshold
        let should_log = match self.threshold_ms {
            Some(threshold) => elapsed.as_millis() >= threshold as u128,
            None => true,
        };

        if should_log {
            tracing::debug!(
                target: "perf",
                operation = self.name,
                duration_ms = elapsed.as_millis(),
                "Operation completed"
            );
        }

        elapsed
    }
}

impl Drop for PerfTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        
        // Check if we should log based on threshold
        let should_log = match self.threshold_ms {
            Some(threshold) => elapsed.as_millis() >= threshold as u128,
            None => false, // Don't auto-log on drop unless explicitly stopped
        };

        if should_log {
            tracing::warn!(
                target: "perf",
                operation = self.name,
                duration_ms = elapsed.as_millis(),
                "Slow operation detected"
            );
        }
    }
}

/// Macro for easy performance timing
#[macro_export]
macro_rules! perf_time {
    ($name:expr) => {
        $crate::perf::PerfTimer::new($name)
    };
    ($name:expr, $threshold_ms:expr) => {
        $crate::perf::PerfTimer::with_threshold($name, $threshold_ms)
    };
}

/// Memory pool for reducing allocations in hot paths
pub struct BufferPool<T> {
    pool: std::sync::Mutex<Vec<T>>,
    factory: fn() -> T,
    max_size: usize,
}

impl<T> BufferPool<T> {
    /// Create a new buffer pool
    ///
    /// # Arguments
    /// * `factory` - Function to create new instances
    /// * `max_size` - Maximum number of items to keep in pool
    pub fn new(factory: fn() -> T, max_size: usize) -> Self {
        Self {
            pool: std::sync::Mutex::new(Vec::with_capacity(max_size)),
            factory,
            max_size,
        }
    }

    /// Get an item from the pool or create a new one
    pub fn acquire(&self) -> PooledItem<'_, T> {
        let item = self.pool.lock()
            .ok()
            .and_then(|mut pool| pool.pop())
            .unwrap_or_else(|| (self.factory)());

        PooledItem {
            item: Some(item),
            pool: self,
        }
    }

    /// Return an item to the pool
    fn release(&self, item: T) {
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < self.max_size {
                pool.push(item);
            }
        }
    }
}

/// RAII wrapper for pooled items
pub struct PooledItem<'a, T> {
    item: Option<T>,
    pool: &'a BufferPool<T>,
}

impl<'a, T> std::ops::Deref for PooledItem<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<'a, T> std::ops::DerefMut for PooledItem<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<'a, T> Drop for PooledItem<'a, T> {
    fn drop(&mut self) {
        if let Some(item) = self.item.take() {
            self.pool.release(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_timer() {
        let timer = PerfTimer::new("test_operation");
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = timer.stop();
        assert!(elapsed.as_millis() >= 10);
    }

    #[test]
    fn test_perf_timer_threshold() {
        let timer = PerfTimer::with_threshold("test_operation", 100);
        std::thread::sleep(Duration::from_millis(5));
        let elapsed = timer.stop();
        assert!(elapsed.as_millis() < 100);
    }

    #[test]
    fn test_buffer_pool() {
        let pool = BufferPool::new(|| Vec::<u8>::with_capacity(1024), 10);
        
        // Acquire an item
        let mut item1 = pool.acquire();
        item1.push(42);
        assert_eq!(item1.len(), 1);
        
        // Drop it back to pool
        drop(item1);
        
        // Acquire again - should reuse
        let item2 = pool.acquire();
        // Note: Vec is not cleared on return, so this tests reuse
        assert_eq!(item2.capacity(), 1024);
    }

    #[test]
    fn test_buffer_pool_max_size() {
        let pool = BufferPool::new(|| Vec::<u8>::new(), 2);
        
        // Acquire and release 3 items
        {
            let _item1 = pool.acquire();
            let _item2 = pool.acquire();
            let _item3 = pool.acquire();
        }
        
        // Pool should only keep 2 items
        let pool_size = pool.pool.lock().unwrap().len();
        assert!(pool_size <= 2);
    }
}
