//! Plan manager for coordinating plan execution

use super::{Plan, PlanStatus, Task, TaskStatus};
use crate::{
    context::{ContextManager, PlanContext},
    execution::TaskExecutor,
    llm::LlmProvider,
    utils::errors::KaiError,
    Result,
};
use std::collections::VecDeque;
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;

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
    UserRequest(String),
    /// Modify the current plan
    ModifyPlan(Plan),
}

/// Manages plan execution with support for interruptions and modifications
pub struct PlanManager {
    /// The currently active plan
    current_plan: Arc<RwLock<Option<Plan>>>,
    /// Queue for high-priority user requests
    user_request_queue: Arc<RwLock<VecDeque<String>>>,
    /// Task executor for running individual tasks
    task_executor: Arc<TaskExecutor>,
    /// Context manager for maintaining project state
    context_manager: Arc<ContextManager>,
    /// Channel for receiving control messages
    message_receiver: mpsc::UnboundedReceiver<PlanManagerMessage>,
    /// Channel sender for external communication
    message_sender: mpsc::UnboundedSender<PlanManagerMessage>,
    /// LLM provider for plan generation and modification
    llm_provider: Arc<dyn LlmProvider>,
    /// Current model to use for LLM operations
    current_model: String,
}

impl PlanManager {
    /// Create a new plan manager
    pub fn new(
        task_executor: TaskExecutor,
        context_manager: ContextManager,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        Self {
            current_plan: Arc::new(RwLock::new(None)),
            user_request_queue: Arc::new(RwLock::new(VecDeque::new())),
            task_executor: Arc::new(task_executor),
            context_manager: Arc::new(context_manager),
            message_receiver: receiver,
            message_sender: sender,
            llm_provider,
            current_model: model,
        }
    }

    /// Get a sender for communicating with the plan manager
    pub fn get_sender(&self) -> mpsc::UnboundedSender<PlanManagerMessage> {
        self.message_sender.clone()
    }

    /// Start the main execution loop
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("Starting plan manager");

        loop {
            tokio::select! {
                // Handle control messages
                message = self.message_receiver.recv() => {
                    if let Some(msg) = message {
                        self.handle_message(msg).await?;
                    } else {
                        break; // Channel closed
                    }
                }

                // Process next task if plan is active
                _ = self.process_next_task() => {
                    // Task processing completed or no tasks ready
                }
            }

            // Small delay to prevent busy waiting
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        tracing::info!("Plan manager stopped");
        Ok(())
    }

    /// Handle a control message
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
            PlanManagerMessage::UserRequest(request) => {
                self.handle_user_request(request).await?;
            }
            PlanManagerMessage::ModifyPlan(plan) => {
                self.modify_plan(plan).await?;
            }
        }
        Ok(())
    }

    /// Start executing a new plan
    async fn start_plan(&self, mut plan: Plan) -> Result<()> {
        tracing::info!("Starting plan: {}", plan.description);
        
        plan.status = PlanStatus::Executing;
        let mut current_plan = self.current_plan.write().await;
        *current_plan = Some(plan);
        
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
        Ok(())
    }

    /// Cancel the current plan
    async fn cancel_plan(&self) -> Result<()> {
        let mut current_plan = self.current_plan.write().await;
        if let Some(ref mut plan) = *current_plan {
            plan.status = PlanStatus::Cancelled;
            tracing::info!("Plan cancelled: {}", plan.description);
        }
        Ok(())
    }

    /// Handle a user request by potentially modifying the current plan
    async fn handle_user_request(&self, request: String) -> Result<()> {
        tracing::info!("Handling user request: {}", request);
        
        // Add to high-priority queue
        {
            let mut queue = self.user_request_queue.write().await;
            queue.push_back(request.clone());
        }

        // Check if we need to modify the current plan
        let current_plan = self.current_plan.read().await;
        if let Some(plan) = current_plan.as_ref() {
            if plan.status == PlanStatus::Executing {
                // Generate a modified plan based on the current plan and new request
                let context = self.context_manager.get_global_context_summary().await?;
                let plan_context = format!(
                    "Current plan: {}\nTasks: {:?}\nNew request: {}",
                    plan.description,
                    plan.tasks.iter().map(|t| &t.description).collect::<Vec<_>>(),
                    request
                );

                match self.llm_provider.generate_plan(&request, &plan_context, &self.current_model).await {
                    Ok(new_plan) => {
                        // Send message to modify the plan
                        let _ = self.message_sender.send(PlanManagerMessage::ModifyPlan(new_plan));
                    }
                    Err(e) => {
                        tracing::error!("Failed to generate modified plan: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Modify the current plan
    async fn modify_plan(&self, new_plan: Plan) -> Result<()> {
        tracing::info!("Modifying current plan");
        
        let mut current_plan = self.current_plan.write().await;
        if let Some(ref mut plan) = *current_plan {
            // Preserve the execution state of completed tasks
            let mut modified_plan = new_plan;
            modified_plan.status = PlanStatus::Executing;
            
            // TODO: Implement smarter plan merging that preserves completed work
            *current_plan = Some(modified_plan);
        }

        Ok(())
    }

    /// Process the next ready task in the current plan
    async fn process_next_task(&self) -> Result<()> {
        let current_plan_clone = {
            let plan_guard = self.current_plan.read().await;
            plan_guard.clone()
        };

        if let Some(plan) = current_plan_clone {
            if plan.status != PlanStatus::Executing {
                return Ok(());
            }

            // Check for high-priority user requests first
            {
                let mut queue = self.user_request_queue.write().await;
                if !queue.is_empty() {
                    // User request takes priority - it will be handled in the main loop
                    return Ok(());
                }
            }

            // Find ready tasks
            let ready_tasks = plan.get_ready_tasks();
            if ready_tasks.is_empty() {
                return Ok(());
            }

            // Execute the first ready task
            let task = ready_tasks[0];
            if task.status == TaskStatus::Pending {
                self.execute_task(task.clone()).await?;
            }
        }

        Ok(())
    }

    /// Execute a single task
    async fn execute_task(&self, mut task: Task) -> Result<()> {
        tracing::info!("Executing task: {}", task.description);
        
        // Update task status to in progress
        {
            let mut plan_guard = self.current_plan.write().await;
            if let Some(ref mut plan) = *plan_guard {
                plan.update_task_status(&task.id, TaskStatus::InProgress)?;
            }
        }

        // Create plan context for this task
        let plan_context = PlanContext::new();
        
        // Execute the task
        let result = self.task_executor.execute_task(&task, &plan_context).await;

        // Update the plan with the result
        {
            let mut plan_guard = self.current_plan.write().await;
            if let Some(ref mut plan) = *plan_guard {
                plan.set_task_result(&task.id, result)?;
            }
        }

        Ok(())
    }

    /// Get the current plan (for UI display)
    pub async fn get_current_plan(&self) -> Option<Plan> {
        self.current_plan.read().await.clone()
    }

    /// Generate a new plan from a user prompt
    pub async fn generate_plan(&self, prompt: &str) -> Result<Plan> {
        let context = self.context_manager.get_global_context_summary().await?;
        self.llm_provider
            .generate_plan(prompt, &context, &self.current_model)
            .await
            .map_err(KaiError::from)
    }
}