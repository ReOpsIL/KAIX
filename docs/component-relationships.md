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

## Context Management Architecture

```mermaid
classDiagram
    class ContextManager {
        +global_context: GlobalContext
        +plan_context: Option~PlanContext~
        +new() Self
        +update_global_context() Result~()~
        +create_plan_context() Result~()~
        +get_merged_context() String
        +save_context() Result~()~
    }
    
    class GlobalContext {
        +working_directory: PathBuf
        +project_files: HashMap~String, String~
        +user_preferences: UserPreferences
        +session_history: Vec~SessionEntry~
        +new() Self
        +add_file() Result~()~
        +remove_file() Result~()~
        +update_preferences() Result~()~
    }
    
    class PlanContext {
        +current_plan: Option~Plan~
        +execution_history: Vec~TaskExecutionResult~
        +active_tasks: VecDeque~Task~
        +completed_tasks: Vec~Task~
        +new() Self
        +update_plan() Result~()~
        +add_execution_result() Result~()~
    }
    
    class UserPreferences {
        +preferred_language: String
        +coding_style: CodingStyle
        +output_format: OutputFormat
        +auto_save: bool
    }
    
    class SessionEntry {
        +timestamp: DateTime
        +user_input: String
        +ai_response: String
        +execution_results: Vec~TaskExecutionResult~
    }
    
    ContextManager --> GlobalContext : manages
    ContextManager --> PlanContext : manages
    GlobalContext --> UserPreferences : contains
    GlobalContext --> SessionEntry : contains
    PlanContext --> TaskExecutionResult : tracks
```

## UI Component Architecture

```mermaid
classDiagram
    class UiManager {
        +terminal: Terminal
        +event_handler: EventHandler
        +components: Vec~Component~
        +new() Self
        +initialize() Result~()~
        +run() Result~()~
        +handle_event() Result~()~
    }
    
    class EventHandler {
        +event_queue: VecDeque~UiEvent~
        +new() Self
        +register_handler() Result~()~
        +emit_event() Result~()~
        +process_events() Result~()~
    }
    
    class Component {
        <<interface>>
        +render() Result~()~
        +handle_input() Result~EventResult~
        +update() Result~()~
    }
    
    class ChatComponent {
        +messages: Vec~ChatMessage~
        +input_buffer: String
        +scroll_position: usize
        +render() Result~()~
        +add_message() Result~()~
        +scroll() Result~()~
    }
    
    class StatusComponent {
        +execution_state: ExecutionState
        +current_task: Option~String~
        +progress: f32
        +render() Result~()~
        +update_status() Result~()~
    }
    
    class InputComponent {
        +current_input: String
        +history: Vec~String~
        +cursor_position: usize
        +render() Result~()~
        +handle_key() Result~InputResult~
    }
    
    class UiEvent {
        <<enumeration>>
        KeyPress
        Resize
        TaskComplete
        ErrorOccurred
    }
    
    UiManager --> EventHandler : uses
    UiManager --> Component : manages
    Component <|.. ChatComponent : implements
    Component <|.. StatusComponent : implements
    Component <|.. InputComponent : implements
    EventHandler --> UiEvent : processes
    ChatComponent --> UiEvent : emits
    StatusComponent --> UiEvent : handles
    InputComponent --> UiEvent : emits
```

## Planning System Architecture

```mermaid
classDiagram
    class PlanningManager {
        +llm_provider: Box~LlmProvider~
        +context: Arc~RwLock~ContextManager~~
        +new() Self
        +create_plan() Result~Plan~
        +refine_plan() Result~Plan~
        +validate_plan() Result~bool~
    }
    
    class Plan {
        +id: String
        +description: String
        +tasks: Vec~Task~
        +created_at: DateTime
        +estimated_duration: Duration
        +dependencies: HashMap~String, Vec~String~~
        +new() Self
        +add_task() Result~()~
        +get_next_tasks() Vec~Task~
    }
    
    class Task {
        +id: String
        +task_type: TaskType
        +instruction: String
        +parameters: HashMap~String, String~
        +dependencies: Vec~String~
        +priority: u8
        +new() Self
        +is_ready() bool
    }
    
    class TaskType {
        <<enumeration>>
        ReadFile
        WriteFile
        ExecuteCommand
        AnalyzeCode
        ListFiles
        CreateDirectory
        Delete
    }
    
    class PlanValidationResult {
        +is_valid: bool
        +errors: Vec~String~
        +warnings: Vec~String~
        +suggestions: Vec~String~
    }
    
    PlanningManager --> Plan : creates
    PlanningManager --> PlanValidationResult : produces
    Plan --> Task : contains
    Task --> TaskType : has
    PlanningManager --> LlmProvider : uses
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