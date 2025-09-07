//! Task execution engine with agentic loop and dual-queue system

use crate::context::{ContextManager, PlanContext};
use crate::llm::LlmProvider;
use crate::planning::{Plan, Task, TaskResult, TaskType};
use crate::utils::errors::KaiError;
use crate::Result;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
use futures::stream::{FuturesUnordered, StreamExt};
use tracing::{debug, error, info, warn};

pub mod executor;
pub mod queue;

pub use executor::TaskExecutor;
pub use queue::{TaskQueue, QueuePriority};

/// Analysis of why a task failed
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TaskFailureAnalysis {
    pub summary: String,
    pub error_category: String,
    pub root_cause: String,
    pub suggested_alternatives: Vec<String>,
}

/// Response structure for alternative task generation
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AlternativeTasksResponse {
    pub tasks: Vec<AlternativeTaskSpec>,
}

/// Specification for an alternative task
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AlternativeTaskSpec {
    pub description: String,
    pub task_type: TaskType,
    pub parameters: serde_json::Value,
    pub dependencies: Vec<String>,
}

/// Main execution engine that orchestrates plan execution with parallelization and monitoring
pub struct ExecutionEngine {
    /// Context manager for global and plan contexts
    context_manager: Arc<RwLock<ContextManager>>,
    /// LLM provider for task refinement and analysis
    llm_provider: Arc<dyn LlmProvider>,
    /// Current model to use
    model: String,
    /// User prompt queue (high priority, LIFO)
    user_prompt_queue: Arc<RwLock<VecDeque<UserPrompt>>>,
    /// Main task queue (standard priority, FIFO)
    main_task_queue: Arc<RwLock<TaskQueue>>,
    /// Task executor for individual task execution
    task_executor: Arc<Mutex<TaskExecutor>>,
    /// Currently executing plan
    current_plan: Arc<RwLock<Option<Plan>>>,
    /// Plan context for the current plan
    current_plan_context: Arc<RwLock<Option<PlanContext>>>,
    /// Execution state
    state: Arc<RwLock<ExecutionState>>,
    /// Configuration for execution behavior
    config: ExecutionConfig,
    /// Cancellation token for graceful shutdown
    cancellation_token: CancellationToken,
    /// Currently running tasks (for parallel execution)
    running_tasks: Arc<RwLock<HashMap<String, TaskHandle>>>,
    /// Event broadcaster for monitoring
    event_sender: broadcast::Sender<ExecutionEvent>,
    /// Metrics collector
    metrics: Arc<RwLock<ExecutionMetrics>>,
}

/// User prompt with metadata
#[derive(Debug, Clone)]
pub struct UserPrompt {
    pub id: String,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub priority: PromptPriority,
}

/// Priority levels for user prompts
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PromptPriority {
    Normal,
    Interrupt,
    Emergency,
}

/// Current state of the execution engine
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionState {
    Idle,
    Planning,
    Executing,
    Paused,
    Cancelled,
}

/// Configuration for the execution engine
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    pub max_concurrent_tasks: usize,
    pub default_timeout_seconds: u64,
    pub auto_retry: bool,
    pub max_retries: usize,
    pub pause_on_error: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 4,
            default_timeout_seconds: 300,
            auto_retry: false,
            max_retries: 3,
            pause_on_error: true,
        }
    }
}

impl ExecutionEngine {
    /// Create a new execution engine with working directory
    pub fn new(
        context_manager: Arc<RwLock<ContextManager>>,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
        working_dir: std::path::PathBuf,
        config: Option<ExecutionConfig>,
    ) -> Self {
        let config = config.unwrap_or_default();
        let task_executor = TaskExecutor::new(
            config.clone(), 
            working_dir,
            llm_provider.clone(),
            model.clone()
        );

        let (event_sender, _) = broadcast::channel(1000);
        let cancellation_token = CancellationToken::new();
        
        Self {
            context_manager,
            llm_provider,
            model,
            user_prompt_queue: Arc::new(RwLock::new(VecDeque::new())),
            main_task_queue: Arc::new(RwLock::new(TaskQueue::new())),
            task_executor: Arc::new(Mutex::new(task_executor)),
            current_plan: Arc::new(RwLock::new(None)),
            current_plan_context: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(ExecutionState::Idle)),
            config,
            cancellation_token,
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            metrics: Arc::new(RwLock::new(ExecutionMetrics::new())),
        }
    }

    /// Submit a user prompt to the high-priority queue
    pub async fn submit_user_prompt(&self, content: String, priority: PromptPriority) -> String {
        let prompt = UserPrompt {
            id: Uuid::new_v4().to_string(),
            content,
            timestamp: chrono::Utc::now(),
            priority,
        };

        let id = prompt.id.clone();
        let mut queue = self.user_prompt_queue.write().await;
        
        // Insert based on priority (LIFO for same priority)
        match prompt.priority {
            PromptPriority::Emergency => queue.push_front(prompt),
            PromptPriority::Interrupt => {
                // Insert after any emergency prompts
                let mut insert_pos = 0;
                for (i, existing) in queue.iter().enumerate() {
                    if existing.priority < PromptPriority::Interrupt {
                        insert_pos = i;
                        break;
                    }
                }
                queue.insert(insert_pos, prompt);
            }
            PromptPriority::Normal => queue.push_back(prompt),
        }

        id
    }

    /// Start the main execution loop with parallel task execution
    pub async fn start(&self) -> Result<()> {
        {
            let mut state = self.state.write().await;
            if *state != ExecutionState::Idle {
                return Err(KaiError::execution("Execution engine is already running"));
            }
            *state = ExecutionState::Executing;
        }

        info!("Starting execution engine with max {} concurrent tasks", self.config.max_concurrent_tasks);
        self.emit_event(ExecutionEvent::EngineStarted).await;

        // Create futures for parallel processing
        let mut futures = FuturesUnordered::new();
        let mut last_work_check = Instant::now();
        let work_check_interval = Duration::from_millis(100);

        // Main agentic loop with parallel execution
        loop {
            // Check cancellation
            if self.cancellation_token.is_cancelled() {
                info!("Cancellation requested, shutting down execution loop");
                break;
            }

            // Check state
            {
                let state = self.state.read().await;
                if *state == ExecutionState::Cancelled {
                    break;
                }
                if *state == ExecutionState::Paused {
                    sleep(Duration::from_millis(100)).await;
                    continue;
                }
            }

            // Priority 1: Handle completed tasks
            if let Some(result) = futures.next().await {
                self.handle_task_completion(result).await?;
                continue;
            }

            // Priority 2: Check for user prompts (every cycle)
            if let Some(user_prompt) = self.pop_user_prompt().await {
                self.handle_user_prompt(user_prompt).await?;
                continue;
            }

            // Priority 3: Start new tasks if we have capacity
            let current_running = self.running_tasks.read().await.len();
            if current_running < self.config.max_concurrent_tasks {
                if let Some(task) = self.pop_task().await {
                    let task_handle = self.start_task_execution(task).await?;
                    futures.push(task_handle);
                    continue;
                }
            }

            // Priority 4: Periodic maintenance (less frequent)
            if last_work_check.elapsed() > work_check_interval {
                self.perform_maintenance().await?;
                last_work_check = Instant::now();
            }

            // Brief pause to prevent busy waiting
            sleep(Duration::from_millis(10)).await;
        }

        // Wait for all remaining tasks to complete
        while let Some(result) = futures.next().await {
            self.handle_task_completion(result).await?;
        }

        {
            let mut state = self.state.write().await;
            *state = ExecutionState::Idle;
        }

        self.emit_event(ExecutionEvent::EngineStopped).await;
        info!("Execution engine stopped");
        Ok(())
    }

    /// Stop the execution engine with graceful shutdown
    pub async fn stop(&self) {
        info!("Stopping execution engine...");
        
        // Signal cancellation to all running tasks
        self.cancellation_token.cancel();
        
        // Wait for all running tasks to complete or timeout
        let timeout_duration = Duration::from_secs(30);
        let start = Instant::now();
        
        while !self.running_tasks.read().await.is_empty() && start.elapsed() < timeout_duration {
            debug!("Waiting for {} tasks to complete...", self.running_tasks.read().await.len());
            sleep(Duration::from_millis(100)).await;
        }
        
        if !self.running_tasks.read().await.is_empty() {
            warn!("Force stopping {} remaining tasks", self.running_tasks.read().await.len());
            let mut tasks = self.running_tasks.write().await;
            for (task_id, handle) in tasks.drain() {
                warn!("Force cancelling task: {}", task_id);
                handle.abort();
            }
        }
        
        let mut state = self.state.write().await;
        *state = ExecutionState::Cancelled;
        
        info!("Execution engine stopped");
    }

    /// Pause the execution engine
    pub async fn pause(&self) {
        let mut state = self.state.write().await;
        if *state == ExecutionState::Executing {
            *state = ExecutionState::Paused;
        }
    }

    /// Resume the execution engine
    pub async fn resume(&self) {
        let mut state = self.state.write().await;
        if *state == ExecutionState::Paused {
            *state = ExecutionState::Executing;
        }
    }

    /// Get the current execution state
    pub async fn get_state(&self) -> ExecutionState {
        self.state.read().await.clone()
    }

    /// Get the current plan
    pub async fn get_current_plan(&self) -> Option<Plan> {
        self.current_plan.read().await.clone()
    }

    /// Pop the next user prompt from the queue
    async fn pop_user_prompt(&self) -> Option<UserPrompt> {
        let mut queue = self.user_prompt_queue.write().await;
        queue.pop_front()
    }

    /// Pop the next task from the main queue
    async fn pop_task(&self) -> Option<Task> {
        let mut queue = self.main_task_queue.write().await;
        queue.pop_ready_task()
    }

    /// Handle a user prompt by generating and queuing a new plan
    async fn handle_user_prompt(&self, prompt: UserPrompt) -> Result<()> {
        {
            let mut state = self.state.write().await;
            *state = ExecutionState::Planning;
        }

        // Get global context summary
        let context_manager = self.context_manager.read().await;
        let global_context = context_manager.get_global_context_summary().await?;

        // Generate a new plan
        let plan = self.llm_provider
            .generate_plan(&prompt.content, &global_context, &self.model)
            .await?;

        // Handle plan based on priority
        match prompt.priority {
            PromptPriority::Emergency => {
                // Replace current plan entirely
                self.replace_current_plan(plan).await?;
            }
            PromptPriority::Interrupt => {
                // Pause current plan and insert new plan
                self.interrupt_with_plan(plan).await?;
            }
            PromptPriority::Normal => {
                // Queue plan after current plan completes
                self.queue_plan(plan).await?;
            }
        }

        {
            let mut state = self.state.write().await;
            *state = ExecutionState::Executing;
        }

        Ok(())
    }

    /// Start execution of a single task asynchronously
    async fn start_task_execution(&self, task: Task) -> Result<TaskExecutionFuture> {
        let task_id = task.id.clone();
        let start_time = Instant::now();
        
        debug!("Starting task execution: {} ({})", task.description, task_id);
        self.emit_event(ExecutionEvent::TaskStarted {
            task_id: task_id.clone(),
            task_type: task.task_type.clone(),
            description: task.description.clone(),
        }).await;

        // Clone necessary data for async task
        let context_manager = self.context_manager.clone();
        let llm_provider = self.llm_provider.clone();
        let model = self.model.clone();
        let task_executor = self.task_executor.clone();
        let current_plan = self.current_plan.clone();
        let current_plan_context = self.current_plan_context.clone();
        let timeout_duration = Duration::from_secs(self.config.default_timeout_seconds);
        let running_tasks = self.running_tasks.clone();
        let event_sender = self.event_sender.clone();
        let cancellation_token = self.cancellation_token.child_token();

        // Create async task
        let handle = tokio::spawn(async move {
            // Register as running task
            {
                let mut tasks = running_tasks.write().await;
                let task_handle = TaskHandle {
                    join_handle: task_id.clone(),
                    start_time,
                    task_type: task.task_type.clone(),
                };
                tasks.insert(task_id.clone(), task_handle);
            }

            let result = Self::execute_single_task_with_cancellation(
                task,
                context_manager,
                llm_provider,
                model,
                task_executor,
                current_plan,
                current_plan_context,
                timeout_duration,
                cancellation_token,
                event_sender,
            ).await;

            // Unregister as running task
            {
                let mut tasks = running_tasks.write().await;
                tasks.remove(&task_id);
            }

            TaskExecutionWrapper {
                task_id,
                result,
                execution_time: start_time.elapsed(),
            }
        });

        Ok(TaskExecutionFuture { handle })
    }

    /// Execute a single task with cancellation support
    async fn execute_single_task_with_cancellation(
        task: Task,
        context_manager: Arc<RwLock<ContextManager>>,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
        task_executor: Arc<Mutex<TaskExecutor>>,
        current_plan: Arc<RwLock<Option<Plan>>>,
        current_plan_context: Arc<RwLock<Option<PlanContext>>>,
        timeout_duration: Duration,
        cancellation_token: CancellationToken,
        event_sender: broadcast::Sender<ExecutionEvent>,
    ) -> Result<TaskResult> {
        // Step 1: Context Assembly
        let context = Self::assemble_task_context_static(&task, &context_manager, &current_plan_context).await?;

        // Step 2: LLM Pre-Execution Refinement
        let refined_instruction = Self::refine_task_instruction_static(
            &task, &context, &llm_provider, &model
        ).await?;

        // Check cancellation before execution
        if cancellation_token.is_cancelled() {
            return Err(KaiError::cancelled(format!("Task {} was cancelled", task.id)));
        }

        // Step 3: Execute Tool with timeout and cancellation
        let execution_result = {
            let mut executor = task_executor.lock().await;
            tokio::select! {
                result = executor.execute_task(&task, &refined_instruction, &context) => {
                    result?
                }
                _ = cancellation_token.cancelled() => {
                    return Err(KaiError::cancelled(format!("Task {} was cancelled during execution", task.id)));
                }
                _ = sleep(timeout_duration) => {
                    return Err(KaiError::timeout(timeout_duration.as_millis() as u64));
                }
            }
        };

        // Step 4: LLM Post-Execution Analysis
        let analyzed_result = Self::analyze_task_result_static(
            &task, &execution_result, &llm_provider, &model
        ).await?;

        // Step 5: Update State and Context
        let task_result = TaskResult {
            success: analyzed_result.success,
            output: analyzed_result.output,
            error: analyzed_result.error,
            execution_time_ms: 0, // Will be filled by caller
            metadata: analyzed_result.metadata,
        };

        // Update plan context
        if let Some(plan_context) = &mut *current_plan_context.write().await {
            plan_context.add_task_result(task.id.clone(), task.description.clone(), task_result.clone());
            
            if let Some(extracted_data) = analyzed_result.extracted_data {
                plan_context.add_output(
                    task.id.clone(),
                    task.description.clone(),
                    extracted_data,
                    task.task_type.to_string(),
                );
            }
        }

        // Update plan with task result
        if let Some(plan) = &mut *current_plan.write().await {
            plan.set_task_result(&task.id, task_result.clone())?;
        }

        // Update global context if files were modified
        if let Some(modified_files) = analyzed_result.modified_files {
            let context_mgr = context_manager.read().await;
            context_mgr.update_global_context_for_files(&modified_files).await?;
        }

        // Emit completion event
        let _ = event_sender.send(ExecutionEvent::TaskCompleted {
            task_id: task.id,
            success: task_result.success,
            execution_time_ms: 0, // Will be updated
        });

        Ok(task_result)
    }

    /// Assemble context for task execution (static version for async tasks)
    async fn assemble_task_context_static(
        task: &Task,
        context_manager: &Arc<RwLock<ContextManager>>,
        current_plan_context: &Arc<RwLock<Option<PlanContext>>>,
    ) -> Result<String> {
        let mut context_parts = Vec::new();

        // Add global context summary
        let context_mgr = context_manager.read().await;
        let global_summary = context_mgr.get_global_context_summary().await?;
        context_parts.push(format!("Global Context:\n{}", global_summary));

        // Add plan context if available
        if let Some(plan_context) = &*current_plan_context.read().await {
            context_parts.push(format!("Plan Context:\n{}", plan_context.get_summary()));
        }

        // Add dependency outputs
        if let Some(plan_context) = &*current_plan_context.read().await {
            for dep_id in &task.dependencies {
                if let Some(dep_result) = plan_context.get_task_result(dep_id) {
                    if let Some(output) = &dep_result.output {
                        context_parts.push(format!(
                            "Dependency Output ({}): {}", 
                            dep_id, 
                            serde_json::to_string_pretty(output).unwrap_or_default()
                        ));
                    }
                }
            }
        }

        Ok(context_parts.join("\n\n"))
    }

    /// Refine task instruction using LLM (static version)
    async fn refine_task_instruction_static(
        task: &Task, 
        context: &str,
        llm_provider: &Arc<dyn LlmProvider>,
        model: &str,
    ) -> Result<String> {
        let prompt = format!(
            "You are about to execute a task. Based on the context provided, \
            generate the specific, concrete instruction to execute this task.\n\n\
            Task Type: {:?}\n\
            Task Description: {}\n\
            Task Parameters: {}\n\n\
            Context:\n{}\n\n\
            Provide only the concrete, executable instruction:",
            task.task_type,
            task.description,
            serde_json::to_string_pretty(&task.parameters).unwrap_or_default(),
            context
        );

        llm_provider.generate_content(&prompt, "", model, None).await
            .map_err(|e| KaiError::execution(format!("Failed to refine task instruction: {}", e)))
    }

    /// Analyze task execution result using LLM (static version)
    async fn analyze_task_result_static(
        task: &Task, 
        raw_result: &crate::llm::TaskExecutionResult,
        llm_provider: &Arc<dyn LlmProvider>,
        model: &str,
    ) -> Result<AnalyzedTaskResult> {
        let prompt = format!(
            "Analyze the result of executing this task and provide structured feedback:\n\n\
            Task: {}\n\
            Task Type: {:?}\n\
            Raw Result: {}\n\n\
            Provide analysis in the following format:\n\
            SUCCESS: true/false\n\
            EXTRACTED_DATA: any important data from the output\n\
            ERROR_MESSAGE: if failed, what went wrong\n\
            MODIFIED_FILES: list of files that were modified\n\
            METADATA: any additional relevant information",
            task.description,
            task.task_type,
            serde_json::to_string_pretty(raw_result).unwrap_or_default()
        );

        let analysis = llm_provider.generate_content(&prompt, "", model, None).await
            .map_err(|e| KaiError::execution(format!("Failed to analyze task result: {}", e)))?;

        // Parse the analysis (simplified - in real implementation, use structured output)
        let success = analysis.contains("SUCCESS: true");
        let error = if !success {
            Some(raw_result.error.clone().unwrap_or_else(|| "Task failed".to_string()))
        } else {
            None
        };

        Ok(AnalyzedTaskResult {
            success,
            output: raw_result.output.clone(),
            error,
            extracted_data: raw_result.output.clone(),
            modified_files: None, // Would be extracted from analysis in real implementation
            metadata: HashMap::new(),
        })
    }

    /// Replace the current plan with a new one
    async fn replace_current_plan(&self, plan: Plan) -> Result<()> {
        // Clear current plan and context
        {
            let mut current_plan = self.current_plan.write().await;
            *current_plan = Some(plan.clone());
        }

        {
            let mut current_context = self.current_plan_context.write().await;
            *current_context = Some(PlanContext::new(plan.id.clone()));
        }

        // Clear main task queue and add new tasks
        {
            let mut queue = self.main_task_queue.write().await;
            queue.clear();
            for task in &plan.tasks {
                queue.add_task(task.clone(), QueuePriority::Normal);
            }
        }

        Ok(())
    }

    /// Interrupt current plan with a new plan
    async fn interrupt_with_plan(&self, plan: Plan) -> Result<()> {
        // For now, just replace the plan
        // TODO: Implement proper interruption with plan merging
        self.replace_current_plan(plan).await
    }

    /// Queue a plan to execute after current plan completes
    async fn queue_plan(&self, plan: Plan) -> Result<()> {
        // For now, just replace the plan
        // TODO: Implement proper plan queuing
        self.replace_current_plan(plan).await
    }
}


impl ExecutionEngine {
    /// Handle task completion
    async fn handle_task_completion(&self, result: TaskExecutionWrapper) -> Result<()> {
        let execution_time_ms = result.execution_time.as_millis() as u64;
        
        match result.result {
            Ok(mut task_result) => {
                // Update execution time
                task_result.execution_time_ms = execution_time_ms;
                
                debug!("Task {} completed successfully in {}ms", result.task_id, execution_time_ms);
                
                // Update metrics
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.task_completed(execution_time_ms, true);
                }

                // Mark task as completed in queue
                {
                    let mut queue = self.main_task_queue.write().await;
                    queue.mark_task_completed(&result.task_id);
                }

                self.emit_event(ExecutionEvent::TaskCompleted {
                    task_id: result.task_id.clone(),
                    success: true,
                    execution_time_ms,
                }).await;
            }
            Err(e) => {
                error!("Task {} failed: {}", result.task_id, e);
                
                // Update metrics
                {
                    let mut metrics = self.metrics.write().await;
                    metrics.task_completed(execution_time_ms, false);
                }

                // Mark task as failed in queue
                {
                    let mut queue = self.main_task_queue.write().await;
                    queue.mark_task_failed(&result.task_id);
                }

                self.emit_event(ExecutionEvent::TaskCompleted {
                    task_id: result.task_id.clone(),
                    success: false,
                    execution_time_ms,
                }).await;

                // Handle retry logic if enabled
                if self.config.auto_retry {
                    // Implement adaptive task decomposition for failures
                    if let Err(decomp_error) = self.handle_adaptive_task_decomposition(&result.task_id, &e).await {
                        error!("Failed to perform adaptive task decomposition for task {}: {}", result.task_id, decomp_error);
                        warn!("Falling back to simple retry for failed task: {}", result.task_id);
                    }
                }

                // Pause on error if configured
                if self.config.pause_on_error {
                    warn!("Pausing execution due to task failure: {}", result.task_id);
                    self.pause().await;
                }
            }
        }

        Ok(())
    }

    /// Handle adaptive task decomposition when a task fails
    async fn handle_adaptive_task_decomposition(&self, task_id: &str, error: &KaiError) -> Result<()> {
        info!("🔄 Starting adaptive task decomposition for failed task: {}", task_id);
        
        // Step 1: Retrieve the failed task from the current plan
        let failed_task = {
            let plan = self.current_plan.read().await;
            if let Some(plan) = &*plan {
                plan.tasks.iter()
                    .find(|task| task.id == task_id)
                    .cloned()
            } else {
                None
            }
        };

        let failed_task = match failed_task {
            Some(task) => task,
            None => {
                error!("Could not find failed task {} in current plan", task_id);
                return Err(KaiError::execution(format!("Task {} not found in current plan", task_id)));
            }
        };

        // Step 2: Analyze the failure with LLM
        let failure_analysis = self.analyze_task_failure(&failed_task, error).await?;
        info!("📊 Failure analysis complete: {}", failure_analysis.summary);

        // Step 3: Generate alternative task decomposition
        let alternative_tasks = self.generate_alternative_tasks(&failed_task, &failure_analysis).await?;
        info!("🔨 Generated {} alternative tasks for failed task", alternative_tasks.len());

        // Step 4: Add alternative tasks to the queue
        self.add_alternative_tasks_to_queue(alternative_tasks).await?;
        info!("✅ Alternative tasks added to execution queue");

        Ok(())
    }

    /// Analyze why a task failed using LLM
    async fn analyze_task_failure(&self, task: &Task, error: &KaiError) -> Result<TaskFailureAnalysis> {
        let prompt = format!(r#"
Analyze this failed task and provide insight into why it failed and how to fix it.

Task Details:
- Description: {}
- Type: {:?}
- Dependencies: {:?}
- Parameters: {}

Error Details:
- Error: {}

Please provide a JSON response with the following structure:
{{
    "summary": "Brief summary of why the task failed",
    "error_category": "One of: dependency_missing, command_not_found, permission_denied, network_error, syntax_error, resource_exhausted, timeout, other",
    "root_cause": "Detailed explanation of the root cause",
    "suggested_alternatives": [
        "Alternative approach 1",
        "Alternative approach 2"
    ]
}}
"#, 
            task.description,
            task.task_type,
            task.dependencies,
            serde_json::to_string_pretty(&task.parameters).unwrap_or_default(),
            error
        );

        let response = self.llm_provider
            .generate_content(&prompt, "", &self.model, None)
            .await
            .map_err(|e| KaiError::provider("llm", format!("Failed to analyze task failure: {}", e)))?;

        let analysis: TaskFailureAnalysis = serde_json::from_str(&response)
            .map_err(|e| KaiError::unknown(format!("Failed to parse failure analysis: {}", e)))?;

        Ok(analysis)
    }

    /// Generate alternative tasks using LLM
    async fn generate_alternative_tasks(&self, failed_task: &Task, analysis: &TaskFailureAnalysis) -> Result<Vec<Task>> {
        let prompt = format!(r#"
Based on this failed task and failure analysis, generate alternative tasks that should accomplish the same goal.

Failed Task:
- Description: {}
- Type: {:?}
- Parameters: {}

Failure Analysis:
- Summary: {}
- Root Cause: {}
- Error Category: {}

Context:
- Break down the original task into smaller, more specific tasks
- Address the root cause identified in the analysis
- Use more robust approaches (e.g., check dependencies first, use alternative tools)
- Keep the same overall goal but use different implementation strategies

Please provide a JSON response with the following structure:
{{
    "tasks": [
        {{
            "description": "Detailed description of the alternative task",
            "task_type": "One of: FileRead, FileWrite, ShellCommand, HttpRequest, Analysis",
            "parameters": {{
                "key": "value pairs specific to the task type"
            }},
            "dependencies": []
        }}
    ]
}}
"#,
            failed_task.description,
            failed_task.task_type,
            serde_json::to_string_pretty(&failed_task.parameters).unwrap_or_default(),
            analysis.summary,
            analysis.root_cause,
            analysis.error_category
        );

        let response = self.llm_provider
            .generate_content(&prompt, "", &self.model, None)
            .await
            .map_err(|e| KaiError::provider("llm", format!("Failed to generate alternative tasks: {}", e)))?;

        let alternative_response: AlternativeTasksResponse = serde_json::from_str(&response)
            .map_err(|e| KaiError::unknown(format!("Failed to parse alternative tasks: {}", e)))?;

        // Convert to proper Task structs with generated IDs
        let mut alternative_tasks = Vec::new();
        for (index, task_spec) in alternative_response.tasks.into_iter().enumerate() {
            let task = Task {
                id: format!("{}-alt-{}", failed_task.id, index + 1),
                description: task_spec.description,
                task_type: task_spec.task_type,
                parameters: {
                    if let serde_json::Value::Object(obj) = task_spec.parameters {
                        obj.into_iter().collect()
                    } else {
                        std::collections::HashMap::new()
                    }
                },
                dependencies: task_spec.dependencies,
                status: crate::planning::TaskStatus::Pending,
                result: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            alternative_tasks.push(task);
        }

        Ok(alternative_tasks)
    }

    /// Add alternative tasks to the execution queue
    async fn add_alternative_tasks_to_queue(&self, tasks: Vec<Task>) -> Result<()> {
        let mut queue = self.main_task_queue.write().await;
        for task in tasks {
            queue.add_task(task, QueuePriority::High); // Use high priority for alternative tasks
        }
        Ok(())
    }

    /// Perform periodic maintenance
    async fn perform_maintenance(&self) -> Result<()> {
        // Clean up completed tasks from running_tasks map
        let mut cleanup_count = 0;
        {
            let mut running = self.running_tasks.write().await;
            let now = Instant::now();
            running.retain(|task_id, handle| {
                // Keep tasks that are still recent (within the last hour)
                let keep = now.duration_since(handle.start_time) < Duration::from_secs(3600);
                if !keep {
                    cleanup_count += 1;
                    debug!("Cleaning up old task handle: {}", task_id);
                }
                keep
            });
        }

        if cleanup_count > 0 {
            debug!("Cleaned up {} old task handles", cleanup_count);
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.cleanup_old_entries();
        }

        Ok(())
    }

    /// Emit an execution event
    async fn emit_event(&self, event: ExecutionEvent) {
        if let Err(e) = self.event_sender.send(event) {
            // Only log if there are actually receivers
            if self.event_sender.receiver_count() > 0 {
                warn!("Failed to emit execution event: {}", e);
            }
        }
    }

    /// Subscribe to execution events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.event_sender.subscribe()
    }

    /// Get current execution metrics
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        self.metrics.read().await.clone()
    }

    /// Get task executor resource stats
    pub async fn get_executor_stats(&self) -> executor::ResourceStats {
        self.task_executor.lock().await.get_resource_stats()
    }

    /// Get current running tasks summary
    pub async fn get_running_tasks_summary(&self) -> Vec<RunningTaskSummary> {
        let running = self.running_tasks.read().await;
        let now = Instant::now();
        
        running.iter().map(|(task_id, handle)| {
            RunningTaskSummary {
                task_id: task_id.clone(),
                task_type: handle.task_type.clone(),
                running_time: now.duration_since(handle.start_time),
                start_time: handle.start_time,
            }
        }).collect()
    }

    /// Get current queue summary
    pub async fn get_queue_summary(&self) -> queue::QueueSummary {
        self.main_task_queue.read().await.get_summary()
    }
}

/// Task handle for tracking running tasks
#[derive(Debug, Clone)]
pub struct TaskHandle {
    pub join_handle: String,
    pub start_time: Instant,
    pub task_type: TaskType,
}

impl TaskHandle {
    pub fn abort(&self) {
        // Note: We can't actually abort a task by ID in tokio
        // In a real implementation, we'd need to store the JoinHandle
        warn!("Task abort requested for task ID: {:?}", self.join_handle);
    }
}

/// Future wrapper for task execution
pub struct TaskExecutionFuture {
    pub handle: tokio::task::JoinHandle<TaskExecutionWrapper>,
}

impl std::future::Future for TaskExecutionFuture {
    type Output = TaskExecutionWrapper;
    
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        use std::pin::Pin;
        use std::task::Poll;
        
        match Pin::new(&mut self.handle).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(e)) => {
                // Handle join error
                error!("Task execution future failed: {}", e);
                Poll::Ready(TaskExecutionWrapper {
                    task_id: "unknown".to_string(),
                    result: Err(KaiError::execution(format!("Task execution failed: {}", e))),
                    execution_time: Duration::ZERO,
                })
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Result of task execution with metadata
#[derive(Debug)]
pub struct TaskExecutionWrapper {
    pub task_id: String,
    pub result: Result<TaskResult>,
    pub execution_time: Duration,
}

/// Execution events for monitoring
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    EngineStarted,
    EngineStopped,
    TaskStarted {
        task_id: String,
        task_type: TaskType,
        description: String,
    },
    TaskCompleted {
        task_id: String,
        success: bool,
        execution_time_ms: u64,
    },
    PlanStarted {
        plan_id: String,
        description: String,
    },
    PlanCompleted {
        plan_id: String,
        success: bool,
        total_tasks: usize,
    },
}

/// Execution metrics for monitoring
#[derive(Debug, Clone)]
pub struct ExecutionMetrics {
    pub tasks_executed: u64,
    pub tasks_successful: u64,
    pub tasks_failed: u64,
    pub total_execution_time_ms: u64,
    pub average_execution_time_ms: u64,
    pub engine_uptime: Duration,
    pub start_time: Instant,
}

impl ExecutionMetrics {
    pub fn new() -> Self {
        Self {
            tasks_executed: 0,
            tasks_successful: 0,
            tasks_failed: 0,
            total_execution_time_ms: 0,
            average_execution_time_ms: 0,
            engine_uptime: Duration::ZERO,
            start_time: Instant::now(),
        }
    }

    pub fn task_completed(&mut self, execution_time_ms: u64, success: bool) {
        self.tasks_executed += 1;
        self.total_execution_time_ms += execution_time_ms;
        
        if success {
            self.tasks_successful += 1;
        } else {
            self.tasks_failed += 1;
        }

        self.average_execution_time_ms = if self.tasks_executed > 0 {
            self.total_execution_time_ms / self.tasks_executed
        } else {
            0
        };

        self.engine_uptime = self.start_time.elapsed();
    }

    pub fn cleanup_old_entries(&mut self) {
        // Update uptime
        self.engine_uptime = self.start_time.elapsed();
    }

    pub fn success_rate(&self) -> f64 {
        if self.tasks_executed == 0 {
            0.0
        } else {
            self.tasks_successful as f64 / self.tasks_executed as f64
        }
    }
}

/// Summary of a running task
#[derive(Debug, Clone)]
pub struct RunningTaskSummary {
    pub task_id: String,
    pub task_type: TaskType,
    pub running_time: Duration,
    pub start_time: Instant,
}

/// Analyzed result after LLM processing
#[derive(Debug)]
struct AnalyzedTaskResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub extracted_data: Option<serde_json::Value>,
    pub modified_files: Option<Vec<std::path::PathBuf>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ToString for TaskType {
    fn to_string(&self) -> String {
        match self {
            TaskType::ReadFile => "read_file".to_string(),
            TaskType::WriteFile => "write_file".to_string(),
            TaskType::ExecuteCommand => "execute_command".to_string(),
            TaskType::GenerateContent => "generate_content".to_string(),
            TaskType::AnalyzeCode => "analyze_code".to_string(),
            TaskType::ListFiles => "list_files".to_string(),
            TaskType::CreateDirectory => "create_directory".to_string(),
            TaskType::Delete => "delete".to_string(),
        }
    }
}