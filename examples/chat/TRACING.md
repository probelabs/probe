# Tracing Support for Probe Chat

This document describes the tracing functionality available in the Probe Chat application.

## Overview

Probe Chat now supports OpenTelemetry tracing for AI model calls, allowing you to:
- Track AI model interactions and performance
- Export traces to files for analysis
- Send traces to remote OpenTelemetry collectors
- Debug AI model calls with console output

## Configuration

### CLI Options

You can enable tracing using the following command-line options:

```bash
# Enable file tracing (default: ./traces.jsonl)
node index.js --trace-file

# Enable file tracing with custom path
node index.js --trace-file ./my-traces.jsonl

# Enable remote tracing (default: http://localhost:4318/v1/traces)
node index.js --trace-remote

# Enable remote tracing with custom endpoint
node index.js --trace-remote http://my-collector:4318/v1/traces

# Enable console tracing for debugging
node index.js --trace-console

# Enable multiple exporters
node index.js --trace-file --trace-remote --trace-console
```

### Environment Variables

You can also configure tracing using environment variables:

```bash
# Enable file tracing
export OTEL_ENABLE_FILE=true
export OTEL_FILE_PATH=./traces.jsonl

# Enable remote tracing
export OTEL_ENABLE_REMOTE=true
export OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://localhost:4318/v1/traces

# Enable console tracing
export OTEL_ENABLE_CONSOLE=true

# Service information
export OTEL_SERVICE_NAME=probe-chat
export OTEL_SERVICE_VERSION=1.0.0
```

## Trace Data

The tracing system captures the following information:

### Span Attributes
- **Service Information**: Service name, version
- **Session Information**: Session ID, iteration number
- **Model Information**: Model name, API type (anthropic, openai, google)
- **Configuration**: Allow edit flag, prompt type
- **Function ID**: Unique identifier for each chat function

### Trace Content
- AI model requests and responses
- Token usage statistics
- Tool call information
- Error details when they occur

## File Format

When using file tracing, traces are saved in JSON Lines format (`.jsonl`), where each line contains a complete trace span:

```json
{
  "traceId": "abcd1234...",
  "spanId": "efgh5678...",
  "name": "ai.generateText",
  "startTimeUnixNano": 1704067200000000000,
  "endTimeUnixNano": 1704067201000000000,
  "attributes": {
    "sessionId": "session-123",
    "iteration": "1",
    "model": "claude-3-7-sonnet-20250219",
    "apiType": "anthropic"
  },
  "status": { "code": "OK" },
  "events": [],
  "links": [],
  "resource": {
    "attributes": {
      "service.name": "probe-chat",
      "service.version": "1.0.0"
    }
  }
}
```

## Remote Tracing

For remote tracing, you can use any OpenTelemetry-compatible collector:

### Using OpenTelemetry Collector

1. Install the OpenTelemetry Collector
2. Configure it with a `receiver` for OTLP HTTP
3. Configure exporters for your preferred backend (Jaeger, Zipkin, etc.)

Example collector config:

```yaml
receivers:
  otlp:
    protocols:
      http:
        endpoint: 0.0.0.0:4318

exporters:
  jaeger:
    endpoint: jaeger:14250
    tls:
      insecure: true

service:
  pipelines:
    traces:
      receivers: [otlp]
      exporters: [jaeger]
```

### Using Jaeger

You can send traces directly to Jaeger using the OTLP endpoint:

```bash
# Start Jaeger
docker run -d --name jaeger \
  -p 16686:16686 \
  -p 14250:14250 \
  -p 14268:14268 \
  -p 4317:4317 \
  -p 4318:4318 \
  jaegertracing/all-in-one:latest

# Run probe-chat with remote tracing
node index.js --trace-remote http://localhost:4318/v1/traces
```

## Example Usage

### Basic File Tracing

```bash
# Enable file tracing and run a query
node index.js --trace-file --message "What files are in this project?"

# Check the generated traces
cat traces.jsonl | jq '.'
```

### Remote Tracing with Jaeger

```bash
# Start Jaeger
docker run -d --name jaeger -p 16686:16686 -p 4318:4318 jaegertracing/all-in-one:latest

# Run probe-chat with remote tracing
node index.js --trace-remote --message "Analyze the main function"

# View traces at http://localhost:16686
```

### Combined Tracing

```bash
# Enable both file and remote tracing
node index.js --trace-file ./debug-traces.jsonl --trace-remote http://localhost:4318/v1/traces --trace-console

# This will:
# 1. Save traces to ./debug-traces.jsonl
# 2. Send traces to the remote collector
# 3. Print traces to console for debugging
```

## Performance Considerations

- **File Tracing**: Minimal overhead, suitable for production
- **Remote Tracing**: Slight network overhead, depends on collector performance
- **Console Tracing**: Higher overhead, recommended for debugging only

## Integration with Vercel AI SDK

The tracing system integrates with the Vercel AI SDK's experimental telemetry feature. When tracing is enabled, the SDK automatically:

- Creates spans for each AI model call
- Captures request/response data
- Records token usage information
- Tracks tool calls and their results

## Troubleshooting

### Common Issues

1. **File permissions**: Ensure write permissions for the trace file directory
2. **Remote collector unavailable**: Check network connectivity and collector status
3. **Large trace files**: Consider log rotation for long-running sessions

### Debug Mode

Enable debug mode to see tracing initialization messages:

```bash
node index.js --debug --trace-file
```

This will show:
- Telemetry initialization status
- Exporter configuration
- Any errors during setup