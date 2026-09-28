---
name: reclaim
description: Instructions for AI agents to inspect, analyze, and propose safe cleanup plans on macOS using Project Reclaim.
---

# Reclaim Agent Skill

This skill guides AI agents (Claude, Codex, Antigravity, or custom MCP agents) on how to safely interact with Project Reclaim.

## Operating Principles for Agents

1. **Never Propose Shell Deletions (`rm`)**: Always use `reclaim` commands to inspect and plan.
2. **Respect the Invariants**: `PROTECTED` items (credentials, source code, user documents, system files) and `UNKNOWN` items must NEVER be scheduled for cleanup.
3. **Two-Phase Workflow**:
   - Phase 1: Understand & Inspect (`reclaim scan --json`, `reclaim candidates --json`).
   - Phase 2: Plan & Explain (`reclaim plan --risk safe --json`, `reclaim explain <id> --json`).
   - Phase 3: Present the immutable plan to the human user for approval.

## Agent Workflows

### 1. Storage Health Diagnosis
```bash
reclaim doctor --json
```
Use this to inspect available volume capacity, APFS filesystem status, and storage pressure before proposing any cleanup action.

### 2. Identifying Safe Cleanup Candidates
```bash
reclaim candidates <path> --risk safe --json
```
Query items that meet deterministic safe cleanup criteria (e.g., idle build artifacts, package manager download caches).

### 3. Explaining an Item to the User
```bash
reclaim explain <candidate_id> --json
```
Read the corroborating evidence vector and rebuild consequences, then summarize clearly for the user why this item was generated and what happens if it is reclaimed.

### 4. Compiling a Cleanup Proposal
```bash
reclaim plan <path> --risk safe --json
```
Generate an immutable plan with a SHA-256 hash. Show the user:
- Total physical space reclaimed.
- List of categories affected.
- Plan ID and confirmation prompt.
