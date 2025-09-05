//! Agentic planning coordinator - the "brain" of the KAI-X system
//!
//! This module implements the core agentic planning and execution loop as described
//! in Section XV of the specification. It orchestrates the entire workflow through:
//! - Dual priority queue system (user prompts vs main tasks)
//! - Hierarchical task decomposition with recursive refinement
//! - Context-aware planning using global and plan contexts
//! - Interruptible execution with graceful plan modification
//! - LLM-powered task refinement and post-execution analysis

use super::{Plan, PlanStatus, Task, TaskStatus, TaskType, TaskResult};
use crate::{
    context::{ContextManager, PlanContext},
    execution::TaskExecutor,
    llm::{LlmProvider, TaskRefinementContext, TaskExecutionResult, TaskAnalysis},
    utils::errors::KaiError,
    Result,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::Instant;
use tokio::sync::{mpsc, RwLock, broadcast};
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Messages for controlling plan execution
#[derive(Debug, Clone)]
pub enum PlanManagerMessage {
    /// Start executing a plan
    StartPlan(Plan),
    /// Pause the current plan
    PausePlan,
    /// Resume the paused plan
    ResumePlan,
    /// Cancel the current plan
    CancelPlan,
    /// Add a new user request (high priority)
    UserRequest(UserPrompt),
    /// Modify the current plan
    ModifyPlan(Plan),
    /// Request current status
    GetStatus,
    /// Force task decomposition for abstract tasks
    DecomposeTask(String), // task_id
    /// Shutdown the manager
    Shutdown,
}

/// User prompt with priority and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPrompt {
    pub id: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub priority: PromptPriority,
    pub requires_new_plan: bool,
    pub context_hint: Option<String>,
}

/// Priority levels for user prompts - implements LIFO for high priority
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PromptPriority {
    /// Normal user request - goes to end of queue
    Normal,
    /// Interrupt current execution - goes to front of queue  
    Interrupt,
    /// Emergency stop - clears queue and executes immediately
    Emergency,
}

/// Status updates from the agentic coordinator
#[derive(Debug, Clone, Serialize)]
pub struct CoordinatorStatus {
    pub current_plan: Option<PlanStatusInfo>,
    pub user_prompt_queue_size: usize,
    pub main_task_queue_size: usize,
    pub execution_state: ExecutionState,
    pub current_task: Option<TaskStatusInfo>,
    pub performance_metrics: PerformanceMetrics,
}

/// Plan status information for UI
#[derive(Debug, Clone, Serialize)]
pub struct PlanStatusInfo {
    pub id: String,
    pub description: String,
    pub status: PlanStatus,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Task status information for UI
#[derive(Debug, Clone, Serialize)]
pub struct TaskStatusInfo {
    pub id: String,
    pub description: String,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub execution_time_ms: Option<u64>,
    pub progress: Option<f32>, // 0.0 to 1.0
}

/// Current execution state
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ExecutionState {
    /// Idle - waiting for work
    Idle,
    /// Processing a user prompt to generate plan
    Planning,
    /// Assembling context for task execution
    ContextAssembly,
    /// Refining task with LLM
    TaskRefinement,
    /// Executing primitive task
    TaskExecution,
    /// Analyzing execution results with LLM
    ResultAnalysis,
    /// Updating contexts and plan state
    StateUpdate,
    /// Paused by user
    Paused,
    /// Cancelled
    Cancelled,
    /// Shutting down
    Shutdown,
}

/// Performance metrics for monitoring
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceMetrics {
    pub tasks_processed: u64,
    pub plans_generated: u64,
    pub user_interruptions: u64,
    pub decompositions_performed: u64,
    pub average_task_time_ms: f64,
    pub llm_calls_made: u64,
    pub context_updates: u64,
    pub uptime_seconds: u64,
}

/// The Agentic Planning Coordinator - orchestrates complex multi-step workflows
/// 
/// This is the central intelligence that transforms high-level objectives into
/// concrete, executable operations through systematic task decomposition and 
/// execution management. Implements the core agentic loop from Section XV of the spec.
pub struct AgenticPlanningCoordinator {
    /// The currently active plan
    current_plan: Arc<RwLock<Option<Plan>>>,
    /// Current plan execution context
    current_plan_context: Arc<RwLock<Option<PlanContext>>>,
    
    /// User Prompt Queue (High Priority LIFO) - per spec Section IV.A
    user_prompt_queue: Arc<RwLock<VecDeque<UserPrompt>>>,
    /// Main Task Queue (FIFO) - per spec Section IV.A  
    main_task_queue: Arc<RwLock<VecDeque<Task>>>,
    
    /// Task executor for primitive operations
    task_executor: Arc<RwLock<TaskExecutor>>,
    /// Context manager for global and plan context
    context_manager: Arc<RwLock<ContextManager>>,
    /// LLM provider for plan generation, task refinement, and analysis
    llm_provider: Arc<dyn LlmProvider>,
    
    /// Communication channels
    message_receiver: mpsc::UnboundedReceiver<PlanManagerMessage>,
    message_sender: mpsc::UnboundedSender<PlanManagerMessage>,
    status_broadcaster: broadcast::Sender<CoordinatorStatus>,
    
    /// Configuration
    current_model: String,
    config: CoordinatorConfig,
    
    /// State tracking
    execution_state: Arc<RwLock<ExecutionState>>,
    start_time: Instant,
    metrics: Arc<RwLock<PerformanceMetrics>>,
    
    /// Shutdown signal
    shutdown_requested: Arc<RwLock<bool>>,
}

/// Configuration for the agentic coordinator
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// Maximum recursion depth for task decomposition
    pub max_decomposition_depth: usize,
    /// Maximum tasks allowed in a plan
    pub max_plan_size: usize,
    /// Timeout for individual LLM calls (ms)
    pub llm_timeout_ms: u64,
    /// Timeout for task execution (ms)  
    pub task_timeout_ms: u64,
    /// Maximum user prompt queue size
    pub max_user_prompt_queue: usize,
    /// Whether to auto-decompose abstract tasks
    pub auto_decompose_abstract_tasks: bool,
    /// Parallel task execution limit
    pub max_parallel_tasks: usize,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            max_decomposition_depth: 5,
            max_plan_size: 100,
            llm_timeout_ms: 30_000,
            task_timeout_ms: 300_000,
            max_user_prompt_queue: 50,
            auto_decompose_abstract_tasks: true,
            max_parallel_tasks: 1, // Start with sequential execution
        }
    }
}

/// Legacy alias for backward compatibility
pub type PlanManager = AgenticPlanningCoordinator;

impl AgenticPlanningCoordinator {
    /// Create a new agentic planning coordinator
    pub fn new(
        task_executor: TaskExecutor,
        context_manager: ContextManager,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
        config: Option<CoordinatorConfig>,
    ) -> Self {
        let (msg_sender, msg_receiver) = mpsc::unbounded_channel();
        let (status_sender, _) = broadcast::channel(100);
        let config = config.unwrap_or_default();
        let start_time = Instant::now();
        
        Self {
            current_plan: Arc::new(RwLock::new(None)),
            current_plan_context: Arc::new(RwLock::new(None)),
            user_prompt_queue: Arc::new(RwLock::new(VecDeque::new())),
            main_task_queue: Arc::new(RwLock::new(VecDeque::new())),
            task_executor: Arc::new(RwLock::new(task_executor)),
            context_manager: Arc::new(RwLock::new(context_manager)),
            llm_provider,
            message_receiver: msg_receiver,
            message_sender: msg_sender,
            status_broadcaster: status_sender,
            current_model: model,
            config,
            execution_state: Arc::new(RwLock::new(ExecutionState::Idle)),
            start_time,
            metrics: Arc::new(RwLock::new(PerformanceMetrics {
                tasks_processed: 0,
                plans_generated: 0,
                user_interruptions: 0,
                decompositions_performed: 0,
                average_task_time_ms: 0.0,
                llm_calls_made: 0,
                context_updates: 0,
                uptime_seconds: 0,
            })),
            shutdown_requested: Arc::new(RwLock::new(false)),
        }
    }

    /// Get a message sender for external communication
    pub fn get_message_sender(&self) -> mpsc::UnboundedSender<PlanManagerMessage> {
        self.message_sender.clone()
    }
    
    /// Get a status receiver for monitoring
    pub fn get_status_receiver(&self) -> broadcast::Receiver<CoordinatorStatus> {
        self.status_broadcaster.subscribe()
    }

    /// Submit a user prompt (convenience method)
    pub async fn submit_user_prompt(&self, content: String, priority: PromptPriority) -> Result<String> {
        let prompt = UserPrompt {
            id: Uuid::new_v4().to_string(),
            content,
            timestamp: Utc::now(),
            priority,
            requires_new_plan: true,
            context_hint: None,
        };
        
        let id = prompt.id.clone();
        self.message_sender.send(PlanManagerMessage::UserRequest(prompt))
            .map_err(|e| KaiError::planning(format!("Failed to submit user prompt: {}", e)))?;
        
        Ok(id)
    }

    /// Start the main agentic execution loop - implements Section XV of the spec
    /// 
    /// This is the core orchestration loop that processes the dual priority queues:
    /// 1. User Prompt Queue (High Priority LIFO)
    /// 2. Main Task Queue (FIFO)
    /// 
    /// The loop implements the full agentic cycle:
    /// - Dequeue Task
    /// - Context Assembly  
    /// - LLM Pre-Execution Refinement
    /// - Execute Tool
    /// - LLM Post-Execution Analysis
    /// - Update State and Loop
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("Starting Agentic Planning Coordinator");
        self.update_execution_state(ExecutionState::Idle).await;
        self.broadcast_status().await;

        loop {
            // Check for shutdown request
            if *self.shutdown_requested.read().await {
                tracing::info!("Shutdown requested, stopping coordinator");
                break;
            }

            // Main agentic loop with dual priority queue processing
            let mut cycle_tick = tokio::time::interval(tokio::time::Duration::from_millis(100));
            tokio::select! {
                biased;
                
                // Handle control messages (higher priority)
                message = self.message_receiver.recv() => {
                    if let Some(msg) = message {
                        if let Err(e) = self.handle_message(msg).await {
                            tracing::error!("Error handling message: {}", e);
                        }
                    } else {
                        break; // Channel closed
                    }
                }

                // Main agentic loop execution (lower priority)
                _ = cycle_tick.tick() => {
                    if let Err(e) = self.execute_agentic_cycle().await {
                        tracing::error!("Error in agentic cycle: {}", e);
                    }
                }
            }

            // Update metrics and broadcast status
            self.update_metrics().await;
            self.broadcast_status().await;
            
            // Small delay to prevent busy waiting
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        self.update_execution_state(ExecutionState::Shutdown).await;
        self.broadcast_status().await;
        tracing::info!("Agentic Planning Coordinator stopped");
        Ok(())
    }
    
    /// Execute the main agentic cycle - the heart of the coordinator
    async fn execute_agentic_cycle(&mut self) -> Result<()> {
        // Step 1: Check User Prompt Queue (High Priority LIFO)
        if let Some(user_prompt) = self.dequeue_user_prompt().await {
            return self.handle_user_prompt(user_prompt).await;
        }

        // Step 2: Check Main Task Queue (FIFO) 
        if let Some(task) = self.dequeue_main_task().await {
            return self.execute_task_with_full_agentic_loop(task).await;
        }
        
        // No work available - remain idle
        self.update_execution_state(ExecutionState::Idle).await;
        Ok(())
    }

    /// Handle control messages from external systems
    async fn handle_message(&mut self, message: PlanManagerMessage) -> Result<()> {
        match message {
            PlanManagerMessage::StartPlan(plan) => {
                self.start_plan(plan).await?;
            }
            PlanManagerMessage::PausePlan => {
                self.pause_plan().await?;
            }
            PlanManagerMessage::ResumePlan => {
                self.resume_plan().await?;
            }
            PlanManagerMessage::CancelPlan => {
                self.cancel_plan().await?;
            }
            PlanManagerMessage::UserRequest(prompt) => {
                self.enqueue_user_prompt(prompt).await;
            }
            PlanManagerMessage::ModifyPlan(plan) => {
                self.modify_plan(plan).await?;
            }
            PlanManagerMessage::GetStatus => {
                self.broadcast_status().await;
            }
            PlanManagerMessage::DecomposeTask(task_id) => {
                self.decompose_task(&task_id).await?;
            }
            PlanManagerMessage::Shutdown => {
                let mut shutdown = self.shutdown_requested.write().await;
                *shutdown = true;
            }
        }
        Ok(())
    }

    /// Start executing a new plan
    async fn start_plan(&self, mut plan: Plan) -> Result<()> {
        tracing::info!("Starting plan: {}", plan.description);
        
        // Validate plan first
        if plan.tasks.is_empty() {
            return Err(KaiError::planning("Cannot start plan with no tasks"));
        }
        
        if plan.tasks.len() > self.config.max_plan_size {
            return Err(KaiError::planning(format!(
                "Plan too large: {} tasks (max {})", 
                plan.tasks.len(), 
                self.config.max_plan_size
            )));
        }
        
        // Set plan status and create plan context
        plan.status = PlanStatus::Executing;
        let plan_id = plan.id.clone();
        
        // Update current plan
        {
            let mut current_plan = self.current_plan.write().await;
            *current_plan = Some(plan.clone());
        }
        
        // Create new plan context
        {
            let mut plan_context = self.current_plan_context.write().await;
            *plan_context = Some(PlanContext::new(plan_id.clone()));
        }
        
        // Clear and populate main task queue
        {
            let mut task_queue = self.main_task_queue.write().await;
            task_queue.clear();
            
            // Add ready tasks to queue (tasks with no dependencies)
            for task in &plan.tasks {
                if task.dependencies.is_empty() && task.status == TaskStatus::Pending {
                    task_queue.push_back(task.clone());
                }
            }
        }
        
        // Increment metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.plans_generated += 1;
        }
        
        tracing::info!("Plan started: {} ({} tasks)", plan.description, plan.tasks.len());
        Ok(())
    }

    /// Pause the current plan
    async fn pause_plan(&self) -> Result<()> {
        let mut current_plan = self.current_plan.write().await;
        if let Some(ref mut plan) = *current_plan {
            if plan.status == PlanStatus::Executing {
                plan.status = PlanStatus::Paused;
                tracing::info!("Plan paused: {}", plan.description);
            }
        }
        self.update_execution_state(ExecutionState::Paused).await;
        Ok(())
    }

    /// Resume the paused plan
    async fn resume_plan(&self) -> Result<()> {
        let mut current_plan = self.current_plan.write().await;
        if let Some(ref mut plan) = *current_plan {
            if plan.status == PlanStatus::Paused {
                plan.status = PlanStatus::Executing;
                tracing::info!("Plan resumed: {}", plan.description);
            }
        }
        self.update_execution_state(ExecutionState::Idle).await;
        Ok(())
    }

    /// Cancel the current plan
    async fn cancel_plan(&self) -> Result<()> {
        let mut current_plan = self.current_plan.write().await;
        if let Some(ref mut plan) = *current_plan {
            plan.status = PlanStatus::Cancelled;
            tracing::info!("Plan cancelled: {}", plan.description);
        }
        
        // Clear task queues
        {
            let mut task_queue = self.main_task_queue.write().await;
            task_queue.clear();
        }
        
        // Clear plan context
        {
            let mut plan_context = self.current_plan_context.write().await;
            *plan_context = None;
        }
        
        Ok(())
    }

    /// Handle a user prompt with priority-based processing
    async fn handle_user_prompt(&self, prompt: UserPrompt) -> Result<()> {
        tracing::info!("Handling user prompt: {} (priority: {:?})", prompt.content, prompt.priority);
        
        self.update_execution_state(ExecutionState::Planning).await;
        
        // Increment interruption counter
        {
            let mut metrics = self.metrics.write().await;
            metrics.user_interruptions += 1;
        }
        
        // Handle based on priority level
        match prompt.priority {
            PromptPriority::Emergency => {
                // Emergency: Clear everything and start fresh
                self.handle_emergency_prompt(prompt).await?
            }
            PromptPriority::Interrupt => {
                // Interrupt: Pause current plan and handle immediately
                self.handle_interrupt_prompt(prompt).await?
            }
            PromptPriority::Normal => {
                // Normal: Generate plan and queue or modify current plan
                self.handle_normal_prompt(prompt).await?
            }
        }
        
        Ok(())
    }
    
    /// Handle emergency prompt - clears everything and starts fresh
    async fn handle_emergency_prompt(&self, prompt: UserPrompt) -> Result<()> {
        tracing::warn!("Emergency prompt received, clearing all queues and current plan");
        
        // Clear all queues
        {
            let mut user_queue = self.user_prompt_queue.write().await;
            user_queue.clear();
            
            let mut task_queue = self.main_task_queue.write().await;
            task_queue.clear();
        }
        
        // Cancel current plan
        {
            let mut current_plan = self.current_plan.write().await;
            if let Some(ref mut plan) = *current_plan {
                plan.status = PlanStatus::Cancelled;
            }
        }
        
        // Generate and start new plan immediately
        let new_plan = self.generate_plan_from_prompt(&prompt).await?;
        self.start_plan(new_plan).await?;
        
        Ok(())
    }
    
    /// Handle interrupt prompt - pause current and handle immediately
    async fn handle_interrupt_prompt(&self, prompt: UserPrompt) -> Result<()> {
        tracing::info!("Interrupt prompt received, pausing current execution");
        
        // Pause current plan if executing
        self.pause_plan().await?;
        
        // Generate plan for interrupt
        let interrupt_plan = self.generate_plan_from_prompt(&prompt).await?;
        
        // For now, replace current plan (TODO: implement plan stacking)
        self.start_plan(interrupt_plan).await?;
        
        Ok(())
    }
    
    /// Handle normal prompt - smart plan modification or queuing
    async fn handle_normal_prompt(&self, prompt: UserPrompt) -> Result<()> {
        // Check if there's a current plan to modify
        let current_plan = self.current_plan.read().await.clone();
        
        if let Some(plan) = current_plan {
            if plan.status == PlanStatus::Executing {
                // Try to modify existing plan
                match self.generate_modified_plan(&plan, &prompt).await {
                    Ok(modified_plan) => {
                        self.modify_plan(modified_plan).await?;
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!("Failed to modify plan, creating new plan: {}", e);
                    }
                }
            }
        }
        
        // Fallback: create new plan
        let new_plan = self.generate_plan_from_prompt(&prompt).await?;
        self.start_plan(new_plan).await?;
        
        Ok(())
    }

    /// Execute a task using the full agentic loop from Section XV of the spec
    /// 
    /// This implements the complete cycle:
    /// 1. Dequeue Task ✓ (already done by caller)
    /// 2. Context Assembly
    /// 3. LLM Pre-Execution Refinement  
    /// 4. Execute Tool
    /// 5. LLM Post-Execution Analysis
    /// 6. Update State and Loop
    async fn execute_task_with_full_agentic_loop(&self, mut task: Task) -> Result<()> {
        let task_start_time = Instant::now();
        
        tracing::info!("Starting agentic loop for task: {} ({})", task.description, task.id);
        
        // Update task status to in progress in the plan
        self.update_task_status_in_plan(&task.id, TaskStatus::InProgress).await?;
        
        // STEP 2: Context Assembly
        self.update_execution_state(ExecutionState::ContextAssembly).await;
        let refinement_context = self.assemble_task_refinement_context(&task).await?;
        
        // Check if this is an abstract task that needs decomposition
        if self.config.auto_decompose_abstract_tasks && self.is_abstract_task(&task) {
            return self.decompose_and_queue_subtasks(task, &refinement_context).await;
        }
        
        // STEP 3: LLM Pre-Execution Refinement
        self.update_execution_state(ExecutionState::TaskRefinement).await;
        let concrete_instruction = self.refine_task_for_execution(&task, &refinement_context).await?;
        
        // STEP 4: Execute Tool
        self.update_execution_state(ExecutionState::TaskExecution).await;
        let raw_execution_result = self.execute_primitive_task(&task, &concrete_instruction).await?;
        
        // STEP 5: LLM Post-Execution Analysis
        self.update_execution_state(ExecutionState::ResultAnalysis).await;
        let analyzed_result = self.analyze_task_execution_result(&task, &raw_execution_result).await?;
        
        // STEP 6: Update State and Context
        self.update_execution_state(ExecutionState::StateUpdate).await;
        let execution_time_ms = task_start_time.elapsed().as_millis() as u64;
        
        // Create task result from analysis
        let task_result = TaskResult {
            success: analyzed_result.success,
            output: analyzed_result.extracted_data.clone(),
            error: analyzed_result.error.clone(),
            execution_time_ms,
            metadata: analyzed_result.metadata.clone(),
        };
        
        // Update plan with task result
        self.update_task_result_in_plan(&task.id, task_result.clone()).await?;
        
        // Update plan context with results
        if let Some(plan_context) = &mut *self.current_plan_context.write().await {
            plan_context.add_task_result(task.id.clone(), task.description.clone(), task_result);
            
            if let Some(extracted_data) = analyzed_result.extracted_data {
                plan_context.add_output(
                    task.id.clone(),
                    task.description.clone(),
                    extracted_data,
                    task.task_type.to_string(),
                );
            }
        }
        
        // Update global context if files were modified
        if let Some(modified_files) = analyzed_result.modified_files {
            let context_manager = self.context_manager.read().await;
            context_manager.update_global_context_for_files(&modified_files).await?;
            
            let mut metrics = self.metrics.write().await;
            metrics.context_updates += 1;
        }
        
        // Queue any dependent tasks that are now ready
        self.queue_newly_ready_tasks().await?;
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.tasks_processed += 1;
            
            // Update average task time
            let new_avg = (metrics.average_task_time_ms * (metrics.tasks_processed - 1) as f64 + execution_time_ms as f64) 
                / metrics.tasks_processed as f64;
            metrics.average_task_time_ms = new_avg;
        }
        
        tracing::info!(
            "Completed agentic loop for task: {} in {}ms (success: {})", 
            task.description, 
            execution_time_ms,
            analyzed_result.success
        );
        
        Ok(())
    }

    /// Execute a primitive task using the task executor
    async fn execute_primitive_task(&self, task: &Task, concrete_instruction: &str) -> Result<TaskExecutionResult> {
        tracing::debug!("Executing primitive task: {} with instruction: {}", task.id, concrete_instruction);
        
        let plan_context = match &*self.current_plan_context.read().await {
            Some(context) => context.clone(),
            None => {
                tracing::warn!("No plan context available, creating empty context");
                PlanContext::new("unknown".to_string())
            }
        };
        
        // Use task executor with timeout
        let execution_future = async {
            // Convert PlanContext to string for context parameter
            let context_str = plan_context.get_summary();
            self.task_executor.write().await.execute_task(task, concrete_instruction, &context_str).await
        };
        
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(self.config.task_timeout_ms),
            execution_future
        ).await;
        
        match result {
            Ok(task_result) => {
                // task_result is Result<TaskExecutionResult>, so we need to handle it
                match task_result {
                    Ok(execution_result) => Ok(execution_result),
                    Err(e) => {
                        tracing::error!("Task execution failed: {}", e);
                        Ok(TaskExecutionResult {
                            success: false,
                            stdout: None,
                            stderr: Some(format!("Execution failed: {}", e)),
                            exit_code: Some(-1),
                            output: None,
                            error: Some(e.to_string()),
                            execution_time_ms: 0,
                            metadata: HashMap::new(),
                        })
                    }
                }
            }
            Err(_) => {
                tracing::error!("Task execution timed out after {}ms", self.config.task_timeout_ms);
                Ok(TaskExecutionResult {
                    success: false,
                    stdout: None,
                    stderr: Some("Task execution timed out".to_string()),
                    exit_code: Some(-1),
                    output: None,
                    error: Some(format!("Task timed out after {}ms", self.config.task_timeout_ms)),
                    execution_time_ms: self.config.task_timeout_ms,
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// Get the current plan (for UI display)
    pub async fn get_current_plan(&self) -> Option<Plan> {
        self.current_plan.read().await.clone()
    }
    
    /// Get current coordinator status
    pub async fn get_status(&self) -> CoordinatorStatus {
        let current_plan = self.get_current_plan_status_info().await;
        let user_queue_size = self.user_prompt_queue.read().await.len();
        let task_queue_size = self.main_task_queue.read().await.len();
        let execution_state = self.execution_state.read().await.clone();
        let current_task = self.get_current_task_info().await;
        let metrics = self.get_current_metrics().await;
        
        CoordinatorStatus {
            current_plan,
            user_prompt_queue_size: user_queue_size,
            main_task_queue_size: task_queue_size,
            execution_state,
            current_task,
            performance_metrics: metrics,
        }
    }
    
    /// Generate a new plan from a user prompt
    pub async fn generate_plan(&self, prompt: &str) -> Result<Plan> {
        let context_manager = self.context_manager.read().await;
        let context = context_manager.get_global_context_summary().await?;
        
        let plan = self.llm_provider
            .generate_plan(prompt, &context, &self.current_model)
            .await
            .map_err(KaiError::from)?;
            
        // Increment LLM call counter
        {
            let mut metrics = self.metrics.write().await;
            metrics.llm_calls_made += 1;
        }
        
        Ok(plan)
    }
    
    // ===== PRIVATE HELPER METHODS =====
    
    /// Dequeue next user prompt (LIFO for high priority)
    async fn dequeue_user_prompt(&self) -> Option<UserPrompt> {
        let mut queue = self.user_prompt_queue.write().await;
        queue.pop_front()
    }
    
    /// Enqueue user prompt with priority handling
    async fn enqueue_user_prompt(&self, prompt: UserPrompt) {
        let mut queue = self.user_prompt_queue.write().await;
        
        // Check queue size limit
        if queue.len() >= self.config.max_user_prompt_queue {
            tracing::warn!("User prompt queue full, removing oldest normal priority prompt");
            // Remove the oldest normal priority prompt
            if let Some(pos) = queue.iter().rposition(|p| p.priority == PromptPriority::Normal) {
                queue.remove(pos);
            } else {
                // If no normal priority prompts, remove oldest
                queue.pop_back();
            }
        }
        
        // Insert based on priority (LIFO for same priority level)
        match prompt.priority {
            PromptPriority::Emergency => {
                queue.push_front(prompt);
            }
            PromptPriority::Interrupt => {
                // Insert after any emergency prompts but before normal prompts
                let mut insert_pos = 0;
                for (i, existing) in queue.iter().enumerate() {
                    if existing.priority < PromptPriority::Interrupt {
                        insert_pos = i;
                        break;
                    }
                    insert_pos = i + 1;
                }
                queue.insert(insert_pos, prompt);
            }
            PromptPriority::Normal => {
                queue.push_back(prompt);
            }
        }
    }
    
    /// Dequeue next main task (FIFO)
    async fn dequeue_main_task(&self) -> Option<Task> {
        let mut queue = self.main_task_queue.write().await;
        queue.pop_front()
    }
    
    /// Update execution state and notify
    async fn update_execution_state(&self, new_state: ExecutionState) {
        let mut state = self.execution_state.write().await;
        if *state != new_state {
            tracing::debug!("Execution state: {:?} -> {:?}", *state, new_state);
            *state = new_state;
        }
    }
    
    /// Broadcast current status to subscribers
    async fn broadcast_status(&self) {
        let status = self.get_status().await;
        if let Err(e) = self.status_broadcaster.send(status) {
            tracing::debug!("No status subscribers: {}", e);
        }
    }
    
    /// Update performance metrics
    async fn update_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.uptime_seconds = self.start_time.elapsed().as_secs();
    }
    
    /// Get current metrics snapshot
    async fn get_current_metrics(&self) -> PerformanceMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Get current plan status info for UI
    async fn get_current_plan_status_info(&self) -> Option<PlanStatusInfo> {
        let plan = self.current_plan.read().await;
        plan.as_ref().map(|p| {
            let completed_tasks = p.tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();
            let failed_tasks = p.tasks.iter().filter(|t| t.status == TaskStatus::Failed).count();
            
            PlanStatusInfo {
                id: p.id.clone(),
                description: p.description.clone(),
                status: p.status.clone(),
                total_tasks: p.tasks.len(),
                completed_tasks,
                failed_tasks,
                created_at: p.created_at,
                updated_at: p.updated_at,
            }
        })
    }
    
    /// Get current task info for UI (stub for now)
    async fn get_current_task_info(&self) -> Option<TaskStatusInfo> {
        // TODO: Track currently executing task
        None
    }

    /// Generate a plan from a user prompt with full context
    async fn generate_plan_from_prompt(&self, prompt: &UserPrompt) -> Result<Plan> {
        let context_manager = self.context_manager.read().await;
        let global_context = context_manager.get_global_context_summary().await?;
        
        let full_context = if let Some(hint) = &prompt.context_hint {
            format!("{}\n\nAdditional Context: {}", global_context, hint)
        } else {
            global_context
        };
        
        let plan = self.llm_provider
            .generate_plan(&prompt.content, &full_context, &self.current_model)
            .await?;
            
        {
            let mut metrics = self.metrics.write().await;
            metrics.llm_calls_made += 1;
        }
        
        Ok(plan)
    }
    
    /// Generate a modified plan based on current plan and new prompt
    async fn generate_modified_plan(&self, current_plan: &Plan, prompt: &UserPrompt) -> Result<Plan> {
        let context_manager = self.context_manager.read().await;
        let global_context = context_manager.get_global_context_summary().await?;
        
        let plan_summary = format!(
            "Current Plan: {}\nCompleted Tasks: {}\nRemaining Tasks: {}\nNew Request: {}",
            current_plan.description,
            current_plan.tasks.iter().filter(|t| t.status == TaskStatus::Completed).count(),
            current_plan.tasks.iter().filter(|t| t.status != TaskStatus::Completed).count(),
            prompt.content
        );
        
        let modification_prompt = format!(
            "Modify the existing plan to incorporate the new user request. \
            Preserve completed work where possible.\n\n{}",
            plan_summary
        );
        
        let modified_plan = self.llm_provider
            .generate_plan(&modification_prompt, &global_context, &self.current_model)
            .await?;
            
        {
            let mut metrics = self.metrics.write().await;
            metrics.llm_calls_made += 1;
        }
        
        Ok(modified_plan)
    }
    
    /// Assemble comprehensive context for task refinement
    async fn assemble_task_refinement_context(&self, task: &Task) -> Result<TaskRefinementContext> {
        // Get global context
        let context_manager = self.context_manager.read().await;
        let global_context = context_manager.get_global_context_summary().await?;
        
        // Get plan context
        let plan_context = match &*self.current_plan_context.read().await {
            Some(context) => context.get_summary(),
            None => "No plan context available".to_string(),
        };
        
        // Get dependency outputs
        let mut dependency_outputs = HashMap::new();
        if let Some(context) = &*self.current_plan_context.read().await {
            for dep_id in &task.dependencies {
                if let Some(result) = context.get_task_result(dep_id) {
                    if let Some(output) = &result.output {
                        dependency_outputs.insert(dep_id.clone(), output.clone());
                    }
                }
            }
        }
        
        // Get plan description
        let plan_description = match &*self.current_plan.read().await {
            Some(plan) => plan.description.clone(),
            None => "No active plan".to_string(),
        };
        
        Ok(TaskRefinementContext {
            global_context,
            plan_context,
            dependency_outputs,
            plan_description,
        })
    }
    
    /// Use LLM to refine task into concrete execution instruction
    async fn refine_task_for_execution(&self, task: &Task, context: &TaskRefinementContext) -> Result<String> {
        let instruction = self.llm_provider
            .refine_task_for_execution(task, context, &self.current_model)
            .await?;
            
        {
            let mut metrics = self.metrics.write().await;
            metrics.llm_calls_made += 1;
        }
        
        Ok(instruction)
    }
    
    /// Use LLM to analyze task execution results
    async fn analyze_task_execution_result(&self, task: &Task, result: &TaskExecutionResult) -> Result<TaskAnalysis> {
        let analysis = self.llm_provider
            .analyze_task_result(task, result, &task.description, &self.current_model)
            .await?;
            
        {
            let mut metrics = self.metrics.write().await;
            metrics.llm_calls_made += 1;
        }
        
        Ok(analysis)
    }
    
    /// Check if a task is abstract and needs decomposition
    fn is_abstract_task(&self, task: &Task) -> bool {
        // Simple heuristics for detecting abstract tasks
        // TODO: Make this more sophisticated with LLM analysis
        let abstract_keywords = [
            "refactor", "improve", "enhance", "optimize", "redesign",
            "implement", "create", "build", "develop", "design"
        ];
        
        let description_lower = task.description.to_lowercase();
        abstract_keywords.iter().any(|keyword| description_lower.contains(keyword))
            && task.parameters.is_empty() // Abstract tasks usually lack specific parameters
    }
    
    /// Decompose an abstract task into concrete subtasks
    async fn decompose_and_queue_subtasks(&self, task: Task, context: &TaskRefinementContext) -> Result<()> {
        tracing::info!("Decomposing abstract task: {}", task.description);
        
        // Use LLM to decompose the task
        let decomposition_prompt = format!(
            "Decompose this high-level task into concrete, executable subtasks:\n\n\
            Task: {}\nTask Type: {:?}\nParameters: {}\n\n\
            Context: {}\n\n\
            Return a JSON array of subtasks with fields: id, description, task_type, parameters, dependencies",
            task.description,
            task.task_type,
            serde_json::to_string_pretty(&task.parameters).unwrap_or_default(),
            context.global_context
        );
        
        let subtasks_json = self.llm_provider
            .generate_content(&decomposition_prompt, &context.plan_context, &self.current_model, None)
            .await?;
        
        // Parse subtasks (simplified - in production, use structured output)
        // TODO: Implement proper JSON parsing with error handling
        tracing::info!("Task decomposition result: {}", subtasks_json);
        
        // For now, mark the original task as completed and log the decomposition
        self.update_task_status_in_plan(&task.id, TaskStatus::Completed).await?;
        
        {
            let mut metrics = self.metrics.write().await;
            metrics.decompositions_performed += 1;
            metrics.llm_calls_made += 1;
        }
        
        // TODO: Parse subtasks and add them to the plan and queue
        tracing::warn!("Task decomposition parsing not fully implemented yet");
        
        Ok(())
    }
    
    /// Update task status in the current plan
    async fn update_task_status_in_plan(&self, task_id: &str, new_status: TaskStatus) -> Result<()> {
        let mut plan = self.current_plan.write().await;
        if let Some(ref mut p) = *plan {
            p.update_task_status(task_id, new_status)?;
        }
        Ok(())
    }
    
    /// Update task result in the current plan
    async fn update_task_result_in_plan(&self, task_id: &str, result: TaskResult) -> Result<()> {
        let mut plan = self.current_plan.write().await;
        if let Some(ref mut p) = *plan {
            p.set_task_result(task_id, result)?;
        }
        Ok(())
    }
    
    /// Queue newly ready tasks after a task completes
    async fn queue_newly_ready_tasks(&self) -> Result<()> {
        let current_plan = self.current_plan.read().await.clone();
        
        if let Some(plan) = current_plan {
            let ready_tasks = plan.get_ready_tasks();
            let mut task_queue = self.main_task_queue.write().await;
            
            for task in ready_tasks {
                // Only queue tasks that aren't already queued or executing
                if task.status == TaskStatus::Pending {
                    // Check if task is already in queue
                    let already_queued = task_queue.iter().any(|queued_task| queued_task.id == task.id);
                    
                    if !already_queued {
                        task_queue.push_back(task.clone());
                        tracing::debug!("Queued newly ready task: {}", task.description);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Decompose a specific task by ID (for external requests)
    async fn decompose_task(&self, task_id: &str) -> Result<()> {
        let current_plan = self.current_plan.read().await.clone();
        
        if let Some(plan) = current_plan {
            if let Some(task) = plan.tasks.iter().find(|t| t.id == task_id) {
                let context = self.assemble_task_refinement_context(task).await?;
                self.decompose_and_queue_subtasks(task.clone(), &context).await?;
            } else {
                return Err(KaiError::planning(format!("Task not found: {}", task_id)));
            }
        } else {
            return Err(KaiError::planning("No active plan"));
        }
        
        Ok(())
    }
    
    /// Modify the current plan (preserving completed work)
    async fn modify_plan(&self, new_plan: Plan) -> Result<()> {
        tracing::info!("Modifying current plan: {}", new_plan.description);
        
        // TODO: Implement intelligent plan merging that preserves completed tasks
        // For now, just replace the plan
        
        let mut current_plan = self.current_plan.write().await;
        *current_plan = Some(new_plan.clone());
        
        // Clear and repopulate task queue with new tasks
        let mut task_queue = self.main_task_queue.write().await;
        task_queue.clear();
        
        for task in &new_plan.tasks {
            if task.status == TaskStatus::Pending {
                task_queue.push_back(task.clone());
            }
        }
        
        Ok(())
    }
}

// Legacy compatibility - need separate impl to avoid name collision  
impl AgenticPlanningCoordinator {
    /// Create a new plan manager (legacy compatibility)
    pub fn new_legacy(
        task_executor: TaskExecutor,
        context_manager: ContextManager,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
    ) -> PlanManager {
        Self::new(
            task_executor,
            context_manager, 
            llm_provider,
            model,
            None
        )
    }
}

impl PlanManager {
    /// Get a sender for communicating with the plan manager (legacy)
    pub fn get_sender(&self) -> mpsc::UnboundedSender<PlanManagerMessage> {
        self.get_message_sender()
    }
}