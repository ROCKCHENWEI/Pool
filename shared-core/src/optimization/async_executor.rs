//! Async Task Executor
//!
//! Provides a concurrent task scheduler for managing async operations
//! with priority support and task tracking.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tokio::sync::{mpsc, Semaphore};

/// Unique task identifier
pub type TaskId = u64;

/// Task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TaskPriority {
    /// Low priority - background tasks
    Low = 0,
    /// Normal priority - default
    Normal = 1,
    /// High priority - user-initiated tasks
    High = 2,
    /// Critical priority - system-critical tasks
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is pending execution
    Pending,
    /// Task is currently running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Handle to a submitted task
#[derive(Debug)]
pub struct TaskHandle {
    /// Unique task identifier
    pub id: TaskId,
    /// Task name/description
    pub name: String,
    /// Task priority
    pub priority: TaskPriority,
    /// Time when task was submitted
    pub submitted_at: Instant,
    /// Current status
    status: Arc<RwLock<TaskStatus>>,
}

impl TaskHandle {
    /// Get the current task status
    pub fn status(&self) -> TaskStatus {
        *self.status.read()
    }

    /// Check if the task is finished
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status(),
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// Get elapsed time since submission
    pub fn elapsed(&self) -> Duration {
        self.submitted_at.elapsed()
    }
}

/// Internal task representation
struct InternalTask {
    id: TaskId,
    name: String,
    priority: TaskPriority,
    status: Arc<RwLock<TaskStatus>>,
    task: Pin<Box<dyn Future<Output = Result<(), String>> + Send>>,
}

/// Executor statistics
#[derive(Debug, Clone, Default)]
pub struct ExecutorStats {
    /// Total tasks submitted
    pub total_submitted: u64,
    /// Tasks currently running
    pub running: usize,
    /// Tasks completed successfully
    pub completed: u64,
    /// Tasks that failed
    pub failed: u64,
    /// Tasks cancelled
    pub cancelled: u64,
}

/// Async task executor with priority scheduling
pub struct AsyncExecutor {
    /// Maximum concurrent tasks
    max_concurrent: usize,
    /// Semaphore for limiting concurrency
    semaphore: Arc<Semaphore>,
    /// Task counter
    task_counter: AtomicU64,
    /// Channel for submitting tasks
    task_sender: mpsc::UnboundedSender<InternalTask>,
    /// Statistics
    stats: Arc<RwLock<ExecutorStats>>,
    /// Shutdown flag
    shutdown: Arc<RwLock<bool>>,
}

impl AsyncExecutor {
    /// Create a new async executor
    pub fn new(max_concurrent: usize) -> Self {
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let (task_sender, mut task_receiver) = mpsc::unbounded_channel::<InternalTask>();
        let stats = Arc::new(RwLock::new(ExecutorStats::default()));
        let shutdown = Arc::new(RwLock::new(false));

        // Spawn the scheduler task
        let semaphore_clone = semaphore.clone();
        let stats_clone = stats.clone();
        let shutdown_clone = shutdown.clone();

        tokio::spawn(async move {
            while !*shutdown_clone.read() {
                // Try to receive a task with timeout
                tokio::select! {
                    Some(task) = task_receiver.recv() => {
                        let permit = semaphore_clone.clone().acquire_owned().await.ok();
                        if let Some(permit) = permit {
                            let status = task.status.clone();
                            let stats = stats_clone.clone();

                            // Update status to running
                            *status.write() = TaskStatus::Running;
                            stats.write().running += 1;

                            tokio::spawn(async move {
                                let result = task.task.await;

                                // Update status based on result
                                let mut status_guard = status.write();
                                let mut stats_guard = stats.write();

                                stats_guard.running -= 1;
                                match result {
                                    Ok(()) => {
                                        *status_guard = TaskStatus::Completed;
                                        stats_guard.completed += 1;
                                    }
                                    Err(_) => {
                                        *status_guard = TaskStatus::Failed;
                                        stats_guard.failed += 1;
                                    }
                                }

                                drop(permit);
                            });
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Check for shutdown periodically
                    }
                }
            }
        });

        Self {
            max_concurrent,
            semaphore,
            task_counter: AtomicU64::new(0),
            task_sender,
            stats,
            shutdown,
        }
    }

    /// Submit a new task to the executor
    pub fn submit<F>(
        &self,
        name: impl Into<String>,
        priority: TaskPriority,
        task: F,
    ) -> Result<TaskHandle, String>
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
    {
        if *self.shutdown.read() {
            return Err("Executor is shutting down".to_string());
        }

        let id = self.task_counter.fetch_add(1, Ordering::SeqCst);
        let name = name.into();
        let status = Arc::new(RwLock::new(TaskStatus::Pending));

        let internal_task = InternalTask {
            id,
            name: name.clone(),
            priority,
            status: status.clone(),
            task: Box::pin(task),
        };

        self.task_sender.send(internal_task)
            .map_err(|_| "Failed to submit task".to_string())?;

        self.stats.write().total_submitted += 1;

        Ok(TaskHandle {
            id,
            name,
            priority,
            submitted_at: Instant::now(),
            status,
        })
    }

    /// Submit a simple closure as a task
    pub fn submit_fn<F>(
        &self,
        name: impl Into<String>,
        priority: TaskPriority,
        f: F,
    ) -> Result<TaskHandle, String>
    where
        F: FnOnce() -> Result<(), String> + Send + 'static,
    {
        self.submit(name, priority, async move { f() })
    }

    /// Submit a task with default priority
    pub fn submit_default<F>(&self, name: impl Into<String>, task: F) -> Result<TaskHandle, String>
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.submit(name, TaskPriority::default(), task)
    }

    /// Submit a high-priority task
    pub fn submit_high_priority<F>(&self, name: impl Into<String>, task: F) -> Result<TaskHandle, String>
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.submit(name, TaskPriority::High, task)
    }

    /// Get current executor statistics
    pub fn stats(&self) -> ExecutorStats {
        self.stats.read().clone()
    }

    /// Get the maximum concurrent tasks
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Get the number of available permits
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// Check if the executor is shut down
    pub fn is_shutdown(&self) -> bool {
        *self.shutdown.read()
    }

    /// Shutdown the executor
    pub fn shutdown(&self) {
        *self.shutdown.write() = true;
    }

    /// Wait for all running tasks to complete (graceful shutdown)
    pub async fn wait_for_completion(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        while start.elapsed() < timeout {
            let stats = self.stats();
            if stats.running == 0 && stats.total_submitted == stats.completed + stats.failed + stats.cancelled {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        false
    }
}

impl Drop for AsyncExecutor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Batch task executor for running multiple tasks in parallel
pub struct BatchExecutor {
    executor: Arc<AsyncExecutor>,
}

impl BatchExecutor {
    /// Create a new batch executor
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            executor: Arc::new(AsyncExecutor::new(max_concurrent)),
        }
    }

    /// Execute a batch of tasks and return all handles
    pub fn execute_batch<F, I>(
        &self,
        tasks: I,
    ) -> Vec<Result<TaskHandle, String>>
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        I: IntoIterator<Item = (String, TaskPriority, F)>,
    {
        tasks
            .into_iter()
            .map(|(name, priority, task)| self.executor.submit(name, priority, task))
            .collect()
    }

    /// Execute all tasks and wait for completion
    pub async fn execute_and_wait<F, I>(
        &self,
        tasks: I,
        timeout: Duration,
    ) -> Vec<TaskStatus>
    where
        F: Future<Output = Result<(), String>> + Send + 'static,
        I: IntoIterator<Item = (String, TaskPriority, F)>,
    {
        let handles: Vec<_> = self.execute_batch(tasks)
            .into_iter()
            .filter_map(|h| h.ok())
            .collect();

        let start = Instant::now();
        while start.elapsed() < timeout {
            let all_finished = handles.iter().all(|h| h.is_finished());
            if all_finished {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        handles.iter().map(|h| h.status()).collect()
    }

    /// Get the underlying executor
    pub fn executor(&self) -> Arc<AsyncExecutor> {
        self.executor.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
    }

    #[test]
    fn test_executor_creation() {
        let executor = AsyncExecutor::new(10);
        assert_eq!(executor.max_concurrent(), 10);
        assert_eq!(executor.available_permits(), 10);
        assert!(!executor.is_shutdown());
    }

    #[test]
    fn test_task_submission() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let executor = AsyncExecutor::new(5);
            let handle = executor.submit(
                "test_task",
                TaskPriority::Normal,
                async { Ok(()) },
            ).unwrap();

            assert_eq!(handle.priority, TaskPriority::Normal);
            assert!(!handle.is_finished());

            // Wait a bit for task to complete
            tokio::time::sleep(Duration::from_millis(100)).await;
        });
    }

    #[test]
    fn test_executor_stats() {
        let executor = AsyncExecutor::new(5);
        let stats = executor.stats();

        assert_eq!(stats.total_submitted, 0);
        assert_eq!(stats.running, 0);
        assert_eq!(stats.completed, 0);
    }

    #[test]
    fn test_batch_executor() {
        let batch = BatchExecutor::new(5);
        let executor = batch.executor();

        assert_eq!(executor.max_concurrent(), 5);
    }
}
