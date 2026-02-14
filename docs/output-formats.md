# Output Formats Reference

Complete reference documentation for Probe's output formats, including JSON and XML schemas for all commands. This reference covers the structure and usage of each format for the `search`, `query`, and `extract` commands.

## Common Format Options

All three commands (`search`, `query`, and `extract`) support the following output formats:

| Format | Description |
|--------|-------------|
| `color` | Colorized terminal output (default) |
| `terminal` | Plain terminal output without colors |
| `markdown` | Markdown-formatted output |
| `plain` | Plain text output without formatting |
| `json` | JSON-formatted output |
| `xml` | XML-formatted output |

To specify an output format, use the `--format` or `-o` option:

```bash
probe search "query" --format json
probe query "pattern" ./src --format xml
probe extract file.rs:42 --format markdown
```

## JSON Output Format

The JSON output format provides a structured representation of search results that's easy to parse programmatically. This is particularly useful for integrating Probe with other tools or scripts.

### Common JSON Structure

All three commands share a common top-level JSON structure:

```json
{
  "results": [
    // Array of result objects
  ],
  "summary": {
    "count": 5,           // Number of results
    "total_bytes": 1024,  // Total bytes of code in results
    "total_tokens": 256   // Total tokens in results (for AI usage)
  }
}
```

### Search Command JSON Format

The `search` command's JSON output includes the following fields for each result:

```json
{
  "results": [
    {
      "file": "/path/to/file.rs",       // File path
      "lines": [10, 20],                // Start and end line numbers
      "node_type": "function",          // Type of code block (function, class, struct, etc.)
      "code": "fn example() { ... }",   // The actual code content
      "matched_keywords": [             // Keywords that matched (if available)
        "example",
        "function"
      ],
      "rank": 1,                        // Rank in search results (if available)
      "score": 0.95                     // Relevance score (if available)
    },
    // More results...
  ],
  "summary": {
    "count": 5,
    "total_bytes": 1024,
    "total_tokens": 256
  }
}
```

#### Example: Search JSON Output

```bash
probe search "authentication" --format json
```

```json
{
  "results": [
    {
      "file": "/path/to/auth.rs",
      "lines": [15, 30],
      "node_type": "function",
      "code": "fn authenticate_user(username: &str, password: &str) -> Result<User, AuthError> {\n    // Authentication logic\n    ...\n}",
      "matched_keywords": ["authenticate", "user"],
      "rank": 1,
      "score": 0.98
    }
  ],
  "summary": {
    "count": 1,
    "total_bytes": 120,
    "total_tokens": 30
  }
}
```

### Query Command JSON Format

The `query` command's JSON output is similar to the search command but includes additional fields specific to AST-based queries:

```json
{
  "results": [
    {
      "file": "/path/to/file.rs",       // File path
      "lines": [10, 20],                // Start and end line numbers
      "node_type": "function",          // Type of code block (function, class, struct, etc.)
      "code": "fn example() { ... }",   // The actual code content
      "column_start": 0,                // Starting column
      "column_end": 20                  // Ending column
    },
    // More results...
  ],
  "summary": {
    "count": 5,
    "total_bytes": 1024,
    "total_tokens": 256
  }
}
```

#### Example: Query JSON Output

```bash
probe query "fn $NAME($$$PARAMS) $$$BODY" ./src --language rust --format json
```

```json
{
  "results": [
    {
      "file": "/path/to/functions.rs",
      "lines": [5, 8],
      "node_type": "function",
      "code": "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}",
      "column_start": 0,
      "column_end": 35
    }
  ],
  "summary": {
    "count": 1,
    "total_bytes": 35,
    "total_tokens": 15
  }
}
```

### Extract Command JSON Format

The `extract` command's JSON output is similar to the other commands but may include different `node_type` values depending on the extraction method:

```json
{
  "results": [
    {
      "file": "/path/to/file.rs",       // File path
      "lines": [10, 20],                // Start and end line numbers
      "node_type": "function",          // Type: "function", "struct", "file", "range", "context"
      "code": "fn example() { ... }"    // The extracted code content
    },
    // More results...
  ],
  "summary": {
    "count": 1,
    "total_bytes": 1024,
    "total_tokens": 256
  }
}
```

When using the `--keep-input` flag, the JSON output includes an additional `original_input` field containing the exact input text:

```json
{
  "original_input": "src/main.rs:42: error: invalid syntax",  // Original input when using --keep-input
  "results": [
    {
      "file": "src/main.rs",
      "lines": [40, 45],
      "node_type": "function",
      "code": "fn process_data(data: &[u8]) -> Result<Vec<u8>, Error> {\n    // Processing logic\n    ...\n}"
    }
  ],
  "summary": {
    "count": 1,
    "total_bytes": 85,
    "total_tokens": 25
  }
}
```

#### Example: Extract JSON Output

```bash
probe extract src/main.rs:42 --format json
```

```json
{
  "results": [
    {
      "file": "src/main.rs",
      "lines": [40, 45],
      "node_type": "function",
      "code": "fn process_data(data: &[u8]) -> Result<Vec<u8>, Error> {\n    // Processing logic\n    ...\n}"
    }
  ],
  "summary": {
    "count": 1,
    "total_bytes": 85,
    "total_tokens": 25
  }
}
```

## XML Output Format

The XML output format provides a structured representation similar to JSON but in XML format. This can be useful for tools that prefer XML over JSON.

### Common XML Structure

All three commands share a common XML structure:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<probe_results>
  <result>
    <!-- Result elements -->
  </result>
  <!-- More results... -->
  <summary>
    <count>5</count>
    <total_bytes>1024</total_bytes>
    <total_tokens>256</total_tokens>
  </summary>
</probe_results>
```

### Search Command XML Format

The `search` command's XML output includes the following elements for each result:

```xml
<probe_results>
  <result>
    <file>/path/to/file.rs</file>
    <lines>
      <start>10</start>
      <end>20</end>
    </lines>
    <node_type>function</node_type>
    <code><![CDATA[fn example() { ... }]]></code>
    <matched_keywords>
      <keyword>example</keyword>
      <keyword>function</keyword>
    </matched_keywords>
    <rank>1</rank>
    <score>0.95</score>
  </result>
  <!-- More results... -->
  <summary>
    <count>5</count>
    <total_bytes>1024</total_bytes>
    <total_tokens>256</total_tokens>
  </summary>
</probe_results>
```

#### Example: Search XML Output

```bash
probe search "authentication" --format xml
```

```xml
<?xml version="1.0" encoding="UTF-8"?>
<probe_results>
  <result>
    <file>/path/to/auth.rs</file>
    <lines>
      <start>15</start>
      <end>30</end>
    </lines>
    <node_type>function</node_type>
    <code><![CDATA[fn authenticate_user(username: &str, password: &str) -> Result<User, AuthError> {
    // Authentication logic
    ...
}]]></code>
    <matched_keywords>
      <keyword>authenticate</keyword>
      <keyword>user</keyword>
    </matched_keywords>
    <rank>1</rank>
    <score>0.98</score>
  </result>
  <summary>
    <count>1</count>
    <total_bytes>120</total_bytes>
    <total_tokens>30</total_tokens>
  </summary>
</probe_results>
```

### Query Command XML Format

The `query` command's XML output is similar to the search command but includes additional elements specific to AST-based queries:

```xml
<probe_results>
  <result>
    <file>/path/to/file.rs</file>
    <lines>
      <start>10</start>
      <end>20</end>
    </lines>
    <node_type>function</node_type>
    <column_start>0</column_start>
    <column_end>20</column_end>
    <code><![CDATA[fn example() { ... }]]></code>
  </result>
  <!-- More results... -->
  <summary>
    <count>5</count>
    <total_bytes>1024</total_bytes>
    <total_tokens>256</total_tokens>
  </summary>
</probe_results>
```

#### Example: Query XML Output

```bash
probe query "fn $NAME($$$PARAMS) $$$BODY" ./src --language rust --format xml
```

```xml
<?xml version="1.0" encoding="UTF-8"?>
<probe_results>
  <result>
    <file>/path/to/functions.rs</file>
    <lines>
      <start>5</start>
      <end>8</end>
    </lines>
    <node_type>function</node_type>
    <column_start>0</column_start>
    <column_end>35</column_end>
    <code><![CDATA[fn add(a: i32, b: i32) -> i32 {
    a + b
}]]></code>
  </result>
  <summary>
    <count>1</count>
    <total_bytes>35</total_bytes>
    <total_tokens>15</total_tokens>
  </summary>
</probe_results>
```

### Extract Command XML Format

The `extract` command's XML output is similar to the other commands but may include different `node_type` values depending on the extraction method:

```xml
<probe_results>
  <result>
    <file>/path/to/file.rs</file>
    <lines>
      <start>10</start>
      <end>20</end>
    </lines>
    <node_type>function</node_type>
    <code><![CDATA[fn example() { ... }]]></code>
  </result>
  <!-- More results... -->
  <summary>
    <count>1</count>
    <total_bytes>1024</total_bytes>
    <total_tokens>256</total_tokens>
  </summary>
</probe_results>
```

When using the `--keep-input` flag, the XML output includes an additional `original_input` element containing the exact input text:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<probe_results>
  <original_input><![CDATA[src/main.rs:42: error: invalid syntax]]></original_input>
  <result>
    <file>src/main.rs</file>
    <lines>
      <start>40</start>
      <end>45</end>
    </lines>
    <node_type>function</node_type>
    <code><![CDATA[fn process_data(data: &[u8]) -> Result<Vec<u8>, Error> {
    // Processing logic
    ...
}]]></code>
  </result>
  <summary>
    <count>1</count>
    <total_bytes>85</total_bytes>
    <total_tokens>25</total_tokens>
  </summary>
</probe_results>
```

#### Example: Extract XML Output

```bash
probe extract src/main.rs:42 --format xml

# With --keep-input flag to preserve original input
probe extract src/main.rs:42 --format xml --keep-input
```

```xml
<?xml version="1.0" encoding="UTF-8"?>
<probe_results>
  <result>
    <file>src/main.rs</file>
    <lines>
      <start>40</start>
      <end>45</end>
    </lines>
    <node_type>function</node_type>
    <code><![CDATA[fn process_data(data: &[u8]) -> Result<Vec<u8>, Error> {
    // Processing logic
    ...
}]]></code>
  </result>
  <summary>
    <count>1</count>
    <total_bytes>85</total_bytes>
    <total_tokens>25</total_tokens>
  </summary>
</probe_results>
```

## Parsing and Using the Output

### Parsing JSON Output

You can easily parse the JSON output using any JSON parser. Here are some examples:

#### Python Example

```python
import json
import subprocess

# Run probe search and capture output
output = subprocess.check_output(["probe", "search", "authentication", "--format", "json"])
results = json.loads(output)

# Process the results
# Check if original input is present (when using --keep-input)
if "original_input" in results:
    print(f"Original Input:\n{results['original_input']}")
    print("---")

for result in results["results"]:
    print(f"File: {result['file']}")
    print(f"Lines: {result['lines'][0]}-{result['lines'][1]}")
    print(f"Code:\n{result['code']}")
    print("---")

# Get summary information
print(f"Found {results['summary']['count']} results")
print(f"Total bytes: {results['summary']['total_bytes']}")
print(f"Total tokens: {results['summary']['total_tokens']}")
```

#### JavaScript/Node.js Example

```javascript
const { execSync } = require('child_process');

// Run probe search and capture output
const output = execSync('probe search "authentication" --format json', { encoding: 'utf-8' });
const results = JSON.parse(output);

// Process the results
// Check if original input is present (when using --keep-input)
if (results.original_input) {
  console.log(`Original Input:\n${results.original_input}`);
  console.log('---');
}

results.results.forEach(result => {
  console.log(`File: ${result.file}`);
  console.log(`Lines: ${result.lines[0]}-${result.lines[1]}`);
  console.log(`Code:\n${result.code}`);
  console.log('---');
});

// Get summary information
console.log(`Found ${results.summary.count} results`);
console.log(`Total bytes: ${results.summary.total_bytes}`);
console.log(`Total tokens: ${results.summary.total_tokens}`);
```

### Parsing XML Output

You can parse the XML output using any XML parser. Here are some examples:

#### Python Example

```python
import subprocess
import xml.etree.ElementTree as ET

# Run probe search and capture output
output = subprocess.check_output(["probe", "search", "authentication", "--format", "xml"])
root = ET.fromstring(output)

# Process the results
# Check if original input is present (when using --keep-input)
original_input = root.find('original_input')
if original_input is not None:
    print(f"Original Input:\n{original_input.text}")
    print("---")

for result in root.findall('./result'):
    file = result.find('file').text
    lines_start = result.find('./lines/start').text
    lines_end = result.find('./lines/end').text
    code = result.find('code').text
    
    print(f"File: {file}")
    print(f"Lines: {lines_start}-{lines_end}")
    print(f"Code:\n{code}")
    print("---")

# Get summary information
summary = root.find('summary')
count = summary.find('count').text
total_bytes = summary.find('total_bytes').text
total_tokens = summary.find('total_tokens').text

print(f"Found {count} results")
print(f"Total bytes: {total_bytes}")
print(f"Total tokens: {total_tokens}")
```

#### JavaScript/Node.js Example

```javascript
const { execSync } = require('child_process');
const { parseString } = require('xml2js');

// Run probe search and capture output
const output = execSync('probe search "authentication" --format xml', { encoding: 'utf-8' });

// Parse XML
parseString(output, (err, result) => {
  if (err) {
    console.error('Error parsing XML:', err);
    return;
  }
  
  // Process the results
  // Check if original input is present (when using --keep-input)
  if (result.probe_results.original_input) {
    console.log(`Original Input:\n${result.probe_results.original_input[0]}`);
    console.log('---');
  }
  
  const results = result.probe_results.result || [];
  results.forEach(result => {
    const file = result.file[0];
    const linesStart = result.lines[0].start[0];
    const linesEnd = result.lines[0].end[0];
    const code = result.code[0];
    
    console.log(`File: ${file}`);
    console.log(`Lines: ${linesStart}-${linesEnd}`);
    console.log(`Code:\n${code}`);
    console.log('---');
  });
  
  // Get summary information
  const summary = result.probe_results.summary[0];
  console.log(`Found ${summary.count[0]} results`);
  console.log(`Total bytes: ${summary.total_bytes[0]}`);
  console.log(`Total tokens: ${summary.total_tokens[0]}`);
});
```

## Format Differences Between Commands

While the overall structure is similar, there are some differences in the output formats between commands:

1. **Search Command**:
   - Includes ranking and scoring information
   - May include matched keywords
   - Node types are typically language constructs (function, class, etc.)

2. **Query Command**:
   - Includes column start and end positions
   - Results are based on AST patterns rather than text search
   - More precise structural matching

3. **Extract Command**:
   - Node types can be "file", "function", "range", or "context"
   - Focused on extracting specific code blocks rather than searching
   - May include context lines around the extracted code
   - Can include original input text when using `--keep-input` flag

## Special Character Handling

Both JSON and XML formats handle special characters appropriately:

- **JSON**: Special characters in code are properly escaped according to JSON rules
- **XML**: Code blocks are wrapped in CDATA sections to preserve formatting and special characters

This ensures that the output can be reliably parsed regardless of the content of the code.