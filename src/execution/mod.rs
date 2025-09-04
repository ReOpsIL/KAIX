//! Task execution engine with agentic loop and dual-queue system

use crate::context::{ContextManager, PlanContext};
use crate::llm::LlmProvider;
use crate::planning::{Plan, Task, TaskResult, TaskStatus, TaskType};
use crate::utils::errors::KaiError;
use crate::Result;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{timeout, Duration};
use uuid::Uuid;

pub mod executor;
pub mod queue;

pub use executor::TaskExecutor;
pub use queue::{TaskQueue, QueuePriority};

/// Main execution engine that orchestrates plan execution
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
    task_executor: TaskExecutor,
    /// Currently executing plan
    current_plan: Arc<RwLock<Option<Plan>>>,
    /// Plan context for the current plan
    current_plan_context: Arc<RwLock<Option<PlanContext>>>,
    /// Execution state
    state: Arc<RwLock<ExecutionState>>,
    /// Configuration for execution behavior
    config: ExecutionConfig,
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
    /// Create a new execution engine
    pub fn new(
        context_manager: Arc<RwLock<ContextManager>>,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
        config: Option<ExecutionConfig>,
    ) -> Self {
        let config = config.unwrap_or_default();
        let task_executor = TaskExecutor::new(config.clone());

        Self {
            context_manager,
            llm_provider,
            model,
            user_prompt_queue: Arc::new(RwLock::new(VecDeque::new())),
            main_task_queue: Arc::new(RwLock::new(TaskQueue::new())),
            task_executor,
            current_plan: Arc::new(RwLock::new(None)),
            current_plan_context: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(ExecutionState::Idle)),
            config,
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

    /// Start the main execution loop
    pub async fn start(&self) -> Result<()> {
        {
            let mut state = self.state.write().await;
            if *state != ExecutionState::Idle {
                return Err(KaiError::execution("Execution engine is already running"));
            }
            *state = ExecutionState::Executing;
        }

        // Main agentic loop
        loop {
            // Check if we should stop
            {
                let state = self.state.read().await;
                if *state == ExecutionState::Cancelled {
                    break;
                }
            }

            // Priority 1: Check for user prompts
            if let Some(user_prompt) = self.pop_user_prompt().await {
                self.handle_user_prompt(user_prompt).await?;
                continue;
            }

            // Priority 2: Process tasks from the main queue
            if let Some(task) = self.pop_task().await {
                self.execute_task_with_agentic_loop(task).await?;
                continue;
            }

            // No work to do, wait briefly
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        {
            let mut state = self.state.write().await;
            *state = ExecutionState::Idle;
        }

        Ok(())
    }

    /// Stop the execution engine
    pub async fn stop(&self) {
        let mut state = self.state.write().await;
        *state = ExecutionState::Cancelled;
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

    /// Execute a single task using the agentic loop
    async fn execute_task_with_agentic_loop(&self, mut task: Task) -> Result<()> {
        let start_time = Instant::now();

        // Step 1: Context Assembly
        let context = self.assemble_task_context(&task).await?;

        // Step 2: LLM Pre-Execution Refinement
        let refined_instruction = self.refine_task_instruction(&task, &context).await?;

        // Step 3: Execute Tool
        let raw_result = timeout(
            Duration::from_secs(self.config.default_timeout_seconds),
            self.task_executor.execute_task(&task, &refined_instruction, &context)
        )
        .await
        .map_err(|_| KaiError::timeout(self.config.default_timeout_seconds * 1000))?;

        // Step 4: LLM Post-Execution Analysis
        let analyzed_result = self.analyze_task_result(&task, &raw_result).await?;

        // Step 5: Update State and Context
        let execution_time = start_time.elapsed().as_millis() as u64;
        let task_result = TaskResult {
            success: analyzed_result.success,
            output: analyzed_result.output,
            error: analyzed_result.error,
            execution_time_ms: execution_time,
            metadata: analyzed_result.metadata,
        };

        // Update plan context
        if let Some(plan_context) = &mut *self.current_plan_context.write().await {
            plan_context.add_task_result(task.id.clone(), task_result.clone());
            
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
        if let Some(plan) = &mut *self.current_plan.write().await {
            plan.set_task_result(&task.id, task_result)?;
        }

        // Update global context if files were modified
        if let Some(modified_files) = analyzed_result.modified_files {
            let context_manager = self.context_manager.read().await;
            context_manager.update_global_context_for_files(&modified_files).await?;
        }

        Ok(())
    }

    /// Assemble context for task execution
    async fn assemble_task_context(&self, task: &Task) -> Result<String> {
        let mut context_parts = Vec::new();

        // Add global context summary
        let context_manager = self.context_manager.read().await;
        let global_summary = context_manager.get_global_context_summary().await?;
        context_parts.push(format!("Global Context:\n{}", global_summary));

        // Add plan context if available
        if let Some(plan_context) = &*self.current_plan_context.read().await {
            context_parts.push(format!("Plan Context:\n{}", plan_context.get_summary()));
        }

        // Add dependency outputs
        if let Some(plan_context) = &*self.current_plan_context.read().await {
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

    /// Refine task instruction using LLM
    async fn refine_task_instruction(&self, task: &Task, context: &str) -> Result<String> {
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

        self.llm_provider.generate_content(&prompt, "", &self.model, None).await
            .map_err(|e| KaiError::execution(format!("Failed to refine task instruction: {}", e)))
    }

    /// Analyze task execution result using LLM
    async fn analyze_task_result(&self, task: &Task, raw_result: &TaskExecutionResult) -> Result<AnalyzedTaskResult> {
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

        let analysis = self.llm_provider.generate_content(&prompt, "", &self.model, None).await
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

/// Raw result from task execution
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskExecutionResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
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