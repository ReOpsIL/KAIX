# KAI-X Component Relationships

This document provides detailed views of component relationships within each module and their internal architectures.

## Execution Engine Internal Architecture

```mermaid
classDiagram
    class ExecutionEngine {
        +queue: TaskQueue
        +executor: TaskExecutor
        +state: ExecutionState
        +config: ExecutionConfig
        +new() ExecutionEngine
        +start() Result~()~
        +stop() Result~()~
        +pause() Result~()~
        +resume() Result~()~
        +execute_task() Result~TaskExecutionResult~
    }
    
    class TaskExecutor {
        +config: ExecutionConfig
        +new() TaskExecutor
        +execute() Result~TaskExecutionResult~
        -execute_read_file() Result~TaskExecutionResult~
        -execute_write_file() Result~TaskExecutionResult~
        -execute_command() Result~TaskExecutionResult~
        -execute_analyze_code() Result~TaskExecutionResult~
    }
    
    class TaskQueue {
        +pending_tasks: VecDeque~Task~
        +completed_tasks: Vec~TaskExecutionResult~
        +add_task() Result~()~
        +get_next_task() Option~Task~
        +mark_completed() Result~()~
    }
    
    class ExecutionState {
        <<enumeration>>
        Idle
        Planning
        Executing
        Paused
        Cancelled
    }
    
    class ExecutionConfig {
        +max_retries: u32
        +timeout_seconds: u64
        +parallel_execution: bool
        +default() ExecutionConfig
    }
    
    class TaskExecutionResult {
        +success: bool
        +output: String
        +error_message: Option~String~
        +execution_time_ms: u64
    }
    
    ExecutionEngine --> TaskQueue : manages
    ExecutionEngine --> TaskExecutor : uses
    ExecutionEngine --> ExecutionState : maintains
    ExecutionEngine --> ExecutionConfig : configured by
    TaskExecutor --> ExecutionConfig : configured by
    TaskExecutor --> TaskExecutionResult : produces
    TaskQueue --> TaskExecutionResult : stores
```

## LLM Provider Architecture

```mermaid
classDiagram
    class LlmProvider {
        <<interface>>
        +list_models() Result~Vec~ModelInfo~~
        +generate() Result~LlmResponse~
        +generate_plan() Result~Plan~
        +refine_instruction() Result~String~
        +validate_model() Result~ModelInfo~
    }
    
    class LlmProviderFactory {
        +create_provider() Result~Box~LlmProvider~~
    }
    
    class OpenRouterProvider {
        +api_key: String
        +base_url: String
        +model: String
        +client: reqwest::Client
        +new() Self
        +execute_with_retry() Result~T~
        -parse_error_response() LlmError
    }
    
    class GeminiProvider {
        +api_key: String
        +model: String
        +client: reqwest::Client
        +new() Self
        -convert_messages() Result~GeminiContent~
        -convert_tools() Result~Vec~GeminiTool~~
    }
    
    class StreamingLlmProvider {
        <<interface>>
        +generate_stream() Result~LlmStream~
    }
    
    class LlmResponse {
        +content: String
        +tool_calls: Option~Vec~ToolCall~~
        +usage: Option~TokenUsage~
        +model: String
    }
    
    class LlmError {
        <<enumeration>>
        Network
        Authentication
        RateLimit
        InvalidModel
        RequestFailed
        InvalidResponse
        Unknown
    }
    
    LlmProvider <|.. OpenRouterProvider : implements
    LlmProvider <|.. GeminiProvider : implements
    StreamingLlmProvider <|.. OpenRouterProvider : implements
    StreamingLlmProvider <|.. GeminiProvider : implements
    LlmProviderFactory --> LlmProvider : creates
    LlmProvider --> LlmResponse : returns
    LlmProvider --> LlmError : may throw
    OpenRouterProvider --> LlmError : handles
    GeminiProvider --> LlmError : handles
```

## Enhanced Context Management Architecture

```mermaid
classDiagram
    class ContextManager {
        +global_context: Arc~RwLock~GlobalContext~~
        +plan_contexts: HashMap~String, Arc~RwLock~PlanContext~~~
        +working_directory: PathBuf
        +llm_provider: Arc~LlmProvider~
        +model: String
        +config: ContextConfig
        +health_config: ContextHealthConfig
        +last_health_check: Option~ContextHealthReport~
        +new() Self
        +with_health_config() Self
        +with_memory_config() Self
        +create_plan_context() Arc~RwLock~PlanContext~~
        +refresh_global_context() Result~()~
        +health_check() Result~ContextHealthReport~
        +maintenance() Result~MaintenanceReport~
        +validate_consistency() Result~ValidationReport~
    }
    
    class GlobalContext {
        +working_directory: PathBuf
        +project_files: HashMap~String, ContextEntry~
        +context_metadata: HashMap~PathBuf, FileMetadata~
        +project_summary: Option~String~
        +config: ContextConfig
        +memory_config: ContextMemoryConfig
        +stats: GlobalContextStats
        +file_access_info: HashMap~PathBuf, FileAccessInfo~
        +cached_summaries: HashMap~PathBuf, CachedFileSummary~
        +refresh() Result~()~
        +regenerate() Result~()~
        +update_file_context() Result~()~
        +enforce_memory_limits() Result~()~
        +get_file_context() Result~String~
        +check_modifications_detailed() Result~ModificationCheckResult~
        +update_modified_files() Result~IncrementalUpdateResult~
        +get_memory_stats() ContextMemoryStats
    }
    
    class PlanContext {
        +plan_id: String
        +task_results: HashMap~String, TaskResult~
        +variables: HashMap~String, serde_json::Value~
        +outputs: Vec~PlanOutput~
        +dependency_graph: HashMap~String, Vec~String~~
        +created_at: DateTime~Utc~
        +last_updated: DateTime~Utc~
        +new() Self
        +with_dependency_graph() Self
        +add_task_result() Result~()~
        +set_variable() Result~()~
        +get_variable_as() Result~Option~T~~
        +add_output() String
        +get_dependency_outputs() Result~HashMap~String, Value~~
        +get_memory_stats() PlanContextMemoryStats
        +to_json() Result~String~
        +from_json() Result~Self~
    }
    
    class ContextHealthConfig {
        +check_interval_seconds: u64
        +max_memory_mb: usize
        +max_files: usize
        +auto_cleanup: bool
        +validate_integrity: bool
    }
    
    class ContextHealthReport {
        +overall_health: OverallHealth
        +warnings: Vec~ContextWarning~
        +memory_usage_mb: f64
        +file_count: usize
        +last_check: DateTime~Utc~
    }
    
    class ContextMemoryStats {
        +total_memory_usage: usize
        +file_summaries_size: usize
        +metadata_size: usize
        +cached_content_size: usize
        +file_count: usize
    }
    
    class FileAccessInfo {
        +last_accessed: DateTime~Utc~
        +access_count: u64
        +last_modified: DateTime~Utc~
        +file_size: u64
        +priority_score() f64
    }
    
    ContextManager --> GlobalContext : manages
    ContextManager --> PlanContext : manages
    ContextManager --> ContextHealthConfig : uses
    ContextManager --> ContextHealthReport : generates
    GlobalContext --> ContextMemoryStats : provides
    GlobalContext --> FileAccessInfo : tracks
    PlanContext --> PlanContextMemoryStats : provides
```

## UI Component Architecture

```mermaid
classDiagram
    class UiManager {
        +terminal: Terminal
        +chat_component: ChatComponent
        +plan_component: PlanComponent
        +status_component: StatusComponent
        +progress_component: ProgressComponent
        +input_service: InputBufferService
        +history_service: HistoryService
        +completion_service: CompletionService
        +new() Self
        +initialize() Result~()~
        +run() Result~()~
        +handle_event() Result~()~
    }
    
    class ChatComponent {
        +messages: Vec~ChatMessage~
        +new() Self
        +add_message() Result~()~
        +render() Result~()~
        +clear() Result~()~
        +get_messages() Vec~ChatMessage~
    }
    
    class PlanComponent {
        +current_plan: Option~Plan~
        +new() Self
        +set_plan() Result~()~
        +render() Result~()~
        +render_plan() Result~()~
        +render_task_list() Result~()~
    }
    
    class StatusComponent {
        +current_status: ApplicationStatus
        +new() Self
        +update_status() Result~()~
        +render() Result~()~
        +get_execution_state_style() Style
    }
    
    class ProgressComponent {
        +current_task: Option~String~
        +progress: f64
        +new() Self
        +set_task() Result~()~
        +set_progress() Result~()~
        +render() Result~()~
    }
    
    class InputBufferService {
        +lines: Vec~String~
        +cursor_line: usize
        +cursor_col: usize
        +new() Self
        +insert_char() Result~()~
        +insert_string() Result~()~
        +handle_newline() Result~()~
        +delete_backward() Result~()~
        +move_cursor_left() Result~()~
        +move_cursor_right() Result~()~
        +get_content() String
        +apply_completion() Result~()~
    }
    
    class HistoryService {
        +entries: VecDeque~String~
        +current_index: Option~usize~
        +max_entries: usize
        +new() Self
        +add_entry() Result~()~
        +navigate_up() Option~String~
        +navigate_down() Option~String~
        +search() Vec~String~
        +load_from_file() Result~()~
        +save_to_file() Result~()~
    }
    
    class CompletionService {
        +suggestions: Option~Vec~String~~
        +active_index: Option~usize~
        +matcher: SkimMatcherV2
        +new() Self
        +update_suggestions() Result~()~
        +update_slash_completions() Result~()~
        +update_file_completions() Result~()~
        +get_suggestions() Option~Vec~String~~
        +next_suggestion() Result~()~
        +previous_suggestion() Result~()~
    }
    
    class ChatMessage {
        +role: MessageRole
        +content: String
        +timestamp: DateTime
    }
    
    class MessageRole {
        <<enumeration>>
        User
        Assistant
        System
    }
    
    class ApplicationStatus {
        +execution_state: String
        +current_task: Option~String~
        +tasks_completed: usize
        +tasks_total: usize
    }
    
    UiManager --> ChatComponent : manages
    UiManager --> PlanComponent : manages
    UiManager --> StatusComponent : manages
    UiManager --> ProgressComponent : manages
    UiManager --> InputBufferService : uses
    UiManager --> HistoryService : uses
    UiManager --> CompletionService : uses
    ChatComponent --> ChatMessage : contains
    ChatMessage --> MessageRole : has
    StatusComponent --> ApplicationStatus : displays
    CompletionService --> InputBufferService : integrates with
    HistoryService --> InputBufferService : provides history to
```

## Agentic Planning System Architecture

```mermaid
classDiagram
    class AgenticPlanningCoordinator {
        +task_executor: TaskExecutor
        +context_manager: ContextManager
        +llm_provider: Arc~LlmProvider~
        +model: String
        +config: CoordinatorConfig
        +message_sender: UnboundedSender~PlanManagerMessage~
        +status_receiver: Receiver~CoordinatorStatus~
        +new() Self
        +start() Result~()~
        +submit_user_prompt() Result~String~
        +execute_agentic_cycle() Result~()~
        +handle_message() Result~()~
        +start_plan() Result~()~
        +pause_plan() Result~()~
        +resume_plan() Result~()~
        +cancel_plan() Result~()~
        +execute_task_with_full_agentic_loop() Result~()~
    }
    
    class PlanManagerMessage {
        <<enumeration>>
        StartPlan(Plan)
        PausePlan
        ResumePlan
        CancelPlan
        UserRequest(UserPrompt)
        ModifyPlan(Plan)
        GetStatus
        DecomposeTask(String)
        Shutdown
    }
    
    class UserPrompt {
        +content: String
        +priority: PromptPriority
        +timestamp: DateTime
        +user_id: Option~String~
        +session_id: String
    }
    
    class PromptPriority {
        <<enumeration>>
        Normal
        Interrupt
        Emergency
    }
    
    class CoordinatorStatus {
        +execution_state: ExecutionState
        +current_plan: Option~PlanStatusInfo~
        +current_task: Option~TaskStatusInfo~
        +metrics: PerformanceMetrics
    }
    
    class ExecutionState {
        <<enumeration>>
        Idle
        Planning
        ContextAssembly
        TaskRefinement
        TaskExecution
        ResultAnalysis
        StateUpdate
        Paused
        Cancelled
        Shutdown
    }
    
    class PerformanceMetrics {
        +tasks_completed: u64
        +tasks_failed: u64
        +average_task_duration: Duration
        +total_execution_time: Duration
        +memory_usage: usize
    }
    
    class TaskRefinementContext {
        +global_context: String
        +plan_context: String
        +execution_history: String
        +available_tools: Vec~String~
    }
    
    AgenticPlanningCoordinator --> PlanManagerMessage : processes
    AgenticPlanningCoordinator --> UserPrompt : handles
    AgenticPlanningCoordinator --> CoordinatorStatus : reports
    AgenticPlanningCoordinator --> ExecutionState : manages
    AgenticPlanningCoordinator --> PerformanceMetrics : tracks
    AgenticPlanningCoordinator --> TaskRefinementContext : creates
    UserPrompt --> PromptPriority : has
    CoordinatorStatus --> ExecutionState : contains
    CoordinatorStatus --> PerformanceMetrics : contains
```

## Configuration System Architecture

```mermaid
classDiagram
    class ConfigManager {
        +config_dir: PathBuf
        +config_data: ConfigData
        +new() Result~Self~
        +load() Result~()~
        +save() Result~()~
        +get() T
        +set() Result~()~
        +validate() Result~()~
    }
    
    class ConfigData {
        +llm: LlmConfig
        +execution: ExecutionConfig
        +ui: UiConfig
        +context: ContextConfig
        +default() Self
    }
    
    class LlmConfig {
        +default_provider: String
        +providers: HashMap~String, ProviderConfig~
        +timeout_seconds: u64
        +max_tokens: u32
    }
    
    class ProviderConfig {
        +api_key: String
        +base_url: Option~String~
        +model: String
        +enabled: bool
    }
    
    class ExecutionConfig {
        +max_retries: u32
        +timeout_seconds: u64
        +parallel_execution: bool
        +max_concurrent_tasks: usize
    }
    
    class UiConfig {
        +theme: String
        +show_progress: bool
        +auto_scroll: bool
        +max_history: usize
    }
    
    class ContextConfig {
        +max_context_size: usize
        +auto_save: bool
        +compression_enabled: bool
        +retention_days: u32
    }
    
    ConfigManager --> ConfigData : manages
    ConfigData --> LlmConfig : contains
    ConfigData --> ExecutionConfig : contains
    ConfigData --> UiConfig : contains
    ConfigData --> ContextConfig : contains
    LlmConfig --> ProviderConfig : contains
```

## File System Operations Component

```mermaid
classDiagram
    class FileSystemOperations {
        <<interface>>
        +read_file() Result~String~
        +write_file() Result~()~
        +list_directory() Result~Vec~PathBuf~~
        +create_directory() Result~()~
        +delete_path() Result~()~
        +exists() bool
        +is_file() bool
        +is_directory() bool
    }
    
    class PathUtils {
        +normalize_path() PathBuf
        +resolve_relative() Result~PathBuf~
        +get_file_extension() Option~String~
        +is_safe_path() bool
        +get_project_root() Option~PathBuf~
    }
    
    class SafeFileOperations {
        +path_utils: PathUtils
        +validate_path() Result~()~
        +safe_read() Result~String~
        +safe_write() Result~()~
        +safe_delete() Result~()~
    }
    
    FileSystemOperations <|.. SafeFileOperations : implements
    SafeFileOperations --> PathUtils : uses
```

## Error Handling Component

```mermaid
classDiagram
    class KaiError {
        <<enumeration>>
        Config(String)
        Context(String)
        Execution(String)
        Llm(LlmError)
        Ui(String)
        FileSystem(String)
        Network(String)
    }
    
    class ErrorHandler {
        +handle_error() Result~()~
        +log_error() Result~()~
        +should_retry() bool
        +get_retry_delay() Duration
        +format_user_message() String
    }
    
    class RetryPolicy {
        +max_retries: u32
        +initial_delay: Duration
        +max_delay: Duration
        +backoff_factor: f64
        +calculate_delay() Duration
        +should_retry() bool
    }
    
    ErrorHandler --> KaiError : handles
    ErrorHandler --> RetryPolicy : uses
```

## Agentic Loop Architecture

```mermaid
stateDiagram-v2
    [*] --> Initialize : Start execution
    
    Initialize --> Planning : Initialize planning phase
    Planning --> GeneratePlan : Use LLM to create plan
    GeneratePlan --> ValidatePlan : Validate generated plan
    
    ValidatePlan --> ExecuteTask : Plan valid, start execution
    ValidatePlan --> RefinePlan : Plan needs refinement
    
    RefinePlan --> GeneratePlan : Create updated plan
    
    ExecuteTask --> PreExecutionRefine : Refine task instruction with LLM
    PreExecutionRefine --> ActualExecution : Execute the refined task
    ActualExecution --> PostExecutionAnalysis : Analyze results with LLM
    
    PostExecutionAnalysis --> UpdateContext : Update context with results
    UpdateContext --> CheckQueue : Check for more tasks
    
    CheckQueue --> ExecuteTask : More tasks available
    CheckQueue --> PlanEvaluation : No more tasks
    
    PlanEvaluation --> Complete : Plan successfully executed
    PlanEvaluation --> AdaptivePlanning : Plan needs adjustment
    
    AdaptivePlanning --> GeneratePlan : Create adaptive plan
    Complete --> [*] : Execution finished
    
    note right of PreExecutionRefine
        LLM refines task instruction
        based on current context
        and execution environment
    end note
    
    note right of PostExecutionAnalysis
        LLM analyzes execution results
        and suggests next steps or
        identifies issues
    end note
```

## Streaming Architecture

```mermaid
classDiagram
    class StreamingLlmProvider {
        <<interface>>
        +generate_stream() Result~LlmStream~
    }
    
    class LlmStream {
        +stream: Pin~Box~Stream~~
        +collect() Result~LlmResponse~
        +for_each() Result~()~
    }
    
    class StreamChunk {
        +delta: String
        +tool_calls: Option~Vec~ToolCall~~
        +finish_reason: Option~String~
        +usage: Option~TokenUsage~
    }
    
    class StreamCollector {
        +content: String
        +tool_calls: Vec~ToolCall~
        +usage: Option~TokenUsage~
        +process_chunk() Result~()~
        +into_response() LlmResponse
    }
    
    class StreamProcessor {
        +process_content_stream() Stream~String~
        +collect_final_response() Result~LlmResponse~
        +merge_streams() LlmStream
    }
    
    StreamingLlmProvider --> LlmStream : produces
    LlmStream --> StreamChunk : yields
    StreamCollector --> StreamChunk : processes
    StreamCollector --> LlmResponse : creates
    StreamProcessor --> LlmStream : manipulates
```

## Task Types and Execution Patterns

```mermaid
graph TD
    subgraph "File Operations"
        ReadTask[Read File Task]
        WriteTask[Write File Task]
        CreateDirTask[Create Directory Task]
        DeleteTask[Delete Task]
        ListTask[List Files Task]
    end
    
    subgraph "Command Operations"
        ExecTask[Execute Command Task]
        ShellCmd[Shell Command Execution]
        ProcessMgmt[Process Management]
    end
    
    subgraph "AI Operations"
        AnalyzeTask[Analyze Code Task]
        LLMCall[LLM API Call]
        ContextAssembly[Context Assembly]
    end
    
    subgraph "Execution Pipeline"
        Validate[Validate Task]
        PreProcess[Pre-processing]
        Execute[Execute Task]
        PostProcess[Post-processing]
        Result[Generate Result]
    end
    
    ReadTask --> Validate
    WriteTask --> Validate
    CreateDirTask --> Validate
    DeleteTask --> Validate
    ListTask --> Validate
    ExecTask --> Validate
    AnalyzeTask --> Validate
    
    Validate --> PreProcess
    PreProcess --> Execute
    Execute --> PostProcess
    PostProcess --> Result
    
    Execute -.-> ShellCmd
    Execute -.-> LLMCall
    Execute -.-> ContextAssembly
    Execute -.-> ProcessMgmt
    
    classDef fileOp fill:#e1f5fe
    classDef cmdOp fill:#fff3e0
    classDef aiOp fill:#e8f5e8
    classDef pipeline fill:#f3e5f5
    
    class ReadTask,WriteTask,CreateDirTask,DeleteTask,ListTask fileOp
    class ExecTask,ShellCmd,ProcessMgmt cmdOp
    class AnalyzeTask,LLMCall,ContextAssembly aiOp
    class Validate,PreProcess,Execute,PostProcess,Result pipeline
```

## Component Communication Protocols

### Async Message Passing
- **Execution Engine ↔ LLM Provider**: Async requests with Result types
- **UI Manager ↔ Execution Engine**: Event-driven communication with callbacks
- **Context Manager ↔ All Modules**: Shared state through Arc<RwLock<>>
- **Task Executor ↔ File System**: Direct async I/O operations

### Error Propagation
- **Bottom-up**: Low-level errors bubble up through Result types
- **Cross-cutting**: Logging system captures errors at all levels
- **User-facing**: UI layer translates technical errors to user-friendly messages

### State Synchronization
- **Context Updates**: Atomic updates through RwLock mechanism
- **Configuration Changes**: Immediate propagation to affected modules
- **Execution State**: Shared state with atomic operations

### Performance Considerations
- **Async Operations**: Non-blocking I/O throughout the system
- **Streaming**: Chunked processing for large LLM responses
- **Caching**: Context and configuration caching for performance
- **Resource Management**: Proper cleanup and resource deallocation