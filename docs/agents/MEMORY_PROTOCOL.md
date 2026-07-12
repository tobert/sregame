# MEMORY_PROTOCOL.md - Agent Memory System Guide

**Purpose**: Enable seamless context persistence across sessions and between different AI models.

## 📊 Overview

This memory system provides **<2000 tokens** of persistent context that survives across:
- Session boundaries (closing/reopening editor)
- Model switches (Claude → Gemini → GPT-4)
- Context window resets
- Days, weeks, or months between work sessions

Combined with **Git**, you have two layers of memory:
1. **Code-level memory** (commit messages): Why changes were made
2. **Project-level memory** (docs/agents/): What's happening now

## 📁 The Four Memory Files

### 1. NOW.md - The Present Moment
**Update frequency**: Start and end of every session
**Purpose**: Immediate working state

**What to include:**
- Active task you're working on right now
- Current implementation status
- Known issues blocking progress
- Questions that need answers
- What the next agent should focus on

**When to update:**
- Starting work: Read to understand current state
- During work: Update as you learn or pivot
- Ending work: Summarize state for next session

**Example:**
```markdown
## 🎯 Active Work
Implementing player animation system (Step 03 of build plan)

## 📍 Current State
- TextureAtlas loaded for player sprite sheet
- Created AnimationState component
- Stuck on: animation timer not ticking in system
- Next: Debug system ordering, check if Timer needs manual advance

## 🚧 Known Issues
1. Animation timer not advancing (timer.tick() not being called?)
```

### 2. PATTERNS.md - Reusable Knowledge
**Update frequency**: When you discover something useful
**Purpose**: Solutions that work, anti-patterns to avoid

**What to include:**
- Code patterns that solved problems
- Bevy-specific idioms
- Common pitfalls and their solutions
- Best practices discovered through trial and error
- Links between concepts

**When to update:**
- You solve a non-obvious problem
- You discover a better way to do something
- You encounter a gotcha worth documenting
- You want to remember "how we do X in this project"

**Example:**
```markdown
## Camera Bounds Pattern
// Always check map size before constraining
let half_width = (WINDOW_WIDTH / 2.0).min(map_width / 2.0);

## Why
Prevents panic when map is smaller than viewport.
Discovered after runtime crash with small test map.
```

### 3. CONTEXT.md - The Bridge
**Update frequency**: When project state significantly changes
**Purpose**: Fast onboarding for new sessions/agents

**What to include:**
- 5 key facts every agent needs
- Critical file locations
- How to test the project
- What's working vs not implemented
- Immediate next tasks to choose from
- Handoff checklist

**When to update:**
- Major features completed
- Project structure changes
- New critical facts emerge
- Before long breaks

**This file is your "README for agents"** - optimize for speed to productivity.

### 4. MEMORY_PROTOCOL.md - This Guide
**Update frequency**: Rarely (only when protocol evolves)
**Purpose**: How to use the memory system

You're reading it now. Only update if you discover better ways to use the system.

## 🔄 Daily Workflow

### Starting a Session
```bash
# 1. Load git context (code memory)
git log --oneline -10
git show HEAD

# 2. Load agent memory (project memory)
# Read NOW.md - what's the current state?
# Skim CONTEXT.md - refresh key facts
# Browse PATTERNS.md - remember solutions

# 3. Start a topic branch for the work
git switch -c feat-what-you-are-about-to-do

# 4. Update NOW.md
# Set your active task
# Note your starting point
```

### During Work
```bash
# Commit early and often; refine the message as you learn:
git commit
git commit --amend    # Update with new insights (unpushed commits only)

# When you solve something interesting:
# Add pattern to PATTERNS.md

# If you change direction:
# Update NOW.md active task
```

### Ending a Session
```bash
# 1. Update memory files
# NOW.md: Current state, blockers, next steps
# PATTERNS.md: New solutions discovered
# CONTEXT.md: If major changes occurred

# 2. Finalize the commit message
git commit -m "feat: what you built - why

Why: [problem]
Approach: [solution]
Learned: [insights]
Next: [specific next action]

🤖 YourModel <email>"

# 3. Persist to GitHub
git push -u origin HEAD

# 4. Verify
cargo check && cargo test
```

## 🤝 Model Handoffs

**Scenario**: Claude finishes work, Gemini picks up later.

**Claude's exit:**
```markdown
# In NOW.md
Status: handoff
Context:
- Implemented player animation system
- All tests passing, animations look smooth
- Timer system requires manual tick() in Update
- Created AnimationState component with 4 directions
Next: Add idle animations when player velocity is zero

# In the commit message
Status: complete
Next: Implement idle animations (see AnimationState TODO)
```

**Gemini's entry:**
```bash
git log --oneline -10     # See Claude's work
git show <claude-commit>  # Read full message + diff
cat docs/agents/NOW.md    # Get current state

# Now Gemini knows:
# - What Claude built
# - Why decisions were made
# - Exactly what to do next
```

## 💡 Writing Effective Memory

### Good NOW.md Entry
```markdown
## 🎯 Active Work
Debugging dialogue auto-wrap at 80 chars

## 🚧 Current Issue
Lines wrap mid-word, need word-boundary detection
Tried: str.split_whitespace() but loses multiple spaces
Currently: Investigating unicode-segmentation crate

## Next Steps
1. Try unicode-segmentation for proper word boundaries
2. Preserve intentional whitespace in dialogue files
3. Add unit tests for wrap_text() function
```

### Bad NOW.md Entry
```markdown
## Active Work
Fixing dialogue

## Issue
It's broken

## Next
Fix it
```

**Why bad?** No context for next agent. What's broken? How? What did you try?

### Good PATTERNS.md Entry
```markdown
## Event-Driven Dialogue Pattern
```rust
#[derive(Event)]
enum StartDialogueEvent {
    Message(String),
    Conversation(Handle<DialogueData>),
}
```

Why: Decouples NPC interaction from dialogue rendering
Learned: Events let systems communicate without direct references
Use when: Any system needs to trigger dialogue
```

### Bad PATTERNS.md Entry
```markdown
## Dialogue
Use events for dialogue.
```

**Why bad?** No code example, no context, no reasoning.

## 🎯 Success Metrics

You're using the system well when:

1. **New sessions start fast** - You understand current state in <2 minutes
2. **No duplicate work** - Patterns file prevents re-learning
3. **Smooth handoffs** - Other models continue without asking "what did you mean?"
4. **Knowledge compounds** - Patterns file grows with project wisdom
5. **Context stays fresh** - NOW.md reflects actual current state

## ⚠️ Anti-Patterns to Avoid

❌ **Stale NOW.md** - Says "implementing feature X" but that's done
❌ **Vague descriptions** - "Fixed stuff" tells next agent nothing
❌ **Write-only memory** - Never reading before starting work
❌ **Pattern hoarding** - Solved something cool? Document it!
❌ **Ignoring git history** - Memory system + git log = complete context

## 🔗 Integration with Git

**Memory files** (docs/agents/) answer: *What's happening now?*
**Commit messages** answer: *Why did we do this?*

Together they create complete context:
```
NOW.md: "Implementing portal system between maps"
  ↓
git log: Shows sequence of changes
  ↓
git show <commit>: "feat: portal collision detection - connects scenes

Why: Need map transitions for Town → Team Marathon
Approach: Portal component with target scene + spawn point
Learned: State transitions must cleanup entities with DespawnOnExit
Next: Add fade transition effect"
  ↓
PATTERNS.md: Documents portal pattern for reuse
```

## 🚀 Quick Start for New Agents

1. Read `CLAUDE.md` (project guidelines)
2. Read `CONTEXT.md` (fast onboarding)
3. Read `NOW.md` (current state)
4. Run `git log --oneline -10` (recent work)
5. Browse `PATTERNS.md` (available solutions)
6. Start coding with full context!

---

## Attribution

Memory protocol adapted from otlp-mcp project.
Created for sregame by Claude, 2025-11-09.

**Remember**: Good memory = better continuity = faster progress.
Write for your future self and other models. They'll thank you.
