---
description: How to plan and create a new feature track
---

# Planning a New Track

Follow these exact steps to create a new feature track:

## 1. Create Track Directory Structure

Create a new track directory under `conductor/tracks/` with format `track_name_YYYYMMDD/`:
- Create `conductor/tracks/<track_name>_<YYYYMMDD>/`
- Create `plan.md` inside that directory

## 2. Define Track in plan.md

Include in plan.md:
- Track title and description
- Phases with numbered tasks
- Success metrics
- Dependencies
- Risks and mitigations

Use format:
```markdown
# Track Name

Description...

## Phases

### Phase 1: Name
- [ ] Task 1
- [ ] Task 2
...

### Phase 2: Name
...

## Success Metrics
...

## Dependencies
...
```

## 3. Update GEMINI.md

Add the new track to GEMINI.md with:
- Track name and description
- Current phase
- Last updated date

## 4. Checkpoint Protocol

After each significant phase completion:
1. Create backup tag before major changes: `git tag pre-<track-name>-track`
2. Commit with format: `feat(<area>): <description>`
3. Update plan.md with checkpoint commit hash
4. Mark tasks complete with [x]

## 5. Start Implementation

Begin with Phase 1 tasks, following the plan exactly.
