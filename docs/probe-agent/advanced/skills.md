# Skills System

The skills system enables discoverable, reusable agent capabilities. Skills are markdown files that provide specialized guidance for specific tasks like code review, security analysis, or performance auditing.

---

## TL;DR

```bash
# Create skill directory
mkdir -p .claude/skills/code-review

# Create SKILL.md
cat > .claude/skills/code-review/SKILL.md << 'EOF'
---
name: code-review
description: Guides comprehensive code review
---

# Code Review Skill

When reviewing code, check:
1. Security vulnerabilities
2. Performance issues
3. Error handling
4. Code style
EOF
```

```javascript
const agent = new ProbeAgent({
  path: './project',
  allowSkills: true
});
```

---

## How Skills Work

1. **Discovery**: Agent scans configured directories for skill definitions
2. **Registration**: Valid skills are registered in the SkillRegistry
3. **Listing**: AI can list available skills via `listSkills` tool
4. **Activation**: AI activates skills via `useSkill` tool
5. **Execution**: Full skill instructions are loaded and guide the AI

---

## Creating Skills

### Directory Structure

```
project/
├── .claude/
│   └── skills/
│       ├── code-review/
│       │   └── SKILL.md
│       ├── security-audit/
│       │   └── SKILL.md
│       └── performance-analysis/
│           └── SKILL.md
```

### SKILL.md Format

```yaml
---
name: skill-name
description: Short description (max 400 chars)
---

# Skill Title

Full instructions in markdown format.
```

### Naming Rules

| Rule | Valid | Invalid |
|------|-------|---------|
| Lowercase | `code-review` | `Code-Review` |
| Alphanumeric + hyphens | `api-design` | `api_design` |
| Max 64 characters | `short-name` | `very-long-name-that-exceeds-...` |
| No special chars | `my-skill` | `my@skill` |

**Pattern:** `^[a-z0-9]+(?:-[a-z0-9]+)*$`

---

## Skill Examples

### Code Review Skill

```yaml
---
name: code-review
description: Guides comprehensive code review for quality and best practices
---

# Code Review Skill

## Review Checklist

### 1. Code Quality
- [ ] Clear variable and function names
- [ ] Appropriate abstractions
- [ ] No code duplication
- [ ] Single responsibility principle

### 2. Error Handling
- [ ] All errors caught and handled
- [ ] Meaningful error messages
- [ ] Proper error propagation

### 3. Security
- [ ] Input validation
- [ ] No hardcoded secrets
- [ ] Proper authentication checks

### 4. Performance
- [ ] No unnecessary loops
- [ ] Efficient data structures
- [ ] Proper resource cleanup

## Output Format

Provide findings in categories:
- Critical: Must fix before merge
- Important: Should fix
- Suggestions: Nice to have
```

### Security Audit Skill

```yaml
---
name: security-audit
description: Comprehensive security analysis of code
---

# Security Audit Skill

## Focus Areas

### Authentication
- Check for hardcoded credentials
- Verify password hashing
- Review session management

### Authorization
- Verify permission checks
- Check for privilege escalation
- Review role-based access

### Data Protection
- Check encryption usage
- Review sensitive data handling
- Verify secure storage

### Input Validation
- Check for SQL injection
- Review XSS prevention
- Verify input sanitization

## Severity Levels

- **Critical**: Immediate security risk
- **High**: Significant vulnerability
- **Medium**: Potential issue
- **Low**: Best practice violation
```

### Performance Analysis Skill

```yaml
---
name: performance-analysis
description: Identifies performance bottlenecks and optimization opportunities
---

# Performance Analysis Skill

## Analysis Areas

### Algorithm Complexity
- Identify O(n²) or worse algorithms
- Find inefficient loops
- Check data structure choices

### Database Queries
- Find N+1 query problems
- Check for missing indexes
- Review query efficiency

### Memory Usage
- Identify memory leaks
- Check for unnecessary allocations
- Review object lifecycle

### I/O Operations
- Find blocking I/O
- Check for missing caching
- Review network calls

## Recommendations Format

For each finding:
1. Location (file:line)
2. Current implementation
3. Suggested improvement
4. Expected impact
```

---

## Enabling Skills

```javascript
const agent = new ProbeAgent({
  path: './project',
  allowSkills: true,
  skillDirs: ['.claude/skills', '.codex/skills', 'skills']  // Optional
});
```

### Default Directories

Skills are searched in these directories (relative to repo root):
- `.claude/skills/`
- `.codex/skills/`
- `skills/`
- `.skills/`

---

## Skills API

### SkillRegistry

```javascript
import { SkillRegistry } from '@probelabs/probe/agent';

const registry = new SkillRegistry({
  repoRoot: '/path/to/project',
  skillDirs: ['.claude/skills'],
  debug: true
});

// Load all skills
const skills = await registry.loadSkills();
// Returns: [{ name, description, skillFilePath, ... }, ...]

// Get specific skill
const skill = registry.getSkill('code-review');

// Load full instructions
const instructions = await registry.loadSkillInstructions('code-review');

// Check for errors
const errors = registry.getLoadErrors();
```

### Skill Tools

The AI uses these tools to interact with skills:

**listSkills**: List available skills

```xml
<listSkills>
<filter>security</filter>
</listSkills>
```

Response:
```json
{
  "skills": [
    { "name": "security-audit", "description": "Comprehensive security analysis" },
    { "name": "security-review", "description": "Quick security check" }
  ]
}
```

**useSkill**: Activate a skill

```xml
<useSkill>
<name>security-audit</name>
</useSkill>
```

Response includes full skill instructions.

---

## System Prompt Integration

When skills are enabled, the system prompt includes:

```xml
# Available Skills

<available_skills>
  <skill>
    <name>code-review</name>
    <description>Guides comprehensive code review</description>
  </skill>
  <skill>
    <name>security-audit</name>
    <description>Comprehensive security analysis</description>
  </skill>
</available_skills>

To use a skill, call the useSkill tool with its name.
```

---

## Writing Effective Skills

### Structure

```markdown
---
name: skill-name
description: One-sentence description
---

# Skill Title

## Overview
Brief explanation of the skill's purpose.

## When to Use
- Scenario 1
- Scenario 2

## Step-by-Step Approach
1. First step
2. Second step
3. Third step

## Output Format
How to present findings.

## Example
Real-world example.
```

### Best Practices

1. **Be Specific**: Provide clear, actionable steps
2. **Include Context**: Explain WHY each step matters
3. **Show Examples**: Include real code examples
4. **Enable Judgment**: Allow flexibility while providing structure
5. **Keep Focused**: One skill, one purpose

---

## Debugging Skills

### Enable Debug Logging

```javascript
const registry = new SkillRegistry({
  repoRoot: '/path/to/project',
  debug: true
});
```

### Check Loading Errors

```javascript
const errors = registry.getLoadErrors();
for (const error of errors) {
  console.log(`${error.path}: ${error.code} - ${error.message}`);
}
```

### Error Codes

| Code | Description |
|------|-------------|
| `read_failed` | File read error |
| `invalid_frontmatter` | Missing `---` delimiter |
| `invalid_yaml` | YAML parsing error |
| `invalid_name` | Name doesn't match rules |

---

## Security

### Path Validation
- Skills must be inside repository root
- No directory traversal allowed

### Symlink Detection
- Symlinked SKILL.md files are skipped

### Safe YAML Parsing
- Uses `failsafe` schema
- No code execution

---

## Related Documentation

- [Task Management](./tasks.md) - Track multi-step work
- [Tools Reference](../sdk/tools-reference.md) - Available tools
- [API Reference](../sdk/api-reference.md) - Configuration options
