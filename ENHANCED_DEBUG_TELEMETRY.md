# Enhanced Debug Output and Telemetry Implementation

This document outlines the comprehensive enhancements made to the probe agent's debug output and telemetry systems.

## Overview

Enhanced the probe agent (npm folder) with comprehensive debug output and telemetry integration covering delegation, JSON validation, and Mermaid diagram validation processes.

## Key Enhancements

### 1. Delegation Events Debug Output (`npm/src/delegate.js`)

**Enhanced Debugging:**
- Session ID tracking with detailed lifecycle logging
- Process spawn monitoring with command arguments logging
- Real-time stdout/stderr chunk monitoring with size tracking
- Comprehensive timeout handling with partial output capture
- Duration tracking for all operations
- Detailed error reporting with error type classification

**Telemetry Integration:**
- Delegation spans with task metadata and performance metrics
- Event recording for delegation lifecycle (started, completed, failed, timeout, spawn_error)
- Session correlation and iteration tracking
- Response metrics and success/failure attribution

### 2. JSON Validation Debug Output (`npm/src/agent/schemaUtils.js`)

**Enhanced Debugging:**
- Parse performance timing with millisecond precision
- Detailed error position reporting and common issue classification
- Schema definition detection with indicator analysis
- Multi-attempt correction tracking with response previews
- Comprehensive validation pipeline monitoring

**Telemetry Integration:**
- JSON validation events (started, completed) with detailed metadata
- Retry count tracking and error attribution
- Response length monitoring and schema type detection
- Success/failure metrics with error context

### 3. Mermaid Validation Debug Output (`npm/src/agent/schemaUtils.js`)

**Enhanced Debugging:**
- Initial validation timing with diagram enumeration
- HTML entity auto-fix tracking with before/after comparison
- AI fixing performance metrics with per-diagram timing
- Token usage reporting for AI operations
- Comprehensive fixing result attribution (HTML vs AI methods)

**Telemetry Integration:**
- Mermaid validation events with context tracking
- Performance metrics (total time, AI fixing time, validation time)
- Diagram processing statistics (found, fixed, methods used)
- Token usage tracking for cost analysis

### 4. ProbeAgent Integration (`npm/src/agent/ProbeAgent.js`)

**Enhanced Integration:**
- Tracer propagation to all validation subsystems
- Context-aware debug logging with operation classification
- Performance metrics integration across validation pipeline
- Structured event recording for complex operations

### 5. Telemetry Infrastructure (`npm/src/agent/appTracer.js`)

**New Telemetry Methods:**
- `createDelegationSpan()` - Spans for delegation operations
- `createJsonValidationSpan()` - Spans for JSON validation
- `createMermaidValidationSpan()` - Spans for Mermaid validation
- `recordDelegationEvent()` - Event recording for delegation
- `recordJsonValidationEvent()` - Event recording for JSON validation
- `recordMermaidValidationEvent()` - Event recording for Mermaid validation

**Enhanced Capabilities:**
- Structured event logging with session correlation
- Attribute management for active spans
- Fallback logging for environments without active spans

## Debug Output Examples

### Delegation Debug Output
```
[DELEGATE] Starting delegation session abc123-def456
[DELEGATE] Task: Analyze authentication system for security vulnerabilities
[DELEGATE] Current iteration: 2/10
[DELEGATE] Remaining iterations for subagent: 8
[DELEGATE] Timeout configured: 300 seconds
[DELEGATE] Using clean agent environment with code-researcher prompt
[DELEGATE] Using binary at: /path/to/probe
[DELEGATE] Command args: agent --task "..." --session-id abc123-def456 --prompt-type code-researcher
[DELEGATE] stdout chunk received (156 chars): Analysis complete. Found 3 potential vulnerabilities...
[DELEGATE] Process completed with code 0 in 45.67s
[DELEGATE] Duration: 45.67s
[DELEGATE] Total stdout: 1,247 chars
[DELEGATE] Task completed successfully for session abc123-def456
[DELEGATE] Response length: 1,247 chars
```

### JSON Validation Debug Output
```
[DEBUG] JSON validation: Starting validation process for schema response
[DEBUG] JSON validation: Response length: 342 chars
[DEBUG] JSON validation: Starting validation for response (342 chars)
[DEBUG] JSON validation: Preview: {"analysis": {"vulnerabilities": [{"type": "sql_injection"...
[DEBUG] JSON validation: Successfully parsed in 2ms
[DEBUG] JSON validation: Object type: object, keys: 3
[DEBUG] JSON validation: Final validation successful
```

### Mermaid Validation Debug Output
```
[DEBUG] Mermaid validation: Starting enhanced validation for response (1856 chars)
[DEBUG] Mermaid validation: Initial validation completed in 15ms
[DEBUG] Mermaid validation: Found 2 diagrams, valid: false
[DEBUG] Mermaid validation: Diagram 1: invalid (flowchart)
[DEBUG] Mermaid validation: Error for diagram 1: got PS error in parser
[DEBUG] Mermaid validation: 1 invalid diagrams detected, trying HTML entity auto-fix first...
[DEBUG] Mermaid validation: Fixed diagram 1 with HTML entity decoding
[DEBUG] Mermaid validation: Original error: got PS error in parser
[DEBUG] Mermaid validation: Decoded 3 HTML entities
[DEBUG] Mermaid validation: All diagrams fixed with HTML entity decoding in 127ms, no AI needed
```

## Telemetry Events

### Delegation Events
- `delegation.started` - Task delegation initiated
- `delegation.completed` - Task completed successfully
- `delegation.failed` - Task failed with exit code
- `delegation.timeout` - Task exceeded timeout
- `delegation.spawn_error` - Process spawn failed

### JSON Validation Events
- `json_validation.started` - Validation process started
- `json_validation.completed` - Validation completed with results

### Mermaid Validation Events
- `mermaid_validation.started` - Validation process started
- `mermaid_validation.html_fix_completed` - HTML entity fixes successful
- `mermaid_validation.completed` - Full validation completed

## Configuration

### Enabling Debug Output
Set `debug: true` in ProbeAgent options or use `DEBUG=1` environment variable:

```javascript
const agent = new ProbeAgent({
  debug: true,
  tracer: myTracer
});
```

### Telemetry Integration
Pass tracer instance to ProbeAgent:

```javascript
import { AppTracer } from './appTracer.js';

const tracer = new AppTracer(telemetryConfig);
const agent = new ProbeAgent({
  tracer: tracer
});
```

## Performance Impact

- Debug logging: Minimal overhead when disabled
- Telemetry: Structured events with configurable verbosity
- Timing: Microsecond precision for performance analysis
- Memory: Efficient string handling with truncation for large responses

## Benefits

1. **Enhanced Troubleshooting:** Detailed debug output for all validation processes
2. **Performance Monitoring:** Comprehensive timing and metrics collection
3. **Cost Analysis:** Token usage tracking for AI operations
4. **Process Visibility:** Complete delegation lifecycle tracking
5. **Quality Metrics:** Success/failure rates and error attribution
6. **Observability:** Structured telemetry for monitoring systems

## Future Enhancements

- Integration with OpenTelemetry collectors
- Dashboard visualization for telemetry data
- Automated performance regression detection
- Enhanced error categorization and remediation suggestions