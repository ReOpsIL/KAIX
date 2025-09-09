 The "Core Agent Loop / Reasoning Engine" is the heart. It's the component that orchestrates everything else, acting as the "CPU" or the "brain" of the agent.

Its fundamental purpose is to answer one question over and over again: "Based on what I know right now, what is the very next action I should take to achieve the user's goal?"

Let's break down its function, inputs, outputs, and the iterative process it follows.

High-Level Concept: A Developer's Thought Process

The best analogy for the Core Agent Loop is the thought process of a human developer. A developer doesn't just instantly write the final code. Instead, they go through a cycle:

1. Understand the Goal: Read the ticket or user request.
2. Gather Context: Look at the existing code, search for relevant files.
3. Form a Plan: Decide on the steps needed to implement the feature or fix the bug.
4. Take a Small Action: Write some code, run a command, read a file.
5. Observe the Result: Did the command work? Did the code compile? What did that file contain?
6. Update Understanding & Repeat: Based on the result, decide on the next small action.

The Core Agent Loop is a programmatic implementation of this exact cycle.

  ---

The "Loop" in Detail

The loop is a continuous cycle that takes in the current state of the world and produces a single action. It repeats until the task is complete.

Here are the steps in one full iteration of the loop:

Step 1: Ingest State & Goal (Perception)

The loop begins by assembling its "Current World Model." This is a snapshot of everything it knows, which is fed into the Large Language Model for reasoning. This includes:

* The User's Goal: The original prompt from the user.
* The Strategic Context: The rules from its core prompt and the instructions from any AGENTS.md files.
* The Plan State: The current to-do list from the Planning Module, showing what's done, in-progress, and pending.
* The Working Context: The content of the files it has "opened" (read into memory) via the WCM.
* The Previous Action's Result: The output (stdout/stderr) from the last tool call it executed.

Step 2: Reason & Strategize (Cognition)

This is the core "thinking" step. The entire World Model is passed to the LLM, which is prompted to decide on the single best action to take next. It might decide to:

* Explore: "I don't have enough information. I need to see the whole project structure."
* Search: "Based on the error message, the problem is likely in a file containing the string 'DatabaseConnection'. I need to search for that."
* Read: "The search found db.py. I need to read its contents to understand how the connection is managed."
* Write/Modify: "I have all the context I need. I understand the bug. I will now formulate a patch to fix it."
* Validate: "I have applied the patch. Now I need to run the tests to make sure I didn't break anything."
* Plan: "This is a new, complex task. My first action should be to create a plan."

Step 3: Formulate Action (Decision)

The reasoning from Step 2 is translated into a precise, machine-readable tool call. The LLM's output isn't just conversational text; it's a structured command.

* If it decided to search, it generates: search_content(pattern="DatabaseConnection")
* If it decided to modify a file, it generates: apply_patch(patch_content="...")
* If it decided to update the plan, it generates: update_plan(steps=[...])

Step 4: Emit Action & Preamble (Communication)

The agent sends two things outwards:

1. To the User: A short, conversational "preamble" message explaining its intent (e.g., "Okay, I see the error. I'm going to search for the database connection logic now."). This provides transparency.
2. To the Tool Execution Layer: The structured tool call formulated in Step 3.

Step 5: Receive & Process Result (Observation)

The loop pauses and waits for the tool call to complete. The Tool Executor returns the result—the contents of a file, the output of a search, the success or failure of a patch. This result is now the
"Previous Action's Result" for the next iteration of the loop.

Step 6: Update World Model & Repeat

The loop restarts at Step 1, but now its "Current World Model" is richer and more up-to-date. The result from the previous action informs the next cycle of reasoning, allowing the agent to build on its
knowledge step-by-step.

Termination Condition

The loop doesn't run forever. It terminates when the reasoning in Step 2 concludes that the plan is complete and the user's goal has been fully achieved. At this point, instead of formulating a tool call,
it formulates the final summary message to the user.

Analogy: Core Agent Loop vs. Human Developer


┌────────────────────────────────┬────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ Agent Loop Step                │ Human Developer Equivalent                                                                                         │
├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ 1. Ingest State & Goal         │ Looking at all your open editor tabs, terminal windows, and the Jira ticket.                                       │
│ 2. Reason & Strategize         │ Thinking: "Okay, the tests failed. The error is 'null pointer'. I'll search for where that variable is defined."   │
│ 3. Formulate Action            │ Deciding to type grep -r "myVariable" into the terminal.                                                           │
│ 4. Emit Action & Preamble      │ (To a colleague) "I'm just going to check where this variable is set." Then, typing the command and hitting Enter. │
│ 5. Receive & Process Result    │ Seeing the output of grep appear in the terminal.                                                                  │
│ 6. Update World Model & Repeat │ "Ah, it's defined in three files. I'll start by reading the most likely one." (The loop begins again).             │
└────────────────────────────────┴────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘


This iterative, stateful loop is what makes Codex a true agent, capable of solving complex, multi-step problems, rather than just a stateless chatbot that answers one-off questions.

╭───────────────────────────────────────────────────────────────────────────╮
│  > create detailed flow diagram of  "Core Agent Loop / Reasoning Engine"  │
╰───────────────────────────────────────────────────────────────────────────╯
