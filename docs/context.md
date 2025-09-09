Specification: Grounded Context Module for KAI-X

1. Overview & Core Principles

This document outlines the specification for a Grounded Context Module (GCM) for the KAI-X AI agent. The primary goal is to transform KAI-X from an abstract conversationalist into a grounded agent that can
safely and effectively interact with a user's local filesystem.

The GCM will serve as the central nervous system for KAI-X's environmental interactions, managing how the agent perceives, reasons about, and acts upon the local codebase.

Core Principles:

* Grounded in Reality: The agent's knowledge must be derived directly from the user's filesystem, not from assumptions or user-provided snippets.
* Active, Not Passive: The agent must be able to seek out information proactively, not just wait for it to be provided.
* Safety First: All interactions with the filesystem must be mediated through a well-defined, auditable tool interface.
* Context is Managed: The agent must intelligently manage its limited context window, focusing only on the information relevant to the immediate task.

2. Functional Requirements

The GCM will be composed of three distinct but interconnected layers.

##### Layer 1: The Filesystem Interaction Toolkit (FIT)

This layer is a non-negotiable security boundary. It is the only component that has direct access to the filesystem. The agent itself will not be able to execute arbitrary shell commands; it will only be
able to invoke the specific, hardened tools provided by the FIT.

Required Tools:

1. `list_directory(path: str, recursive: bool = False) -> List[str]`
    * Purpose: To discover the structure of the project.
    * Implementation: Should return a list of files and directories at the given path. The recursive flag allows for a full project tree listing. Must be sandboxed to prevent access to sensitive directories
      outside the project root (e.g., ~/.ssh, /etc).

2. `read_file(path: str, start_line: int = 0, end_line: int = -1) -> str`
    * Purpose: To read the content of specific files.
    * Implementation: Returns the content of the file at path. Must support reading specific line ranges to manage the context window effectively. Should include safeguards against reading excessively large
      files into memory at once.

3. `search_content(pattern: str, path: str = ".") -> Dict[str, List[Tuple[int, str]]]`
    * Purpose: To find specific code snippets, function definitions, or keywords.
    * Implementation: This is the agent's primary discovery tool. It should execute a regular expression pattern search within the specified path. It must return a structured dictionary where keys are file
      paths and values are a list of tuples, each containing a line number and the matching line's content. This is more useful than a raw text dump. ripgrep (rg) is the preferred underlying search tool for
      performance.

4. `apply_patch(patch_content: str) -> bool`
    * Purpose: To make precise, auditable changes to files.
    * Implementation: Accepts a string in a standard diff or unified format. The tool will parse this string and apply the changes to the relevant files. It must not simply overwrite files. This is a
      critical safety feature. The function should return True on success and False on failure (e.g., if the patch does not apply cleanly).

##### Layer 2: The Working Context Manager (WCM)

This is the "brain" of the GCM. It is responsible for the intelligent process of using the FIT tools to build a relevant, in-memory context that the agent can use for reasoning.

Functional Requirements:

1. Context State: The WCM must maintain an internal state representing the agent's current understanding of the codebase. This could be a dictionary or a more complex data structure that stores the content
   of files the agent has "opened."
2. Iterative Context Building: The WCM will not be a single function call. It will be driven by the agent's reasoning loop.
    * The agent's core prompt will instruct it to form a hypothesis about where to find relevant information.
    * The agent will then request a tool call from the FIT (e.g., search_content("UserLogin")).
    * The WCM will process the result of this tool call and update its internal context state.
    * The updated context is then fed back into the agent's prompt for the next reasoning step.
3. Dependency Chaining: The WCM must be able to parse the content it has read to identify dependencies (e.g., import, require). This information should be used to prompt the agent to automatically fetch and
   load those dependencies into the context.
4. Context Pruning: The WCM should have a mechanism to manage the size of the context passed to the LLM, such as a Least Recently Used (LRU) cache policy for the files it holds in memory.

##### Layer 3: The Strategic Context Injector (SCI)

This layer provides the agent with its goals, rules, and project-specific instructions.

Functional Requirements:

1. Core Prompt Injection: The SCI must ensure that a "core prompt" or "constitution" is prepended to every LLM call. This prompt will define the agent's core behaviors, including the mandatory use of the
   planning module and the FIT tools.
2. `KAIX.md` Discovery:
    * On startup, or when the working directory changes, the SCI must automatically search for a file named KAIX.md (or .kaix.md) in the current directory and all parent directories up to the project root.
    * The contents of all discovered KAIX.md files must be collected.
3. `KAIX.md` Injection: The collected contents of the KAIX.md files must be injected into the agent's prompt, clearly demarcated as "Project-Specific Instructions." This gives the agent the necessary
   strategic context for the current repository.
4. Plan Integration: The SCI will be responsible for integrating the output of a separate "Planning Module" into the prompt, ensuring the agent is always aware of its current step in the overall plan.

3. Implementation Plan & Agent Instructions

When you begin development, follow this sequence:

1. Implement the FIT first. This is the foundation. Each tool must be robust, secure, and heavily tested. Expose these tools to the agent as its only means of interacting with the world.
2. Develop the SCI. Implement the logic for finding and injecting the core prompt and the KAIX.md files. This provides the "rules of the road" for the agent.
3. Build the WCM. This is the most complex part. You will need to modify the agent's core reasoning loop. Instead of a simple chat history, the agent's input will now be a composite context assembled by the
   WCM, including the strategic context from the SCI and the working context of file contents.
4. Refine the Core Prompt. The core prompt must be updated to teach the agent how to use this new mechanism. It needs to be explicitly instructed to explore, search, and read files before attempting to write
   code, and to use the apply_patch tool for all modifications.

This specification provides a clear roadmap for building a powerful and safe Grounded Context Module for KAI-X.