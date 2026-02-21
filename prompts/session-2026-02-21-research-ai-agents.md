# Research: AI Agents in Moltis

## Context

Moltis is a Rust rewrite of OpenClaw. This document surveys what OpenClaw already ships for AI agent functionality, maps that against the moltis codebase, identifies gaps, and recommends next steps.

## What Moltis Already Has

The agentic foundation is solid. Key components:

| Component | Location | Status |
|-----------|----------|--------|
| ReAct agent loop | `crates/agents/src/runner.rs` | Complete — configurable iterations (default 25), timeout (600s), streaming events |
| Tool registry | `crates/agents/src/tool_registry.rs` | Complete — trait-based, source tracking (builtin vs MCP) |
| 25+ built-in tools | `crates/tools/src/` | Complete — exec, browser, web_search, web_fetch, calc, cron, spawn_agent, memory, etc. |
| Sub-agent spawning | `crates/tools/src/spawn_agent.rs` | Complete — depth limit 3, filtered tool registry, model override |
| Provider abstraction | `crates/agents/src/model.rs` | Complete — 10+ providers, streaming, tool-calling, vision, reasoning |
| Provider chain/fallback | `crates/agents/src/provider_chain.rs` | Complete — retry, backoff, model aliasing |
| Hook system | `crates/agents/src/runner.rs` | Complete — BeforeLLMCall, AfterLLMCall, BeforeToolCall, AfterToolCall |
| Tool policies | `crates/config/src/schema.rs` | Complete — allow/deny globs by agent/provider/channel/sender/sandbox |
| Session persistence | `crates/sessions/` | Complete — JSONL + SQLite, branching/forking |
| Memory (RAG) | `crates/memory/` | Complete — SQLite FTS + embeddings, semantic search, LLM reranking |
| Skills | `crates/skills/` | Complete — SKILL.md format, dynamic tool registration, discovery |
| MCP integration | `crates/gateway/src/mcp_service.rs` | Complete — stdio + SSE transport, health polling, tool discovery |
| Cron-triggered agents | `crates/cron/` | Complete — scheduled agent loop invocations |
| Streaming events | `RunnerEvent` enum | Complete — thinking, tool calls, text deltas, sub-agent lifecycle |
| Message queue modes | config `chat.message_queue_mode` | Complete — followup (replay each) or collect (batch) |
| Context overflow detection | `runner.rs` | Complete — detects and surfaces context window errors |
| Sandbox execution | `crates/tools/src/sandbox.rs` | Complete — Docker/Apple Container isolation |

## What OpenClaw Has That Moltis Doesn't (Yet)

### 1. Lobster Workflow Runtime (High Priority)

OpenClaw's differentiating feature. A deterministic, composable pipeline engine:

- **YAML/JSON pipeline definitions** — tool calls chained as explicit steps
- **Approval checkpoints** — halted workflows return a resumable token
- **Conditional steps** and environment variable injection
- **Optional `llm-task` plugin** — structured LLM steps within otherwise deterministic flows
- **One tool call replaces many** — reduces token consumption, improves auditability

**Why it matters:** The current agent loop is pure ReAct — the LLM decides every step. For known workflows (deploy, review, onboard), a deterministic pipeline is cheaper, faster, and more reliable. Lobster turns agent skills into composable, auditable workflows.

**Design considerations:**
- Implement as a new crate (`moltis-workflows` or `moltis-lobster`)
- Pipeline definitions stored as YAML files alongside skills
- Expose as a tool (`workflow.run`) in the agent loop
- Approval gates integrate with the existing hook system
- Persist workflow state for resumability (SQLite or JSONL)

### 2. Plan-and-Execute Mode (Medium Priority)

An alternative to the ReAct loop where the LLM plans all steps upfront, then executes sequentially:

- **Separate planner and executor** — planner uses a reasoning model, executor uses a cheaper model
- **Fewer tokens** on multi-step tasks (no repeated re-planning)
- **Better cost control** — plan is bounded, execution is deterministic-ish
- **Iterative replanning** when steps fail

**Design considerations:**
- Could be a strategy enum on the agent loop: `AgentStrategy::React | AgentStrategy::PlanExecute`
- Planner produces a `Vec<Step>` with tool calls and expected outcomes
- Executor runs each step, checking results against expectations
- Falls back to ReAct if plan fails beyond a threshold
- Configurable per-agent or per-task via config or tool parameter

### 3. Multi-Agent Routing (Medium Priority)

OpenClaw has deterministic multi-agent routing with first-match-wins bindings:

- **Per-channel agent assignment** — different agents for different Telegram chats, web sessions, etc.
- **Isolated state** — each agent gets its own workspace, session, tool permissions, memory
- **Agent-to-agent messaging** — opt-in with allowlists (off by default)

**Current state in moltis:** The `spawn_agent` tool does hierarchical delegation (parent spawns child), but there's no lateral coordination (agent A talks to agent B) or routing (message → best agent).

**Design considerations:**
- Agent definitions in config: `[agents.<name>]` with identity, tools, model, channel bindings
- Router in gateway that maps incoming messages to the right agent
- Optional message bus for inter-agent communication (guarded by allowlist)
- Each agent gets its own session namespace and memory scope

### 4. Repetition/Loop Detection (High Priority, Low Effort)

OpenClaw tracks recent tool-call history and blocks no-progress loops. Moltis has max iterations but no semantic loop detection.

**Design considerations:**
- Track last N tool calls (name + args hash) in the agent loop
- If the same call repeats K times, inject a system message ("You appear to be in a loop. Try a different approach.") or abort
- Configurable thresholds: `tools.loop_detection_window`, `tools.loop_detection_threshold`

### 5. Automatic Context Summarization (Medium Priority)

When approaching context window limits, instead of erroring out:

- **Summarize** earlier conversation turns (keep recent, compress old)
- **Session branching** — fork to handle a sub-problem, merge results back
- **Sliding window** with summary prefix

**Current state:** Moltis detects overflow errors. OpenClaw/Pi handles it with scratchpad summarization and session branching.

**Design considerations:**
- Implement as a pre-LLM-call hook that checks token count and summarizes if needed
- Use a cheap/fast model for summarization
- Preserve tool call results (they're often the most important context)
- Config: `chat.auto_summarize: bool`, `chat.summarize_threshold_pct: f32`

### 6. Skill Marketplace / Registry (Low Priority)

OpenClaw has ClawHub with 5,700+ community skills. Moltis has skill loading from archives but no discovery/sharing.

**Design considerations:**
- Not urgent for core agent functionality
- Needs trust/sandboxing model first (CrowdStrike and Cisco have flagged OpenClaw's skill system for prompt injection risks)
- Could start with a curated list of first-party skills

### 7. Cost Budgeting Per Agent Run (Medium Priority)

Track and limit token/cost consumption per agent invocation:

- **Token budget** — hard cap on total tokens per run
- **Cost estimation** — approximate cost per provider/model
- **Budget alerts** — warn when approaching limits

**Current state:** Moltis tracks `Usage` (input/output tokens) in `AgentRunResult` but doesn't enforce budgets.

**Design considerations:**
- Add `tools.agent_max_tokens: Option<u64>` to config
- Check cumulative usage after each iteration in the agent loop
- Return a budget-exceeded error (not a timeout) when hit

## Architecture Patterns Summary

| Pattern | Description | Moltis Status | Priority |
|---------|-------------|---------------|----------|
| ReAct | Observe → reason → act → repeat | Implemented | — |
| Plan-and-Execute | Plan all steps, then execute | Not implemented | Medium |
| Workflow/Pipeline | Deterministic YAML pipelines (Lobster) | Not implemented | High |
| Multi-Agent Routing | Route messages to specialized agents | Partial (spawn only) | Medium |
| Reflection/Self-Critique | Agent reviews own output before returning | Not implemented (could be a hook) | Low |
| Tool Learning | Track tool success rates, adjust selection | Not implemented | Low |
| Episodic Memory | Time-stamped event memory | Not implemented | Low |
| Loop Detection | Detect and break no-progress cycles | Not implemented | High |
| Context Summarization | Compress history when approaching limits | Not implemented | Medium |
| Cost Budgeting | Token/cost caps per run | Not implemented | Medium |

## Recommended Sequencing

1. **Loop detection** — Low effort, high safety impact. Add to `runner.rs` immediately.
2. **Workflow runtime** — High value differentiator. New crate, ~2-3 weeks.
3. **Cost budgeting** — Small addition to the agent loop. ~1-2 days.
4. **Plan-and-Execute mode** — Meaningful for structured tasks. ~1 week.
5. **Context summarization** — Improves long conversation handling. ~1 week.
6. **Multi-agent routing** — Needed for multi-channel deployments. ~2 weeks.
7. **Reflection/self-critique** — Can be done as a hook or prompt technique. ~2 days.
8. **Skill marketplace** — Lower priority, needs trust model first.

## Security Notes

- **Prompt injection in skills** is a known attack surface (flagged by CrowdStrike, Cisco, Snyk for OpenClaw). Any skill/plugin system needs input validation, sandboxing, and ideally signature verification.
- **Agent-to-agent messaging** should be off by default with explicit allowlists (OpenClaw's approach is correct here).
- **Tool policy enforcement** must cover dynamically loaded tools (MCP, skills) — moltis already does this.
- **Cost budgets** are also a security feature — prevent runaway token consumption from adversarial inputs.
