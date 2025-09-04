//! Task queue implementation with priority management

use crate::planning::{Task, TaskStatus};
use std::collections::{HashMap, VecDeque};

/// Priority levels for tasks in the queue
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QueuePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Task queue with priority and dependency management
#[derive(Debug)]
pub struct TaskQueue {
    /// Tasks organized by priority
    priority_queues: HashMap<QueuePriority, VecDeque<Task>>,
    /// Map of task ID to dependency list
    dependencies: HashMap<String, Vec<String>>,
    /// Set of completed task IDs
    completed_tasks: std::collections::HashSet<String>,
    /// Set of in-progress task IDs
    in_progress_tasks: std::collections::HashSet<String>,
}

impl TaskQueue {
    /// Create a new task queue
    pub fn new() -> Self {
        let mut priority_queues = HashMap::new();
        priority_queues.insert(QueuePriority::Low, VecDeque::new());
        priority_queues.insert(QueuePriority::Normal, VecDeque::new());
        priority_queues.insert(QueuePriority::High, VecDeque::new());
        priority_queues.insert(QueuePriority::Critical, VecDeque::new());

        Self {
            priority_queues,
            dependencies: HashMap::new(),
            completed_tasks: std::collections::HashSet::new(),
            in_progress_tasks: std::collections::HashSet::new(),
        }
    }

    /// Add a task to the queue
    pub fn add_task(&mut self, task: Task, priority: QueuePriority) {
        // Store dependency information
        if !task.dependencies.is_empty() {
            self.dependencies.insert(task.id.clone(), task.dependencies.clone());
        }

        // Add to appropriate priority queue
        if let Some(queue) = self.priority_queues.get_mut(&priority) {
            queue.push_back(task);
        }
    }

    /// Add multiple tasks to the queue
    pub fn add_tasks(&mut self, tasks: Vec<Task>, priority: QueuePriority) {
        for task in tasks {
            self.add_task(task, priority);
        }
    }

    /// Pop the next ready task (highest priority, dependencies satisfied)
    pub fn pop_ready_task(&mut self) -> Option<Task> {
        // Check queues in priority order (highest first)
        let priorities = vec![
            QueuePriority::Critical,
            QueuePriority::High,
            QueuePriority::Normal,
            QueuePriority::Low,
        ];

        for priority in priorities {
            if let Some(queue) = self.priority_queues.get_mut(&priority) {
                // Find the first task whose dependencies are satisfied
                for i in 0..queue.len() {
                    if let Some(task) = queue.get(i) {
                        if self.are_dependencies_satisfied(&task.id) {
                            let task = queue.remove(i).unwrap();
                            self.in_progress_tasks.insert(task.id.clone());
                            return Some(task);
                        }
                    }
                }
            }
        }

        None
    }

    /// Mark a task as completed
    pub fn mark_task_completed(&mut self, task_id: &str) {
        self.in_progress_tasks.remove(task_id);
        self.completed_tasks.insert(task_id.to_string());
    }

    /// Mark a task as failed
    pub fn mark_task_failed(&mut self, task_id: &str) {
        self.in_progress_tasks.remove(task_id);
        // Failed tasks are not added to completed_tasks, so dependent tasks won't run
    }

    /// Check if a task's dependencies are satisfied
    fn are_dependencies_satisfied(&self, task_id: &str) -> bool {
        if let Some(deps) = self.dependencies.get(task_id) {
            deps.iter().all(|dep| self.completed_tasks.contains(dep))
        } else {
            true // No dependencies
        }
    }

    /// Get the number of tasks in the queue
    pub fn len(&self) -> usize {
        self.priority_queues.values().map(|q| q.len()).sum()
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the number of tasks at each priority level
    pub fn priority_counts(&self) -> HashMap<QueuePriority, usize> {
        self.priority_queues
            .iter()
            .map(|(priority, queue)| (*priority, queue.len()))
            .collect()
    }

    /// Get the number of tasks ready to execute
    pub fn ready_task_count(&self) -> usize {
        let mut count = 0;
        
        for queue in self.priority_queues.values() {
            for task in queue {
                if self.are_dependencies_satisfied(&task.id) {
                    count += 1;
                }
            }
        }
        
        count
    }

    /// Get the number of tasks waiting for dependencies
    pub fn waiting_task_count(&self) -> usize {
        self.len() - self.ready_task_count()
    }

    /// Get the number of in-progress tasks
    pub fn in_progress_count(&self) -> usize {
        self.in_progress_tasks.len()
    }

    /// Get the number of completed tasks
    pub fn completed_count(&self) -> usize {
        self.completed_tasks.len()
    }

    /// Clear all tasks from the queue
    pub fn clear(&mut self) {
        for queue in self.priority_queues.values_mut() {
            queue.clear();
        }
        self.dependencies.clear();
        self.completed_tasks.clear();
        self.in_progress_tasks.clear();
    }

    /// Get a summary of the queue state
    pub fn get_summary(&self) -> QueueSummary {
        QueueSummary {
            total_tasks: self.len(),
            ready_tasks: self.ready_task_count(),
            waiting_tasks: self.waiting_task_count(),
            in_progress_tasks: self.in_progress_count(),
            completed_tasks: self.completed_count(),
            priority_counts: self.priority_counts(),
        }
    }

    /// Get all tasks in the queue (for inspection)
    pub fn get_all_tasks(&self) -> Vec<&Task> {
        let mut tasks = Vec::new();
        
        // Add in priority order
        let priorities = vec![
            QueuePriority::Critical,
            QueuePriority::High,
            QueuePriority::Normal,
            QueuePriority::Low,
        ];

        for priority in priorities {
            if let Some(queue) = self.priority_queues.get(&priority) {
                tasks.extend(queue.iter());
            }
        }
        
        tasks
    }

    /// Get tasks that are ready to execute
    pub fn get_ready_tasks(&self) -> Vec<&Task> {
        let mut ready_tasks = Vec::new();
        
        for queue in self.priority_queues.values() {
            for task in queue {
                if self.are_dependencies_satisfied(&task.id) {
                    ready_tasks.push(task);
                }
            }
        }
        
        ready_tasks
    }

    /// Get tasks that are waiting for dependencies
    pub fn get_waiting_tasks(&self) -> Vec<&Task> {
        let mut waiting_tasks = Vec::new();
        
        for queue in self.priority_queues.values() {
            for task in queue {
                if !self.are_dependencies_satisfied(&task.id) {
                    waiting_tasks.push(task);
                }
            }
        }
        
        waiting_tasks
    }

    /// Check if a specific task is in the queue
    pub fn contains_task(&self, task_id: &str) -> bool {
        for queue in self.priority_queues.values() {
            if queue.iter().any(|task| task.id == task_id) {
                return true;
            }
        }
        false
    }

    /// Remove a specific task from the queue
    pub fn remove_task(&mut self, task_id: &str) -> Option<Task> {
        for queue in self.priority_queues.values_mut() {
            if let Some(pos) = queue.iter().position(|task| task.id == task_id) {
                let removed_task = queue.remove(pos);
                self.dependencies.remove(task_id);
                return removed_task;
            }
        }
        None
    }

    /// Update task priority
    pub fn update_task_priority(&mut self, task_id: &str, new_priority: QueuePriority) -> bool {
        // Find and remove the task
        if let Some(task) = self.remove_task(task_id) {
            // Add it back with new priority
            self.add_task(task, new_priority);
            true
        } else {
            false
        }
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of the task queue state
#[derive(Debug, Clone)]
pub struct QueueSummary {
    pub total_tasks: usize,
    pub ready_tasks: usize,
    pub waiting_tasks: usize,
    pub in_progress_tasks: usize,
    pub completed_tasks: usize,
    pub priority_counts: HashMap<QueuePriority, usize>,
}

impl QueueSummary {
    /// Check if there are any tasks to process
    pub fn has_work(&self) -> bool {
        self.ready_tasks > 0 || self.in_progress_tasks > 0
    }

    /// Check if all tasks are completed
    pub fn is_complete(&self) -> bool {
        self.total_tasks == 0 && self.in_progress_tasks == 0
    }

    /// Get a human-readable status string
    pub fn status_string(&self) -> String {
        format!(
            "Queue: {} total, {} ready, {} waiting, {} in progress, {} completed",
            self.total_tasks,
            self.ready_tasks,
            self.waiting_tasks,
            self.in_progress_tasks,
            self.completed_tasks
        )
    }
}