# Edit and Create Tools

## Overview

The Probe Agent now supports file editing and creation capabilities through two new tools that follow the Claude Code pattern of exact string replacement. These tools are disabled by default and must be explicitly enabled using the `--allow-edit` flag.

## Tools

### Edit Tool

Performs exact string replacements in files. The tool requires the `old_string` to match exactly what's in the file, including all whitespace and indentation.

**Parameters:**
- `file_path`: Path to the file to edit (absolute or relative)
- `old_string`: Exact text to find and replace (must be unique unless `replace_all` is true)
- `new_string`: Text to replace with
- `replace_all`: (optional) Replace all occurrences instead of requiring uniqueness

**Example:**
```xml
<edit>
<file_path>src/main.js</file_path>
<old_string>function oldName() {
  return 42;
}</old_string>
<new_string>function newName() {
  return 42;
}</new_string>
</edit>
```

### Create Tool

Creates new files with specified content. Will create parent directories if they don't exist.

**Parameters:**
- `file_path`: Path where the file should be created (absolute or relative)
- `content`: Content to write to the file
- `overwrite`: (optional) Whether to overwrite if file exists (default: false)

**Example:**
```xml
<create>
<file_path>src/newFile.js</file_path>
<content>export function hello() {
  return "Hello, world!";
}</content>
</create>
```

## Usage

### Command Line

Enable edit and create tools with the `--allow-edit` flag:

```bash
# Enable file modification capabilities
probe agent --allow-edit "Create a function to calculate fibonacci numbers"

# Combine with other options
probe agent --allow-edit --path ./src "Fix the bug in the authentication module"

# With bash tools for comprehensive capabilities
probe agent --allow-edit --enable-bash "Set up a new React component with tests"
```

### Programmatic Usage

```javascript
import { ProbeAgent } from '@probelabs/probe';

const agent = new ProbeAgent({
  allowEdit: true,  // Enable edit and create tools
  allowedFolders: ['./src', './tests'],  // Restrict to specific folders
  // ... other options
});

// The tools will be available to the agent
await agent.runPrompt("Create a new utility function for data validation");
```

### Direct Tool Usage

```javascript
import { editTool, createTool } from '@probelabs/probe';

// Create tool instances
const edit = editTool({
  allowedFolders: ['./src'],
  debug: true
});

const create = createTool({
  allowedFolders: ['./src']
});

// Use the tools
await create.execute({
  file_path: 'src/utils.js',
  content: 'export const utils = {};'
});

await edit.execute({
  file_path: 'src/utils.js',
  old_string: 'export const utils = {};',
  new_string: 'export const utils = { version: "1.0.0" };'
});
```

## Security Features

### Allowed Folders
Both tools respect the `allowedFolders` configuration to restrict file operations to specific directories:

```javascript
const agent = new ProbeAgent({
  allowEdit: true,
  allowedFolders: ['/project/src', '/project/tests']
  // Files outside these folders cannot be edited or created
});
```

### Exact String Matching
The edit tool uses exact string matching (Claude Code style) which provides safety through:
- Prevention of unintended changes
- Clear visibility of what will be modified
- Automatic detection of conflicts (multiple occurrences)

### Default Disabled
File modification tools are disabled by default and must be explicitly enabled with `--allow-edit`.

## Error Handling

The tools provide detailed error messages for common issues:

- **File not found**: When trying to edit a non-existent file
- **String not found**: When the `old_string` doesn't exist in the file
- **Multiple occurrences**: When `old_string` appears multiple times without `replace_all`
- **Permission denied**: When trying to access files outside allowed folders
- **File already exists**: When creating a file that exists without `overwrite: true`

## Integration with Other Tools

The edit and create tools integrate seamlessly with other Probe tools:

```bash
# Search, then edit
probe agent --allow-edit "Find all TODO comments and convert them to proper documentation"

# Create files based on analysis
probe agent --allow-edit "Analyze the API endpoints and create TypeScript interfaces"

# Combine with bash for full development workflow
probe agent --allow-edit --enable-bash "Set up a new feature with tests and documentation"
```

## Best Practices

1. **Use with Search First**: Search for code patterns before editing
   ```
   1. Use `search` to find relevant files
   2. Use `extract` to see the full context
   3. Use `edit` to make targeted changes
   ```

2. **Preserve Formatting**: Copy exact indentation and whitespace when editing

3. **Use Unique Context**: Include enough surrounding code to ensure uniqueness

4. **Test Changes**: Combine with bash tools to run tests after modifications

5. **Review Before Applying**: The exact string matching shows precisely what will change

## Limitations

- **Exact Matching Required**: Whitespace, indentation, and formatting must match exactly
- **No Regex Support**: Unlike search tools, edit uses literal string matching
- **Single File Operations**: Each tool call operates on one file at a time
- **No Partial Line Edits**: Must include complete lines with proper line endings

## Testing

The implementation includes comprehensive test coverage:

```bash
# Run tests for edit and create tools
npm test -- tests/unit/edit-create-tools.test.js

# Test the tools directly
node examples/test-edit-create.js

# Test integration with ProbeAgent
node examples/test-edit-direct.js
```

## Migration from Other Tools

### From Aider/Implement Tool
The new edit and create tools provide more granular control compared to the implement tool:
- Implement tool: Delegates to external AI for modifications
- Edit/Create tools: Direct, deterministic file operations

### From Direct File System Access
These tools provide a safer alternative to unrestricted file system access:
- Controlled through `allowedFolders`
- Clear operation logging
- Integrated with the agent's workflow

## Troubleshooting

### "String not found in file"
- Verify the exact whitespace and indentation
- Use the `extract` tool to see the exact file content
- Copy the string directly from the file content

### "Multiple occurrences found"
- Add more context to make the string unique
- Or use `replace_all: true` to replace all occurrences

### "Permission denied"
- Check that the file path is within `allowedFolders`
- Use absolute paths to avoid confusion
- Verify the folder permissions

## Future Enhancements

Potential improvements being considered:
- [ ] Bulk edit operations across multiple files
- [ ] Pattern-based replacements (with safety checks)
- [ ] Automatic formatting preservation
- [ ] Integration with git for change tracking
- [ ] Dry-run mode for previewing changes