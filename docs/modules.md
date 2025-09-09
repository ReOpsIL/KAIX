# Codex Architecture Diagram

This diagram illustrates the major modules of the Codex architecture and the flow of a typical user request.

It shows how a user's prompt initiates a cycle of reasoning and action, mediated by a structured "Context Stack" and a secure "Tool Execution" layer, with the "Planning Module" guiding the entire process.

```mermaid
graph TD
    subgraph User Interface
        CLI("CLI (User Prompt)")
    end

    subgraph Core Agent
        AgentLoop("Core Agent Loop / Reasoning Engine")
    end

    subgraph Context Stack
        direction TB
        SCI("Layer 3: Strategic Context Injector")
        WCM("Layer 2: Working Context Manager")
        FIT("Layer 1: Filesystem Interaction Toolkit")

        SCI -- "Injects Rules & AGENTS.md" --> AgentLoop
        WCM -- "Provides In-Memory Context" --> AgentLoop
        AgentLoop -- "Requests File/Search Data" --> WCM
        WCM -- "Requests Low-Level I/O" --> FIT
        FIT -- "Returns File/Search Results" --> WCM
    end

    subgraph Tool Execution
        ApprovalManager("Approval Manager")
        ToolExecutor("Tool Executor")
    end

    subgraph State & Strategy
        PlanningModule("Planning Module (update_plan)")
    end

    subgraph External World
        FileSystem[("Filesystem (User's Project)")]
    end

    %% --- Flows ---
    CLI -- "1. User sends prompt" --> AgentLoop
    AgentLoop -- "2. Decides task is complex" --> PlanningModule
    PlanningModule -- "3. Creates/updates plan" --> AgentLoop
    AgentLoop -- "4. Forms hypothesis, decides to call a tool (e.g., rg, apply_patch)" --> ApprovalManager
    ApprovalManager -- "5. Checks policy (may prompt user via CLI)" --> ToolExecutor
    ToolExecutor -- "6. Executes command/patch" --> FileSystem
    FileSystem -- "7. Returns command output" --> ToolExecutor
    ToolExecutor -- "8. Sends result back" --> AgentLoop
    AgentLoop -- "9. Updates plan & context, continues loop until done" --> PlanningModule
    AgentLoop -- "10. Sends final summary to user" --> CLI

    %% --- Style ---
    classDef agent fill:#c9d1fc,stroke:#333,stroke-width:2px;
    classDef context fill:#d4edda,stroke:#333,stroke-width:2px;
    classDef tools fill:#f8d7da,stroke:#333,stroke-width:2px;
    classDef user fill:#fff3cd,stroke:#333,stroke-width:2px;
    classDef state fill:#e2e3e5,stroke:#333,stroke-width:2px;

    class AgentLoop,CLI agent;
    class SCI,WCM,FIT context;
    class ApprovalManager,ToolExecutor tools;
    class PlanningModule state;
    class FileSystem user;
```

### Explanation of the Flow:

1.  **Prompt:** The user initiates a task through the **CLI**.
2.  **Reasoning & Planning:** The **Core Agent Loop** receives the prompt. It consults the **Strategic Context** (its core rules + `AGENTS.md`) and, for any non-trivial task, calls the **Planning Module** to create a step-by-step plan.
3.  **Context Building:** To execute the plan, the agent needs information. It requests data via the **Working Context Manager (WCM)**, which in turn uses the **Filesystem Interaction Toolkit (FIT)** to run commands like `rg` (search) or `cat` (read) on the actual **Filesystem**.
4.  **Tool Execution & Approval:** When the agent decides to take an action (like reading a file or applying a patch), the request is sent to the **Approval Manager**. This module acts as a security gate, checking the current policy and prompting the user for confirmation if necessary.
5.  **Action:** If approved, the **Tool Executor** runs the command or applies the patch to the files on disk.
6.  **Iterate:** The result of the action is fed back into the **Agent Loop**, which updates its **Working Context** and the **Plan**. The agent then continues to the next step, repeating the cycle until the task is complete.
7.  **Response:** Once the plan is fully executed, the agent generates a final summary and presents it to the user via the **CLI**.
