# KAI-X Data Flow Diagrams

This document illustrates how data flows through the KAI-X system during various operations.

## Main Application Data Flow

```mermaid
sequenceDiagram
    participant User
    participant CLI
    participant Main
    participant Config
    participant Context
    participant Engine
    participant LLM
    participant Executor
    
    User->>CLI: Command Input
    CLI->>Main: Parse Arguments
    Main->>Config: Load Configuration
    Config-->>Main: Config Data
    Main->>Context: Initialize Context
    Context-->>Main: Context Manager
    Main->>Engine: Create Execution Engine
    Engine-->>Main: Engine Instance
    
    alt Interactive Mode
        Main->>Engine: Start Interactive Loop
        loop Chat Session
            Engine->>User: Prompt for Input
            User->>Engine: User Message
            Engine->>Context: Update Context
            Engine->>LLM: Generate Response
            LLM-->>Engine: AI Response
            Engine->>User: Display Response
        end
    else Single Prompt Mode
        Main->>Engine: Execute Single Prompt
        Engine->>LLM: Process Prompt
        LLM-->>Engine: AI Response
        Engine->>Executor: Execute Tasks
        Executor-->>Engine: Task Results
        Engine->>User: Final Output
    end
```

## Task Execution Flow

```mermaid
flowchart TD
    Start([User Request]) --> Parse[Parse Request]
    Parse --> Plan[Generate Plan via LLM]
    Plan --> Queue[Add Tasks to Queue]
    Queue --> Dequeue[Dequeue Next Task]
    
    Dequeue --> TaskType{Task Type?}
    
    TaskType -->|Read File| ReadFile[Read File Content]
    TaskType -->|Write File| WriteFile[Write File Content]
    TaskType -->|Execute Command| ExecCmd[Execute Shell Command]
    TaskType -->|Analyze Code| AnalyzeCode[Analyze Code via LLM]
    TaskType -->|List Files| ListFiles[List Directory Contents]
    TaskType -->|Create Dir| CreateDir[Create Directory]
    TaskType -->|Delete| Delete[Delete File/Directory]
    
    ReadFile --> Result[Task Result]
    WriteFile --> Result
    ExecCmd --> Result
    AnalyzeCode --> Result
    ListFiles --> Result
    CreateDir --> Result
    Delete --> Result
    
    Result --> Analyze[LLM Post-Execution Analysis]
    Analyze --> UpdateContext[Update Context]
    UpdateContext --> MoreTasks{More Tasks in Queue?}
    
    MoreTasks -->|Yes| Dequeue
    MoreTasks -->|No| Complete[Execution Complete]
    
    Complete --> Output[Present Results to User]
    Output --> End([End])
```

## LLM Provider Data Flow

```mermaid
graph LR
    subgraph "Request Processing"
        Req[User Request] --> Factory[LLM Provider Factory]
        Factory --> Provider{Provider Type}
        Provider -->|openrouter| OR[OpenRouter Provider]
        Provider -->|gemini| Gemini[Gemini Provider]
    end
    
    subgraph "OpenRouter Flow"
        OR --> ORFormat[Format Request for OpenRouter API]
        ORFormat --> ORRetry[Retry Logic with Backoff]
        ORRetry --> ORCall[HTTP API Call]
        ORCall --> ORResponse[Parse API Response]
    end
    
    subgraph "Gemini Flow"
        Gemini --> GeminiFormat[Format Request for Gemini API]
        GeminiFormat --> GeminiCall[HTTP API Call]
        GeminiCall --> GeminiResponse[Parse API Response]
    end
    
    subgraph "Response Processing"
        ORResponse --> Common[Common Response Format]
        GeminiResponse --> Common
        Common --> Stream{Streaming?}
        Stream -->|Yes| StreamChunks[Process Stream Chunks]
        Stream -->|No| DirectResponse[Direct Response]
        StreamChunks --> FinalResponse[Assembled Response]
        DirectResponse --> FinalResponse
    end
    
    FinalResponse --> Output[Return to Execution Engine]
    
    classDef providerNode fill:#e8f5e8
    classDef streamNode fill:#e1f5fe
    classDef responseNode fill:#fff3e0
    
    class OR,Gemini providerNode
    class StreamChunks,Stream streamNode
    class Common,FinalResponse,Output responseNode
```

## Context Management Flow

```mermaid
stateDiagram-v2
    [*] --> Initialize
    
    Initialize --> LoadGlobalContext: Load existing context
    LoadGlobalContext --> CreatePlanContext: Create new plan context
    CreatePlanContext --> Ready: Context ready for use
    
    Ready --> UpdateFromUser: User provides new information
    Ready --> UpdateFromExecution: Task execution results
    Ready --> UpdateFromPlanning: Planning system updates
    
    UpdateFromUser --> ProcessUpdate: Process and validate update
    UpdateFromExecution --> ProcessUpdate
    UpdateFromPlanning --> ProcessUpdate
    
    ProcessUpdate --> MergeContext: Merge with existing context
    MergeContext --> Ready: Context updated
    
    Ready --> SaveContext: Periodic save or on completion
    SaveContext --> Ready
    
    Ready --> Cleanup: Session ending
    Cleanup --> [*]
    
    note right of ProcessUpdate
        Updates include:
        - File contents
        - Execution results
        - User preferences
        - Project structure
        - Error information
    end note
```

## Configuration Flow

```mermaid
graph TD
    Start([Application Start]) --> CheckConfig{Config Exists?}
    
    CheckConfig -->|No| InitConfig[Initialize Default Config]
    CheckConfig -->|Yes| LoadConfig[Load Existing Config]
    
    InitConfig --> CreateDirs[Create Config Directories]
    CreateDirs --> WriteDefault[Write Default Config Files]
    WriteDefault --> ValidateNew[Validate New Config]
    
    LoadConfig --> ValidateExisting[Validate Existing Config]
    
    ValidateNew --> ConfigReady[Configuration Ready]
    ValidateExisting --> ConfigReady
    
    ConfigReady --> Runtime[Runtime Configuration Access]
    
    Runtime --> ModifyConfig{Config Modification?}
    ModifyConfig -->|Yes| UpdateConfig[Update Configuration]
    ModifyConfig -->|No| Runtime
    
    UpdateConfig --> ValidateUpdate[Validate Update]
    ValidateUpdate --> SaveConfig[Save to Disk]
    SaveConfig --> Runtime
    
    Runtime --> Shutdown[Application Shutdown]
    Shutdown --> End([End])
    
    classDef configNode fill:#f3e5f5
    classDef validateNode fill:#fce4ec
    classDef fileNode fill:#f1f8e9
    
    class InitConfig,LoadConfig,UpdateConfig configNode
    class ValidateNew,ValidateExisting,ValidateUpdate validateNode
    class CreateDirs,WriteDefault,SaveConfig fileNode
```

## Error Handling Flow

```mermaid
graph TD
    Error[Error Occurs] --> Classify{Error Type}
    
    Classify -->|Network Error| NetworkRetry[Retry with Backoff]
    Classify -->|Authentication Error| AuthError[Report Auth Failure]
    Classify -->|Rate Limit| RateLimit[Wait and Retry]
    Classify -->|File System Error| FileError[Handle File Error]
    Classify -->|LLM Error| LLMError[Handle LLM Error]
    Classify -->|Execution Error| ExecError[Handle Execution Error]
    
    NetworkRetry --> RetryCheck{Retry Count < Max?}
    RetryCheck -->|Yes| Wait[Wait with Exponential Backoff]
    RetryCheck -->|No| FailPermanent[Permanent Failure]
    Wait --> Retry[Retry Operation]
    Retry --> Success{Operation Successful?}
    Success -->|Yes| Complete[Operation Complete]
    Success -->|No| Error
    
    AuthError --> UserAction[Prompt User for Credentials]
    RateLimit --> WaitPeriod[Wait for Rate Limit Reset]
    FileError --> FileRecovery[Attempt File Recovery]
    LLMError --> LLMFallback[Try Alternative LLM Provider]
    ExecError --> ExecRecovery[Log and Continue with Next Task]
    
    UserAction --> Retry
    WaitPeriod --> Retry
    FileRecovery --> Retry
    LLMFallback --> Retry
    ExecRecovery --> Complete
    
    FailPermanent --> Log[Log Error Details]
    Log --> UserNotify[Notify User of Failure]
    UserNotify --> Graceful[Graceful Degradation]
    Graceful --> End([End])
    
    Complete --> End
    
    classDef errorNode fill:#ffebee
    classDef retryNode fill:#fff3e0
    classDef successNode fill:#e8f5e8
    
    class Error,Classify,FailPermanent errorNode
    class NetworkRetry,RetryCheck,Wait,Retry retryNode
    class Complete,Success successNode
```

## Data Types and Structures

### Key Data Structures Flow
- **Messages**: User input → CLI parsing → Message struct → LLM processing
- **Tasks**: Plan generation → Task queue → Individual task execution → Results
- **Context**: Global context + Plan context → Context updates → Persistent storage
- **Configurations**: File system → Config manager → Runtime access → Modifications
- **Responses**: LLM API → Response parsing → Stream processing → User output