/**
 * Utility functions for cleaning and validating schema responses from AI models
 * Supports JSON and Mermaid diagram validation
 */

import { createMessagePreview } from '../tools/common.js';

/**
 * HTML entity decoder map for common entities that might appear in mermaid diagrams
 */
const HTML_ENTITY_MAP = {
  '&lt;': '<',
  '&gt;': '>',
  '&amp;': '&',
  '&quot;': '"',
  '&#39;': "'",
  '&apos;': "'",  // Also handle XML/HTML5 apostrophe entity
  '&nbsp;': ' '
};

/**
 * Decode HTML entities in text without requiring external dependencies
 * @param {string} text - Text that may contain HTML entities
 * @returns {string} - Text with HTML entities decoded
 */
export function decodeHtmlEntities(text) {
  if (!text || typeof text !== 'string') {
    return text;
  }

  let decoded = text;
  for (const [entity, character] of Object.entries(HTML_ENTITY_MAP)) {
    // Use global replacement to catch all instances
    decoded = decoded.replace(new RegExp(entity, 'g'), character);
  }

  return decoded;
}

/**
 * Clean AI response by extracting JSON content when response contains JSON
 * Only processes responses that contain JSON structures { or [ 
 * @param {string} response - Raw AI response
 * @returns {string} - Cleaned response with JSON boundaries extracted if applicable
 */
export function cleanSchemaResponse(response) {
  if (!response || typeof response !== 'string') {
    return response;
  }

  const trimmed = response.trim();
  
  // First, look for JSON after code block markers
  const codeBlockPatterns = [
    /```json\s*\n?([{\[][\s\S]*?[}\]])\s*\n?```/,
    /```\s*\n?([{\[][\s\S]*?[}\]])\s*\n?```/,
    /`([{\[][\s\S]*?[}\]])`/
  ];
  
  for (const pattern of codeBlockPatterns) {
    const match = trimmed.match(pattern);
    if (match) {
      return match[1].trim();
    }
  }
  
  // Look for code block start followed immediately by JSON
  const codeBlockStartPattern = /```(?:json)?\s*\n?\s*([{\[])/;
  const codeBlockMatch = trimmed.match(codeBlockStartPattern);
  
  if (codeBlockMatch) {
    const startIndex = codeBlockMatch.index + codeBlockMatch[0].length - 1; // Position of the bracket
    
    // Find the matching closing bracket
    const openChar = codeBlockMatch[1];
    const closeChar = openChar === '{' ? '}' : ']';
    let bracketCount = 1;
    let endIndex = startIndex + 1;
    
    while (endIndex < trimmed.length && bracketCount > 0) {
      const char = trimmed[endIndex];
      if (char === openChar) {
        bracketCount++;
      } else if (char === closeChar) {
        bracketCount--;
      }
      endIndex++;
    }
    
    if (bracketCount === 0) {
      return trimmed.substring(startIndex, endIndex);
    }
  }
  
  // Fallback: Find JSON boundaries anywhere in the text
  const firstBracket = Math.min(
    trimmed.indexOf('{') >= 0 ? trimmed.indexOf('{') : Infinity,
    trimmed.indexOf('[') >= 0 ? trimmed.indexOf('[') : Infinity
  );
  
  const lastBracket = Math.max(
    trimmed.lastIndexOf('}'),
    trimmed.lastIndexOf(']')
  );
  
  // Only extract if we found valid JSON boundaries
  if (firstBracket < Infinity && lastBracket >= 0 && firstBracket < lastBracket) {
    // Check if the response likely starts with JSON (directly or after minimal content)
    const beforeFirstBracket = trimmed.substring(0, firstBracket).trim();
    
    // If there's minimal content before the first bracket, extract the JSON
    if (beforeFirstBracket === '' || 
        beforeFirstBracket.match(/^```\w*$/) ||
        beforeFirstBracket.split('\n').length <= 2) {
      return trimmed.substring(firstBracket, lastBracket + 1);
    }
  }
  
  return response; // Return original if no extractable JSON found
}

/**
 * Validate that the cleaned response is valid JSON if expected
 * @param {string} response - Cleaned response
 * @param {Object} options - Options for validation
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {Object} - {isValid: boolean, parsed?: Object, error?: string}
 */
export function validateJsonResponse(response, options = {}) {
  const { debug = false } = options;
  
  if (debug) {
    console.log(`[DEBUG] JSON validation: Starting validation for response (${response.length} chars)`);
    const preview = createMessagePreview(response);
    console.log(`[DEBUG] JSON validation: Preview: ${preview}`);
  }
  
  try {
    const parseStart = Date.now();
    const parsed = JSON.parse(response);
    const parseTime = Date.now() - parseStart;
    
    if (debug) {
      console.log(`[DEBUG] JSON validation: Successfully parsed in ${parseTime}ms`);
      console.log(`[DEBUG] JSON validation: Object type: ${typeof parsed}, keys: ${Object.keys(parsed || {}).length}`);
    }
    
    return { isValid: true, parsed };
  } catch (error) {
    if (debug) {
      console.log(`[DEBUG] JSON validation: Parse failed with error: ${error.message}`);
      console.log(`[DEBUG] JSON validation: Error at position: ${error.message.match(/position (\d+)/) ? error.message.match(/position (\d+)/)[1] : 'unknown'}`);
      
      // Try to identify common JSON issues
      if (error.message.includes('Unexpected token')) {
        console.log(`[DEBUG] JSON validation: Likely syntax error - unexpected character`);
      } else if (error.message.includes('Unexpected end')) {
        console.log(`[DEBUG] JSON validation: Likely incomplete JSON - missing closing brackets`);
      } else if (error.message.includes('property name')) {
        console.log(`[DEBUG] JSON validation: Likely unquoted property names`);
      }
    }
    
    return { isValid: false, error: error.message };
  }
}

/**
 * Validate that the cleaned response is valid XML if expected
 * @param {string} response - Cleaned response
 * @returns {Object} - {isValid: boolean, error?: string}
 */
export function validateXmlResponse(response) {
  // Basic XML validation - check for matching opening/closing tags
  const xmlPattern = /<\/?[\w\s="'.-]+>/g;
  const tags = response.match(xmlPattern);
  
  if (!tags) {
    return { isValid: false, error: 'No XML tags found' };
  }

  // Simple check for basic XML structure
  if (response.includes('<') && response.includes('>')) {
    return { isValid: true };
  }

  return { isValid: false, error: 'Invalid XML structure' };
}

/**
 * Process schema response with cleaning and optional validation
 * @param {string} response - Raw AI response
 * @param {string} schema - Original schema for context
 * @param {Object} options - Processing options
 * @returns {Object} - {cleaned: string, validation?: Object}
 */
export function processSchemaResponse(response, schema, options = {}) {
  const { validateJson = false, validateXml = false, debug = false } = options;

  if (debug) {
    console.log(`[DEBUG] Schema processing: Starting with response length ${response.length}`);
    console.log(`[DEBUG] Schema processing: Schema type detection...`);
    
    if (isJsonSchema(schema)) {
      console.log(`[DEBUG] Schema processing: Detected JSON schema`);
    } else {
      console.log(`[DEBUG] Schema processing: Non-JSON schema detected`);
    }
  }

  // Clean the response
  const cleanStart = Date.now();
  const cleaned = cleanSchemaResponse(response);
  const cleanTime = Date.now() - cleanStart;

  const result = { cleaned };

  if (debug) {
    console.log(`[DEBUG] Schema processing: Cleaning completed in ${cleanTime}ms`);
    result.debug = {
      originalLength: response.length,
      cleanedLength: cleaned.length,
      wasModified: response !== cleaned,
      cleaningTimeMs: cleanTime,
      removedContent: response !== cleaned ? {
        before: response.substring(0, 100) + (response.length > 100 ? '...' : ''),
        after: cleaned.substring(0, 100) + (cleaned.length > 100 ? '...' : '')
      } : null
    };
    
    if (response !== cleaned) {
      console.log(`[DEBUG] Schema processing: Response was modified during cleaning`);
      console.log(`[DEBUG] Schema processing: Original length: ${response.length}, cleaned length: ${cleaned.length}`);
    } else {
      console.log(`[DEBUG] Schema processing: Response unchanged during cleaning`);
    }
  }

  // Optional validation
  if (validateJson) {
    if (debug) {
      console.log(`[DEBUG] Schema processing: Running JSON validation...`);
    }
    result.jsonValidation = validateJsonResponse(cleaned, { debug });
  }

  if (validateXml) {
    if (debug) {
      console.log(`[DEBUG] Schema processing: Running XML validation...`);
    }
    result.xmlValidation = validateXmlResponse(cleaned);
  }

  return result;
}

/**
 * Detect if a schema expects JSON output
 * @param {string} schema - The schema string
 * @returns {boolean} - True if schema appears to be JSON-based
 */
export function isJsonSchema(schema) {
  if (!schema || typeof schema !== 'string') {
    return false;
  }

  const trimmedSchema = schema.trim().toLowerCase();
  
  // Check for JSON-like patterns
  const jsonIndicators = [
    trimmedSchema.startsWith('{') && trimmedSchema.includes('}'),
    trimmedSchema.startsWith('[') && trimmedSchema.includes(']'),
    trimmedSchema.includes('"type"') && trimmedSchema.includes('object'),
    trimmedSchema.includes('"properties"'),
    trimmedSchema.includes('json'),
    trimmedSchema.includes('application/json')
  ];

  // Return true if any JSON indicators are found
  return jsonIndicators.some(indicator => indicator);
}

/**
 * Detect if a JSON response is actually a JSON schema definition instead of data
 * @param {string} jsonString - The JSON string to check
 * @param {Object} options - Options
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {boolean} - True if this appears to be a schema definition
 */
export function isJsonSchemaDefinition(jsonString, options = {}) {
  const { debug = false } = options;
  
  if (!jsonString || typeof jsonString !== 'string') {
    if (debug) {
      console.log(`[DEBUG] Schema definition check: Invalid input (${typeof jsonString})`);
    }
    return false;
  }

  try {
    const parsed = JSON.parse(jsonString);
    
    if (debug) {
      console.log(`[DEBUG] Schema definition check: JSON parsed successfully, checking indicators...`);
    }
    
    // Check for common JSON schema properties
    const schemaIndicators = [
      parsed.$schema,
      parsed.$id,
      parsed.title && parsed.description,
      parsed.type === 'object' && parsed.properties,
      parsed.type === 'array' && parsed.items,
      parsed.required && Array.isArray(parsed.required),
      parsed.definitions,
      parsed.additionalProperties !== undefined,
      parsed.patternProperties,
      parsed.anyOf || parsed.oneOf || parsed.allOf
    ];

    const indicatorCount = schemaIndicators.filter(Boolean).length;
    const isSchemaDefinition = indicatorCount >= 2;
    
    if (debug) {
      console.log(`[DEBUG] Schema definition check: Found ${indicatorCount} schema indicators`);
      console.log(`[DEBUG] Schema definition check: Indicators found: ${schemaIndicators.map((indicator, i) => {
        const names = ['$schema', '$id', 'title+description', 'object+properties', 'array+items', 'required', 'definitions', 'additionalProperties', 'patternProperties', 'anyOf/oneOf/allOf'];
        return indicator ? names[i] : null;
      }).filter(Boolean).join(', ')}`);
      console.log(`[DEBUG] Schema definition check: Is schema definition: ${isSchemaDefinition}`);
    }

    return isSchemaDefinition;
  } catch (error) {
    if (debug) {
      console.log(`[DEBUG] Schema definition check: JSON parse failed: ${error.message}`);
    }
    return false;
  }
}

/**
 * Create a correction prompt for invalid JSON
 * @param {string} invalidResponse - The invalid JSON response
 * @param {string} schema - The original schema
 * @param {string} error - The JSON parsing error
 * @param {number} [retryCount=0] - The current retry attempt (0-based)
 * @returns {string} - Correction prompt for the AI
 */
export function createJsonCorrectionPrompt(invalidResponse, schema, error, retryCount = 0) {
  // Create increasingly stronger prompts based on retry attempt
  const strengthLevels = [
    {
      prefix: "CRITICAL JSON ERROR:",
      instruction: "You MUST fix this and return ONLY valid JSON.",
      emphasis: "Return ONLY the corrected JSON, with no additional text or markdown formatting."
    },
    {
      prefix: "URGENT - JSON PARSING FAILED:",
      instruction: "This is your second chance. Return ONLY valid JSON that can be parsed by JSON.parse().",
      emphasis: "ABSOLUTELY NO explanatory text, greetings, or formatting. ONLY JSON."
    },
    {
      prefix: "FINAL ATTEMPT - CRITICAL JSON ERROR:",
      instruction: "This is the final retry. You MUST return ONLY raw JSON without any other content.",
      emphasis: "EXAMPLE: {\"key\": \"value\"} NOT: ```json{\"key\": \"value\"}``` NOT: Here is the JSON: {\"key\": \"value\"}"
    }
  ];

  const level = Math.min(retryCount, strengthLevels.length - 1);
  const currentLevel = strengthLevels[level];

  let prompt = `${currentLevel.prefix} Your previous response is not valid JSON and cannot be parsed. Here's what you returned:

${invalidResponse.substring(0, 500)}${invalidResponse.length > 500 ? '...' : ''}

Error: ${error}

${currentLevel.instruction}

Schema to match:
${schema}

${currentLevel.emphasis}`;

  return prompt;
}

/**
 * Create a correction prompt specifically for when AI returns schema definition instead of data
 * @param {string} schemaDefinition - The JSON schema definition that was incorrectly returned
 * @param {string} originalSchema - The original schema that should be followed
 * @param {number} [retryCount=0] - The current retry attempt (0-based)
 * @returns {string} - Correction prompt for the AI
 */
export function createSchemaDefinitionCorrectionPrompt(schemaDefinition, originalSchema, retryCount = 0) {
  const strengthLevels = [
    {
      prefix: "CRITICAL MISUNDERSTANDING:",
      instruction: "You returned a JSON schema definition instead of data. You must return ACTUAL DATA that follows the schema.",
      example: "Instead of: {\"type\": \"object\", \"properties\": {...}}\nReturn: {\"actualData\": \"value\", \"realField\": 123}"
    },
    {
      prefix: "URGENT - WRONG RESPONSE TYPE:",
      instruction: "You are returning the SCHEMA DEFINITION itself. I need DATA that MATCHES the schema, not the schema structure.",
      example: "Schema defines structure - you provide content that fits that structure!"
    },
    {
      prefix: "FINAL ATTEMPT - SCHEMA VS DATA CONFUSION:",
      instruction: "STOP returning schema definitions! Return REAL DATA that conforms to the schema structure.",
      example: "If schema has 'properties.name', return {\"name\": \"actual_value\"} NOT {\"properties\": {\"name\": {...}}}"
    }
  ];

  const level = Math.min(retryCount, strengthLevels.length - 1);
  const currentLevel = strengthLevels[level];

  let prompt = `${currentLevel.prefix} You returned a JSON schema definition when I asked for data that matches a schema.

What you returned (WRONG - this is a schema definition):
${schemaDefinition.substring(0, 300)}${schemaDefinition.length > 300 ? '...' : ''}

What I need: ACTUAL DATA that conforms to this schema structure:
${originalSchema}

${currentLevel.instruction}

${currentLevel.example}

Return ONLY the JSON data object/array that follows the schema structure. NO schema definitions, NO explanations, NO markdown formatting.`;

  return prompt;
}

/**
 * Detect if a schema expects Mermaid diagram output
 * @param {string} schema - The schema string
 * @returns {boolean} - True if schema appears to expect Mermaid diagrams
 */
export function isMermaidSchema(schema) {
  if (!schema || typeof schema !== 'string') {
    return false;
  }

  const trimmedSchema = schema.trim().toLowerCase();
  
  // Check for Mermaid-related keywords
  const mermaidIndicators = [
    trimmedSchema.includes('mermaid'),
    trimmedSchema.includes('diagram'),
    trimmedSchema.includes('flowchart'),
    trimmedSchema.includes('sequence'),
    trimmedSchema.includes('gantt'),
    trimmedSchema.includes('pie chart'),
    trimmedSchema.includes('state diagram'),
    trimmedSchema.includes('class diagram'),
    trimmedSchema.includes('entity relationship'),
    trimmedSchema.includes('user journey'),
    trimmedSchema.includes('git graph'),
    trimmedSchema.includes('requirement diagram'),
    trimmedSchema.includes('c4 context')
  ];

  return mermaidIndicators.some(indicator => indicator);
}

/**
 * Extract Mermaid diagrams from markdown code blocks with position tracking
 * @param {string} response - Response that may contain markdown with mermaid blocks
 * @returns {Object} - {diagrams: Array<{content: string, fullMatch: string, startIndex: number, endIndex: number}>, cleanedResponse: string}
 */
export function extractMermaidFromMarkdown(response) {
  if (!response || typeof response !== 'string') {
    return { diagrams: [], cleanedResponse: response };
  }

  // Find all mermaid code blocks with enhanced regex to capture more variations
  // This regex captures optional attributes on same line as ```mermaid, and all diagram content
  const mermaidBlockRegex = /```mermaid([^\n]*)\n([\s\S]*?)```/gi;
  const diagrams = [];
  let match;

  while ((match = mermaidBlockRegex.exec(response)) !== null) {
    const attributes = match[1] ? match[1].trim() : '';
    const fullContent = match[2].trim();
    
    // If attributes exist, they were captured separately, so fullContent is just the diagram
    // If no attributes, the first line of fullContent might be diagram type or actual content
    diagrams.push({
      content: fullContent,
      fullMatch: match[0],
      startIndex: match.index,
      endIndex: match.index + match[0].length,
      attributes: attributes
    });
  }

  // Return cleaned response (original for now, could be modified if needed)
  return { diagrams, cleanedResponse: response };
}

/**
 * Replace mermaid diagrams in original markdown with corrected versions
 * @param {string} originalResponse - Original response with markdown
 * @param {Array} correctedDiagrams - Array of corrected diagram objects
 * @returns {string} - Response with corrected diagrams in original format
 */
export function replaceMermaidDiagramsInMarkdown(originalResponse, correctedDiagrams) {
  if (!originalResponse || typeof originalResponse !== 'string') {
    return originalResponse;
  }

  if (!correctedDiagrams || correctedDiagrams.length === 0) {
    return originalResponse;
  }

  let modifiedResponse = originalResponse;
  
  // Sort diagrams by start index in reverse order to preserve indices during replacement
  const sortedDiagrams = [...correctedDiagrams].sort((a, b) => b.startIndex - a.startIndex);
  
  for (const diagram of sortedDiagrams) {
    // Reconstruct the code block with original attributes if they existed
    const attributesStr = diagram.attributes ? ` ${diagram.attributes}` : '';
    const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${diagram.content}\n\`\`\``;
    
    // Replace the original code block
    modifiedResponse = modifiedResponse.slice(0, diagram.startIndex) + 
                     newCodeBlock + 
                     modifiedResponse.slice(diagram.endIndex);
  }
  
  return modifiedResponse;
}

/**
 * Validate a single Mermaid diagram
 * @param {string} diagram - Mermaid diagram code
 * @returns {Promise<Object>} - {isValid: boolean, diagramType?: string, error?: string, detailedError?: string}
 */
export async function validateMermaidDiagram(diagram) {
  if (!diagram || typeof diagram !== 'string') {
    return { isValid: false, error: 'Empty or invalid diagram input' };
  }

  try {
    const trimmedDiagram = diagram.trim();
    
    // Check for markdown code block markers
    if (trimmedDiagram.includes('```')) {
      return { 
        isValid: false, 
        error: 'Diagram contains markdown code block markers',
        detailedError: 'Mermaid diagram should not contain ``` markers when extracted from markdown'
      };
    }

    // Check for common mermaid diagram types (more flexible patterns)
    const diagramPatterns = [
      { pattern: /^(graph|flowchart)/i, type: 'flowchart' },
      { pattern: /^sequenceDiagram/i, type: 'sequence' },
      { pattern: /^gantt/i, type: 'gantt' },
      { pattern: /^pie/i, type: 'pie' },
      { pattern: /^stateDiagram/i, type: 'state' },
      { pattern: /^classDiagram/i, type: 'class' },
      { pattern: /^erDiagram/i, type: 'er' },
      { pattern: /^journey/i, type: 'journey' },
      { pattern: /^gitgraph/i, type: 'gitgraph' },
      { pattern: /^requirementDiagram/i, type: 'requirement' },
      { pattern: /^C4Context/i, type: 'c4' },
    ];

    // Find matching diagram type
    let diagramType = null;
    for (const { pattern, type } of diagramPatterns) {
      if (pattern.test(trimmedDiagram)) {
        diagramType = type;
        break;
      }
    }
    
    if (!diagramType) {
      return { 
        isValid: false, 
        error: 'Diagram does not match any known Mermaid diagram pattern',
        detailedError: 'The diagram must start with a valid Mermaid diagram type (graph, sequenceDiagram, gantt, pie, etc.)'
      };
    }

    // GitHub-compatible strict syntax validation
    const lines = trimmedDiagram.split('\n');
    
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (!line) continue;
      
      // Check for GitHub-incompatible patterns that cause "got 'PS'" errors
      if (diagramType === 'flowchart') {
        // Check for unbalanced brackets in node labels
        const brackets = line.match(/\[[^\]]*$/); // Unclosed bracket
        if (brackets) {
          return {
            isValid: false,
            error: `Unclosed bracket on line ${i + 1}`,
            detailedError: `Line "${line}" contains an unclosed bracket`
          };
        }
        
        // GitHub-strict: Check for parentheses inside node labels (causes PS token error)
        // But allow parentheses inside double-quoted strings
        const nodeWithParens = line.match(/\[[^"\[\]]*\([^"\[\]]*\]/);
        if (nodeWithParens) {
          return {
            isValid: false,
            error: `Parentheses in node label on line ${i + 1} (GitHub incompatible)`,
            detailedError: `Line "${line}" contains parentheses inside node label brackets. GitHub mermaid renderer fails with 'got PS' error. Use quotes or escape characters instead.`
          };
        }
        
        // GitHub-strict: Check for single quotes and backticks inside node labels (causes PS token error)
        const nodeWithQuotes = line.match(/\{[^{}]*['`][^{}]*\}|\[[^[\]]*['`][^[\]]*\]/);
        if (nodeWithQuotes) {
          const hasBacktick = line.includes('`');
          const quoteType = hasBacktick ? 'backticks' : 'single quotes';
          return {
            isValid: false,
            error: `${hasBacktick ? 'Backticks' : 'Single quotes'} in node label on line ${i + 1} (GitHub incompatible)`,
            detailedError: `Line "${line}" contains ${quoteType} inside node label. GitHub mermaid renderer fails with 'got PS' error. Use double quotes or escape characters instead.`
          };
        }
        
        // GitHub-strict: Check for complex expressions inside diamond nodes
        // Allow double-quoted strings in diamond nodes, but catch problematic single quotes and complex expressions
        // Allow HTML breaks (<br/>, <br>, etc.) but catch other problematic patterns
        const diamondWithComplexContent = line.match(/\{[^"{}]*[()'"<>&][^"{}]*\}/);
        const hasHtmlBreak = line.match(/\{[^{}]*<br\s*\/?>.*\}/);
        if (diamondWithComplexContent && !line.match(/\{\"[^\"]*\"\}/) && !hasHtmlBreak) {
          return {
            isValid: false,
            error: `Complex expression in diamond node on line ${i + 1} (GitHub incompatible)`,
            detailedError: `Line "${line}" contains special characters in diamond node that may cause GitHub parsing errors. Use simpler text or escape characters.`
          };
        }
        
        // GitHub-strict: Check for parentheses in subgraph labels (causes PS token error)
        if (line.startsWith('subgraph ') && line.match(/subgraph\s+[^"]*\([^"]*\)/)) {
          return {
            isValid: false,
            error: `Parentheses in subgraph label on line ${i + 1} (GitHub incompatible)`,
            detailedError: `Line "${line}" contains parentheses in subgraph label. GitHub mermaid renderer fails with 'got PS' error. Use quotes around the label or avoid parentheses.`
          };
        }
      }
      
      if (diagramType === 'sequence') {
        // Check for missing colon in sequence messages
        if (line.includes('->>') && !line.includes(':')) {
          return {
            isValid: false,
            error: `Missing colon in sequence message on line ${i + 1}`,
            detailedError: `Line "${line}" appears to be a sequence message but is missing a colon`
          };
        }
      }
    }

    // If we get here, basic validation passed
    return { 
      isValid: true, 
      diagramType
    };
    
  } catch (error) {
    return { 
      isValid: false, 
      error: error.message || 'Unknown mermaid parsing error',
      detailedError: error.stack || error.toString()
    };
  }
}

/**
 * Validate all Mermaid diagrams in a response
 * @param {string} response - Response that may contain mermaid diagrams
 * @returns {Promise<Object>} - {isValid: boolean, diagrams: Array, errors?: Array}
 */
export async function validateMermaidResponse(response) {
  const { diagrams } = extractMermaidFromMarkdown(response);
  
  if (diagrams.length === 0) {
    return { isValid: false, diagrams: [], errors: ['No mermaid diagrams found in response'] };
  }

  const results = [];
  const errors = [];

  for (let i = 0; i < diagrams.length; i++) {
    const diagramObj = diagrams[i];
    const validation = await validateMermaidDiagram(diagramObj.content);
    results.push({
      ...diagramObj,
      ...validation
    });

    if (!validation.isValid) {
      errors.push(`Diagram ${i + 1}: ${validation.error}`);
    }
  }

  const isValid = results.every(result => result.isValid);

  return {
    isValid,
    diagrams: results,
    errors: errors.length > 0 ? errors : undefined
  };
}

/**
 * Create a correction prompt for invalid Mermaid diagrams
 * @param {string} invalidResponse - The response with invalid Mermaid
 * @param {string} schema - The original schema
 * @param {Array} errors - Array of validation errors
 * @param {Array} diagrams - Array of diagram validation results
 * @returns {string} - Correction prompt for the AI
 */
export function createMermaidCorrectionPrompt(invalidResponse, schema, errors, diagrams) {
  let prompt = `Your previous response contains invalid Mermaid diagrams that cannot be parsed. Here's what you returned:

${invalidResponse}

Validation Errors:`;

  errors.forEach((error, index) => {
    prompt += `\n${index + 1}. ${error}`;
  });

  if (diagrams && diagrams.length > 0) {
    prompt += `\n\nDiagram Details:`;
    diagrams.forEach((diagramResult, index) => {
      if (!diagramResult.isValid) {
        prompt += `\n\nDiagram ${index + 1}:`;
        const diagramContent = diagramResult.content || diagramResult.diagram || '';
        prompt += `\n- Content: ${diagramContent.substring(0, 100)}${diagramContent.length > 100 ? '...' : ''}`;
        prompt += `\n- Error: ${diagramResult.error}`;
        if (diagramResult.detailedError && diagramResult.detailedError !== diagramResult.error) {
          prompt += `\n- Details: ${diagramResult.detailedError}`;
        }
      }
    });
  }

  prompt += `\n\nPlease correct your response to include valid Mermaid diagrams that match this schema:
${schema}

Ensure all Mermaid diagrams are properly formatted within \`\`\`mermaid code blocks and follow correct Mermaid syntax.`;

  return prompt;
}

/**
 * Specialized Mermaid diagram fixing agent
 * Uses a separate ProbeAgent instance optimized for Mermaid syntax correction
 */
export class MermaidFixingAgent {
  constructor(options = {}) {
    // Import ProbeAgent dynamically to avoid circular dependencies
    this.ProbeAgent = null;
    this.options = {
      sessionId: options.sessionId || `mermaid-fixer-${Date.now()}`,
      path: options.path || process.cwd(),
      provider: options.provider,
      model: options.model,
      debug: options.debug,
      tracer: options.tracer,
      // Set to false since we're only fixing syntax, not implementing code
      allowEdit: false
    };
  }

  /**
   * Get the specialized prompt for mermaid diagram fixing
   */
  getMermaidFixingPrompt() {
    return `You are a world-class Mermaid diagram syntax correction specialist. Your expertise lies in analyzing and fixing Mermaid diagram syntax errors while preserving the original intent, structure, and semantic meaning.

CORE RESPONSIBILITIES:
- Analyze Mermaid diagrams for syntax errors and structural issues  
- Fix syntax errors while maintaining the original diagram's logical flow
- Ensure diagrams follow proper Mermaid syntax rules and best practices
- Handle all diagram types: flowchart, sequence, gantt, pie, state, class, er, journey, gitgraph, requirement, c4

MERMAID DIAGRAM TYPES & SYNTAX RULES:
1. **Flowchart/Graph**: Start with 'graph' or 'flowchart', use proper node definitions and arrows
2. **Sequence**: Start with 'sequenceDiagram', use proper participant and message syntax
3. **Gantt**: Start with 'gantt', use proper date formats and task definitions
4. **State**: Start with 'stateDiagram-v2', use proper state transitions
5. **Class**: Start with 'classDiagram', use proper class and relationship syntax
6. **Entity-Relationship**: Start with 'erDiagram', use proper entity and relationship syntax

FIXING METHODOLOGY:
1. **Identify diagram type** from the first line or content analysis
2. **Validate syntax** against Mermaid specification for that diagram type
3. **Fix errors systematically**:
   - Unclosed brackets, parentheses, or quotes
   - Missing or incorrect arrows and connectors
   - Invalid node IDs or labels
   - Incorrect formatting for diagram-specific elements
   - **Parentheses in node labels or subgraph names**: Wrap text containing parentheses in double quotes to prevent GitHub parsing errors
   - Single quotes in node labels (GitHub's parser expects double quotes)
4. **Preserve semantic meaning** - never change the intended flow or relationships
5. **Use proper escaping** for special characters and spaces
6. **Ensure consistency** in naming conventions and formatting

CRITICAL RULES:
- ALWAYS output only the corrected Mermaid code within a \`\`\`mermaid code block
- NEVER add explanations, comments, or additional text outside the code block
- PRESERVE the original diagram's intended meaning and flow
- FIX syntax errors without changing the logical structure
- ENSURE the output is valid, parseable Mermaid syntax
- WRAP text containing parentheses in double quotes for GitHub compatibility

When presented with a broken Mermaid diagram, analyze it thoroughly and provide the corrected version that maintains the original intent while fixing all syntax issues.`;
  }

  /**
   * Initialize the ProbeAgent if not already done
   */
  async initializeAgent() {
    if (!this.ProbeAgent) {
      // Dynamic import to avoid circular dependency
      const { ProbeAgent } = await import('./ProbeAgent.js');
      this.ProbeAgent = ProbeAgent;
    }

    if (!this.agent) {
      this.agent = new this.ProbeAgent({
        sessionId: this.options.sessionId,
        customPrompt: this.getMermaidFixingPrompt(),
        path: this.options.path,
        provider: this.options.provider,
        model: this.options.model,
        debug: this.options.debug,
        tracer: this.options.tracer,
        allowEdit: this.options.allowEdit
      });
    }

    return this.agent;
  }

  /**
   * Fix a single Mermaid diagram using the specialized agent
   * @param {string} diagramContent - The broken Mermaid diagram content
   * @param {Array} originalErrors - Array of errors detected in the original diagram
   * @param {Object} diagramInfo - Additional context about the diagram (type, position, etc.)
   * @returns {Promise<string>} - The corrected Mermaid diagram
   */
  async fixMermaidDiagram(diagramContent, originalErrors = [], diagramInfo = {}) {
    // First, try auto-fixing HTML entities without AI
    const decodedContent = decodeHtmlEntities(diagramContent);
    
    // If HTML entity decoding changed the content, validate it first
    if (decodedContent !== diagramContent) {
      try {
        const quickValidation = await validateMermaidDiagram(decodedContent);
        if (quickValidation.isValid) {
          // HTML entity decoding fixed the issue, no need for AI
          if (this.options.debug) {
            console.error('[DEBUG] Fixed Mermaid diagram with HTML entity decoding only');
          }
          return decodedContent;
        }
      } catch (error) {
        // If validation fails, continue with AI fixing using decoded content
        if (this.options.debug) {
          console.error('[DEBUG] HTML entity decoding didn\'t fully fix diagram, continuing with AI fixing');
        }
      }
    }

    await this.initializeAgent();

    const errorContext = originalErrors.length > 0 
      ? `\n\nDetected errors: ${originalErrors.join(', ')}` 
      : '';

    const diagramTypeHint = diagramInfo.diagramType 
      ? `\n\nExpected diagram type: ${diagramInfo.diagramType}` 
      : '';

    // Use decoded content for AI fixing to ensure HTML entities are handled
    const contentToFix = decodedContent !== diagramContent ? decodedContent : diagramContent;
    
    const prompt = `Analyze and fix the following Mermaid diagram.${errorContext}${diagramTypeHint}

Broken Mermaid diagram:
\`\`\`mermaid
${contentToFix}
\`\`\`

Provide only the corrected Mermaid diagram within a mermaid code block. Do not add any explanations or additional text.`;

    try {
      const result = await this.agent.answer(prompt, [], { 
        schema: 'Return only valid Mermaid diagram code within ```mermaid code block' 
      });
      
      // Extract the mermaid code from the response
      const extractedDiagram = this.extractCorrectedDiagram(result);
      return extractedDiagram || result;
    } catch (error) {
      if (this.options.debug) {
        console.error(`[DEBUG] Mermaid fixing failed: ${error.message}`);
      }
      throw new Error(`Failed to fix Mermaid diagram: ${error.message}`);
    }
  }

  /**
   * Extract the corrected diagram from the agent's response
   * @param {string} response - The agent's response
   * @returns {string} - The extracted mermaid diagram
   */
  extractCorrectedDiagram(response) {
    // Try to extract mermaid code block
    const mermaidMatch = response.match(/```mermaid\s*\n([\s\S]*?)\n```/);
    if (mermaidMatch) {
      return mermaidMatch[1].trim();
    }

    // Fallback: try to extract any code block
    const codeMatch = response.match(/```\s*\n([\s\S]*?)\n```/);
    if (codeMatch) {
      return codeMatch[1].trim();
    }

    // If no code blocks found, return the response as-is (cleaned)
    return response.replace(/```\w*\n?/g, '').replace(/\n?```/g, '').trim();
  }

  /**
   * Get token usage information from the specialized agent
   * @returns {Object} - Token usage statistics
   */
  getTokenUsage() {
    return this.agent ? this.agent.getTokenUsage() : null;
  }

  /**
   * Cancel any ongoing operations
   */
  cancel() {
    if (this.agent) {
      this.agent.cancel();
    }
  }
}

/**
 * Enhanced Mermaid validation with specialized agent fixing
 * @param {string} response - Response that may contain mermaid diagrams
 * @param {Object} options - Options for validation and fixing
 * @returns {Promise<Object>} - Enhanced validation result with fixing capability
 */
export async function validateAndFixMermaidResponse(response, options = {}) {
  const { schema, debug, path, provider, model, tracer } = options;
  const startTime = Date.now();
  
  if (debug) {
    console.log(`[DEBUG] Mermaid validation: Starting enhanced validation for response (${response.length} chars)`);
    console.log(`[DEBUG] Mermaid validation: Options - path: ${path}, provider: ${provider}, model: ${model}`);
  }

  /**
   * Helper function to determine if node content needs quoting due to problematic characters
   * @param {string} content - The node content to check
   * @returns {boolean} - True if content needs to be quoted
   */
  const needsQuoting = (content) => {
    return /[()'"<>&`]/.test(content) ||  // Core problematic characters
           content.includes('e.g.') ||
           content.includes('i.e.') ||
           content.includes('src/') ||
           content.includes('defaults/') ||
           content.includes('.ts') ||
           content.includes('.js') ||
           content.includes('.yaml') ||
           content.includes('.json') ||
           content.includes('.md') ||
           content.includes('.html') ||
           content.includes('.css');
  };
  
  // Record mermaid validation start in telemetry
  if (tracer) {
    tracer.recordMermaidValidationEvent('started', {
      'mermaid_validation.response_length': response.length,
      'mermaid_validation.provider': provider,
      'mermaid_validation.model': model
    });
  }
  
  // First, run standard validation
  const validationStart = Date.now();
  const validation = await validateMermaidResponse(response);
  const validationTime = Date.now() - validationStart;
  
  if (debug) {
    console.log(`[DEBUG] Mermaid validation: Initial validation completed in ${validationTime}ms`);
    console.log(`[DEBUG] Mermaid validation: Found ${validation.diagrams?.length || 0} diagrams, valid: ${validation.isValid}`);
    if (validation.diagrams) {
      validation.diagrams.forEach((diag, i) => {
        console.log(`[DEBUG] Mermaid validation: Diagram ${i + 1}: ${diag.isValid ? 'valid' : 'invalid'} (${diag.diagramType || 'unknown type'})`);
        if (!diag.isValid) {
          console.log(`[DEBUG] Mermaid validation: Error for diagram ${i + 1}: ${diag.error}`);
        }
      });
    }
  }
  
  // Always check for HTML entities, even if diagrams are technically valid
  let needsHtmlEntityCheck = false;
  if (validation.diagrams && validation.diagrams.length > 0) {
    for (const diagram of validation.diagrams) {
      if (diagram.content && (diagram.content.includes('&lt;') || diagram.content.includes('&gt;') || diagram.content.includes('&amp;') || diagram.content.includes('&quot;') || diagram.content.includes('&#39;'))) {
        needsHtmlEntityCheck = true;
        break;
      }
    }
  }

  if (validation.isValid && !needsHtmlEntityCheck) {
    if (debug) {
      console.log(`[DEBUG] Mermaid validation: All diagrams valid and no HTML entities found, no fixing needed`);
    }
    
    // Record successful validation in telemetry
    if (tracer) {
      tracer.recordMermaidValidationEvent('completed', {
        'mermaid_validation.success': true,
        'mermaid_validation.diagrams_found': validation.diagrams?.length || 0,
        'mermaid_validation.fixes_needed': false,
        'mermaid_validation.duration_ms': Date.now() - startTime
      });
    }
    
    // All diagrams are valid and no HTML entities found, no fixing needed
    return {
      ...validation,
      wasFixed: false,
      originalResponse: response,
      fixedResponse: response
    };
  }

  // If no diagrams found at all, return without attempting to fix
  if (!validation.diagrams || validation.diagrams.length === 0) {
    if (debug) {
      console.log(`[DEBUG] Mermaid validation: No mermaid diagrams found in response, skipping fixes`);
    }
    return {
      ...validation,
      wasFixed: false,
      originalResponse: response,
      fixedResponse: response
    };
  }

  // Try HTML entity decoding auto-fix (for both invalid diagrams and valid diagrams with HTML entities)
  const invalidCount = validation.diagrams.filter(d => !d.isValid).length;
  if (debug) {
    if (invalidCount > 0) {
      console.log(`[DEBUG] Mermaid validation: ${invalidCount} invalid diagrams detected, trying HTML entity auto-fix first...`);
    } else {
      console.log(`[DEBUG] Mermaid validation: Diagrams are valid but HTML entities detected, applying HTML entity auto-fix...`);
    }
  }

  try {
    let fixedResponse = response;
    const fixingResults = [];
    let htmlEntityFixesApplied = false;
    
    // Extract diagrams with position information for replacement
    const { diagrams } = extractMermaidFromMarkdown(response);
    
    // First pass: Try HTML entity decoding on ALL diagrams (not just invalid ones)
    // HTML entities in mermaid diagrams are almost always unintended, even if the diagram is technically valid
    const allDiagrams = validation.diagrams
      .map((result, index) => ({ ...result, originalIndex: index }))
      .reverse();

    for (const diagram of allDiagrams) {
      const originalContent = diagram.content;
      const decodedContent = decodeHtmlEntities(originalContent);
      
      if (decodedContent !== originalContent) {
        // HTML entities were found and decoded, validate the result
        try {
          const quickValidation = await validateMermaidDiagram(decodedContent);
          if (quickValidation.isValid) {
            // HTML entity decoding improved this diagram!
            const originalDiagram = diagrams[diagram.originalIndex];
            const attributesStr = originalDiagram.attributes ? ` ${originalDiagram.attributes}` : '';
            const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${decodedContent}\n\`\`\``;
            
            fixedResponse = fixedResponse.slice(0, originalDiagram.startIndex) + 
                           newCodeBlock + 
                           fixedResponse.slice(originalDiagram.endIndex);
            
            fixingResults.push({
              diagramIndex: diagram.originalIndex,
              wasFixed: true,
              originalContent: originalContent,
              fixedContent: decodedContent,
              originalError: diagram.error || 'HTML entity cleanup',
              fixedWithHtmlDecoding: true
            });
            
            htmlEntityFixesApplied = true;
            
            if (debug) {
              console.log(`[DEBUG] Mermaid validation: Fixed diagram ${diagram.originalIndex + 1} with HTML entity decoding`);
              console.log(`[DEBUG] Mermaid validation: Original status: ${diagram.isValid ? 'valid' : 'invalid'} - ${diagram.error || 'no error'}`);
              console.log(`[DEBUG] Mermaid validation: Decoded HTML entities`);
            }
          }
        } catch (error) {
          if (debug) {
            console.log(`[DEBUG] Mermaid validation: HTML entity decoding didn't improve diagram ${diagram.originalIndex + 1}: ${error.message}`);
          }
        }
      }
    }
    
    // If HTML entity fixes were applied, re-validate the entire response
    if (htmlEntityFixesApplied) {
      const revalidation = await validateMermaidResponse(fixedResponse);
      if (revalidation.isValid) {
        // All diagrams are now valid, return without AI fixing
        const totalTime = Date.now() - startTime;
        if (debug) {
          console.log(`[DEBUG] Mermaid validation: All diagrams fixed with HTML entity decoding in ${totalTime}ms, no AI needed`);
          console.log(`[DEBUG] Mermaid validation: Applied ${fixingResults.length} HTML entity fixes`);
        }
        
        // Record HTML entity fix success in telemetry
        if (tracer) {
          tracer.recordMermaidValidationEvent('html_fix_completed', {
            'mermaid_validation.success': true,
            'mermaid_validation.fix_method': 'html_entity_decoding',
            'mermaid_validation.diagrams_fixed': fixingResults.length,
            'mermaid_validation.duration_ms': totalTime
          });
        }
        return {
          ...revalidation,
          wasFixed: true,
          originalResponse: response,
          fixedResponse: fixedResponse,
          fixingResults: fixingResults
        };
      }
    }
    
    // Proactive pass: Fix common node label issues in ALL diagrams (not just invalid ones)
    let proactiveFixesApplied = false;
    
    // Re-extract diagrams after HTML entity fixes
    const { diagrams: currentDiagrams } = extractMermaidFromMarkdown(fixedResponse);
    
    for (let diagramIndex = currentDiagrams.length - 1; diagramIndex >= 0; diagramIndex--) {
      const diagram = currentDiagrams[diagramIndex];
      const originalContent = diagram.content;
      const lines = originalContent.split('\n');
      let wasFixed = false;
      
      // Proactively fix node labels that contain special characters
      const fixedLines = lines.map(line => {
        const trimmedLine = line.trim();
        let modifiedLine = line;
        
        // Enhanced auto-fixing for square bracket nodes [...]
        if (trimmedLine.match(/\[[^\]]*\]/)) {
          modifiedLine = modifiedLine.replace(/\[([^\]]*)\]/g, (match, content) => {
            // Check if already properly quoted with outer quotes
            if (content.trim().startsWith('"') && content.trim().endsWith('"')) {
              // Extract the inner content (between the outer quotes)
              const innerContent = content.trim().slice(1, -1);
              // Check if inner content has unescaped quotes that need escaping
              if (innerContent.includes('"') || innerContent.includes("'")) {
                wasFixed = true;
                // Decode any existing HTML entities first, then re-encode ALL quotes
                const decodedContent = decodeHtmlEntities(innerContent);
                const safeContent = decodedContent
                  .replace(/"/g, '&quot;')  // Replace ALL double quotes with HTML entity
                  .replace(/'/g, '&#39;');  // Replace ALL single quotes with HTML entity
                return `["${safeContent}"]`;
              }
              return match;
            }
            
            // Check if content needs quoting (contains problematic patterns)
            if (needsQuoting(content)) {
              wasFixed = true;
              // Use HTML entities for quotes as per Mermaid best practices:
              // - GitHub doesn't support single quotes in node labels (causes 'got PS' error)
              // - HTML entities are the official way to escape quotes in Mermaid
              // - Always use double quotes with square brackets ["..."] for node labels
              // IMPORTANT: Decode any existing HTML entities first to avoid double-encoding
              const decodedContent = decodeHtmlEntities(content);
              const safeContent = decodedContent
                .replace(/"/g, '&quot;')  // Replace double quotes with HTML entity
                .replace(/'/g, '&#39;');  // Replace single quotes with HTML entity
              return `["${safeContent}"]`;
            }
            
            return match;
          });
        }
        
        // Enhanced auto-fixing for diamond nodes {...}
        if (trimmedLine.match(/\{[^{}]*\}/)) {
          modifiedLine = modifiedLine.replace(/\{([^{}]*)\}/g, (match, content) => {
            // Check if already properly quoted with outer quotes
            if (content.trim().startsWith('"') && content.trim().endsWith('"')) {
              // Extract the inner content (between the outer quotes)
              const innerContent = content.trim().slice(1, -1);
              // Check if inner content has unescaped quotes that need escaping
              if (innerContent.includes('"') || innerContent.includes("'")) {
                wasFixed = true;
                // Decode any existing HTML entities first, then re-encode ALL quotes
                const decodedContent = decodeHtmlEntities(innerContent);
                const safeContent = decodedContent
                  .replace(/"/g, '&quot;')  // Replace ALL double quotes with HTML entity
                  .replace(/'/g, '&#39;');  // Replace ALL single quotes with HTML entity
                return `{"${safeContent}"}`;
              }
              return match;
            }
            
            // Check if content needs quoting (contains problematic patterns)
            if (needsQuoting(content)) {
              wasFixed = true;
              // Use HTML entities for quotes as per Mermaid best practices:
              // - GitHub doesn't support single quotes in node labels (causes 'got PS' error)
              // - HTML entities are the official way to escape quotes in Mermaid
              // - Always use double quotes with curly brackets {"..."} for diamond nodes
              // IMPORTANT: Decode any existing HTML entities first to avoid double-encoding
              const decodedContent = decodeHtmlEntities(content);
              const safeContent = decodedContent
                .replace(/"/g, '&quot;')  // Replace double quotes with HTML entity
                .replace(/'/g, '&#39;');  // Replace single quotes with HTML entity
              return `{"${safeContent}"}`;
            }
            
            return match;
          });
        }
        
        return modifiedLine;
      });
      
      if (wasFixed) {
        const fixedContent = fixedLines.join('\n');
        
        // Replace the diagram in the response
        const attributesStr = diagram.attributes ? ` ${diagram.attributes}` : '';
        const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${fixedContent}\n\`\`\``;
        
        fixedResponse = fixedResponse.slice(0, diagram.startIndex) + 
                       newCodeBlock + 
                       fixedResponse.slice(diagram.endIndex);
        
        fixingResults.push({
          diagramIndex: diagramIndex,
          wasFixed: true,
          originalContent: originalContent,
          fixedContent: fixedContent,
          originalError: 'Proactive node label quoting',
          fixMethod: 'node_label_quote_wrapping',
          fixedWithProactiveQuoting: true
        });
        
        proactiveFixesApplied = true;
        
        if (debug) {
          console.log(`[DEBUG] Mermaid validation: Proactively fixed diagram ${diagramIndex + 1} with node label quoting`);
          console.log(`[DEBUG] Mermaid validation: Applied automatic quoting to special characters`);
        }
      }
    }
    
    // If proactive fixes were applied, re-validate the entire response
    if (proactiveFixesApplied) {
      const revalidation = await validateMermaidResponse(fixedResponse);
      if (revalidation.isValid) {
        // All diagrams are now valid, return without AI fixing
        const totalTime = Date.now() - startTime;
        if (debug) {
          console.log(`[DEBUG] Mermaid validation: All diagrams fixed with proactive quoting in ${totalTime}ms, no AI needed`);
          console.log(`[DEBUG] Mermaid validation: Applied ${fixingResults.length} proactive fixes`);
        }
        
        // Record proactive fix success in telemetry
        if (tracer) {
          tracer.recordMermaidValidationEvent('proactive_fix_completed', {
            'mermaid_validation.success': true,
            'mermaid_validation.fix_method': 'node_label_quote_wrapping',
            'mermaid_validation.diagrams_fixed': fixingResults.length,
            'mermaid_validation.duration_ms': totalTime
          });
        }
        return {
          ...revalidation,
          wasFixed: true,
          originalResponse: response,
          fixedResponse: fixedResponse,
          fixingResults: fixingResults,
          performanceMetrics: {
            totalTimeMs: totalTime,
            aiFixingTimeMs: 0,
            finalValidationTimeMs: 0,
            diagramsProcessed: fixingResults.length,
            diagramsFixed: fixingResults.length
          }
        };
      }
    }
    
    // Second pass: Try auto-fixing unquoted subgraph names with parentheses
    let subgraphFixesApplied = false;
    
    // Re-extract diagrams and re-validate after HTML entity fixes
    const { diagrams: postHtmlDiagrams } = extractMermaidFromMarkdown(fixedResponse);
    const postHtmlValidation = await validateMermaidResponse(fixedResponse);
    
    const stillInvalidAfterHtml = postHtmlValidation.diagrams
      .map((result, index) => ({ ...result, originalIndex: index }))
      .filter(result => !result.isValid)
      .reverse();

    for (const invalidDiagram of stillInvalidAfterHtml) {
      // Check if this is a subgraph parentheses error that we can auto-fix
      if (invalidDiagram.error && invalidDiagram.error.includes('Parentheses in subgraph label')) {
        const originalContent = invalidDiagram.content;
        const lines = originalContent.split('\n');
        let wasFixed = false;
        
        // Find and fix unquoted subgraph lines with parentheses
        const fixedLines = lines.map(line => {
          const trimmedLine = line.trim();
          if (trimmedLine.startsWith('subgraph ') && 
              !trimmedLine.match(/subgraph\s+"[^"]*"/) && // not already quoted
              trimmedLine.match(/subgraph\s+[^"]*\([^"]*\)/)) { // has unquoted parentheses
            
            // Extract the subgraph name part
            const match = trimmedLine.match(/^(\s*subgraph\s+)(.+)$/);
            if (match) {
              const prefix = match[1];
              const name = match[2];
              const fixedLine = line.replace(trimmedLine, `${prefix.trim()} "${name}"`);
              wasFixed = true;
              return fixedLine;
            }
          }
          return line;
        });
        
        if (wasFixed) {
          const fixedContent = fixedLines.join('\n');
          
          // Validate the fixed content
          try {
            const quickValidation = await validateMermaidDiagram(fixedContent);
            if (quickValidation.isValid) {
              // Subgraph auto-fix worked!
              const originalDiagram = postHtmlDiagrams[invalidDiagram.originalIndex];
              const attributesStr = originalDiagram.attributes ? ` ${originalDiagram.attributes}` : '';
              const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${fixedContent}\n\`\`\``;
              
              fixedResponse = fixedResponse.slice(0, originalDiagram.startIndex) + 
                             newCodeBlock + 
                             fixedResponse.slice(originalDiagram.endIndex);
              
              fixingResults.push({
                originalIndex: invalidDiagram.originalIndex,
                wasFixed: true,
                originalError: invalidDiagram.error,
                fixMethod: 'subgraph_quote_wrapping',
                fixedWithSubgraphQuoting: true
              });
              
              subgraphFixesApplied = true;
              
              if (debug) {
                console.log(`[DEBUG] Mermaid validation: Fixed diagram ${invalidDiagram.originalIndex + 1} with subgraph quote wrapping`);
                console.log(`[DEBUG] Mermaid validation: Original error: ${invalidDiagram.error}`);
              }
            }
          } catch (error) {
            if (debug) {
              console.log(`[DEBUG] Mermaid validation: Subgraph auto-fix didn't work for diagram ${invalidDiagram.originalIndex + 1}: ${error.message}`);
            }
          }
        }
      }
    }
    
    // If subgraph fixes were applied, re-validate the entire response
    if (subgraphFixesApplied) {
      const revalidation = await validateMermaidResponse(fixedResponse);
      if (revalidation.isValid) {
        // All diagrams are now valid, return without AI fixing
        const totalTime = Date.now() - startTime;
        if (debug) {
          console.log(`[DEBUG] Mermaid validation: All diagrams fixed with auto-fixes in ${totalTime}ms, no AI needed`);
          console.log(`[DEBUG] Mermaid validation: Applied ${fixingResults.length} auto-fixes (HTML entities + subgraph quotes)`);
        }
        
        // Record auto-fix success in telemetry
        if (tracer) {
          tracer.recordMermaidValidationEvent('auto_fix_completed', {
            'mermaid_validation.success': true,
            'mermaid_validation.fix_method': 'auto_fixes',
            'mermaid_validation.diagrams_fixed': fixingResults.length,
            'mermaid_validation.duration_ms': totalTime
          });
        }
        return {
          ...revalidation,
          wasFixed: true,
          originalResponse: response,
          fixedResponse: fixedResponse,
          fixingResults: fixingResults,
          performanceMetrics: {
            totalTimeMs: totalTime,
            aiFixingTimeMs: 0,
            finalValidationTimeMs: 0,
            diagramsProcessed: fixingResults.length,
            diagramsFixed: fixingResults.length
          }
        };
      }
    }
    
    // Third pass: Try auto-fixing node labels with parentheses or single quotes
    let nodeLabelFixesApplied = false;
    
    // Re-extract diagrams and re-validate after previous fixes
    const { diagrams: postSubgraphDiagrams } = extractMermaidFromMarkdown(fixedResponse);
    const postSubgraphValidation = await validateMermaidResponse(fixedResponse);
    
    const stillInvalidAfterSubgraph = postSubgraphValidation.diagrams
      .map((result, index) => ({ ...result, originalIndex: index }))
      .filter(result => !result.isValid)
      .reverse();

    for (const invalidDiagram of stillInvalidAfterSubgraph) {
      // Check if this is a node label error that we can auto-fix
      if (invalidDiagram.error && 
          (invalidDiagram.error.includes('Parentheses in node label') || 
           invalidDiagram.error.includes('Complex expression in diamond node') ||
           invalidDiagram.error.includes('Single quotes in node label') ||
           invalidDiagram.error.includes('Backticks in node label'))) {
        const originalContent = invalidDiagram.content;
        const lines = originalContent.split('\n');
        let wasFixed = false;
        
        // Find and fix node labels with special characters that need quoting
        const fixedLines = lines.map(line => {
          const trimmedLine = line.trim();
          let modifiedLine = line;
          
          // Enhanced auto-fixing for square bracket nodes [...]
          // Look for any node labels that contain special characters and aren't already quoted
          if (trimmedLine.match(/\[[^\]]*\]/)) {
            modifiedLine = modifiedLine.replace(/\[([^\]]*)\]/g, (match, content) => {
              // Check if already properly quoted with outer quotes
              if (content.trim().startsWith('"') && content.trim().endsWith('"')) {
                // Extract the inner content (between the outer quotes)
                const innerContent = content.trim().slice(1, -1);
                // Check if inner content has unescaped quotes that need escaping
                if (innerContent.includes('"') || innerContent.includes("'")) {
                  wasFixed = true;
                  // Decode any existing HTML entities first, then re-encode ALL quotes
                  const decodedContent = decodeHtmlEntities(innerContent);
                  const safeContent = decodedContent
                    .replace(/"/g, '&quot;')  // Replace ALL double quotes with HTML entity
                    .replace(/'/g, '&#39;');  // Replace ALL single quotes with HTML entity
                  return `["${safeContent}"]`;
                }
                return match;
              }
              
              // Check if content needs quoting (contains problematic patterns)
              if (needsQuoting(content)) {
                wasFixed = true;
                // Use HTML entities for quotes as per Mermaid best practices:
                // - GitHub doesn't support single quotes in node labels (causes 'got PS' error)
                // - HTML entities are the official way to escape quotes in Mermaid
                // - Always use double quotes with square brackets ["..."] for node labels
                // IMPORTANT: Decode any existing HTML entities first to avoid double-encoding
                const decodedContent = decodeHtmlEntities(content);
                const safeContent = decodedContent
                  .replace(/"/g, '&quot;')  // Replace double quotes with HTML entity
                  .replace(/'/g, '&#39;');  // Replace single quotes with HTML entity
                return `["${safeContent}"]`;
              }
              
              return match;
            });
          }
          
          // Enhanced auto-fixing for diamond nodes {...}
          if (trimmedLine.match(/\{[^{}]*\}/)) {
            modifiedLine = modifiedLine.replace(/\{([^{}]*)\}/g, (match, content) => {
              // Check if already properly quoted with outer quotes
              if (content.trim().startsWith('"') && content.trim().endsWith('"')) {
                // Extract the inner content (between the outer quotes)
                const innerContent = content.trim().slice(1, -1);
                // Check if inner content has unescaped quotes that need escaping
                if (innerContent.includes('"') || innerContent.includes("'")) {
                  wasFixed = true;
                  // Decode any existing HTML entities first, then re-encode ALL quotes
                  const decodedContent = decodeHtmlEntities(innerContent);
                  const safeContent = decodedContent
                    .replace(/"/g, '&quot;')  // Replace ALL double quotes with HTML entity
                    .replace(/'/g, '&#39;');  // Replace ALL single quotes with HTML entity
                  return `{"${safeContent}"}`;
                }
                return match;
              }
              
              // Check if content needs quoting (contains problematic patterns)
              if (needsQuoting(content)) {
                wasFixed = true;
                // Use HTML entities for quotes as per Mermaid best practices:
                // - GitHub doesn't support single quotes in node labels (causes 'got PS' error)
                // - HTML entities are the official way to escape quotes in Mermaid
                // - Always use double quotes with curly brackets {"..."} for diamond nodes
                // IMPORTANT: Decode any existing HTML entities first to avoid double-encoding
                const decodedContent = decodeHtmlEntities(content);
                const safeContent = decodedContent
                  .replace(/"/g, '&quot;')  // Replace double quotes with HTML entity
                  .replace(/'/g, '&#39;');  // Replace single quotes with HTML entity
                return `{"${safeContent}"}`;
              }
              
              return match;
            });
          }
          
          return modifiedLine;
        });
        
        if (wasFixed) {
          const fixedContent = fixedLines.join('\n');
          
          // Validate the fixed content
          try {
            const quickValidation = await validateMermaidDiagram(fixedContent);
            if (quickValidation.isValid) {
              // Node label auto-fix worked!
              const originalDiagram = postSubgraphDiagrams[invalidDiagram.originalIndex];
              const attributesStr = originalDiagram.attributes ? ` ${originalDiagram.attributes}` : '';
              const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${fixedContent}\n\`\`\``;
              
              fixedResponse = fixedResponse.slice(0, originalDiagram.startIndex) + 
                             newCodeBlock + 
                             fixedResponse.slice(originalDiagram.endIndex);
              
              fixingResults.push({
                originalIndex: invalidDiagram.originalIndex,
                wasFixed: true,
                originalError: invalidDiagram.error,
                fixMethod: 'node_label_quote_wrapping',
                fixedWithNodeLabelQuoting: true
              });
              
              nodeLabelFixesApplied = true;
              
              if (debug) {
                console.log(`[DEBUG] Mermaid validation: Fixed diagram ${invalidDiagram.originalIndex + 1} with node label quote wrapping`);
                console.log(`[DEBUG] Mermaid validation: Original error: ${invalidDiagram.error}`);
              }
            }
          } catch (error) {
            if (debug) {
              console.log(`[DEBUG] Mermaid validation: Node label auto-fix didn't work for diagram ${invalidDiagram.originalIndex + 1}: ${error.message}`);
            }
          }
        }
      }
    }
    
    // If node label fixes were applied, re-validate the entire response
    if (nodeLabelFixesApplied) {
      const revalidation = await validateMermaidResponse(fixedResponse);
      if (revalidation.isValid) {
        // All diagrams are now valid, return without AI fixing
        const totalTime = Date.now() - startTime;
        if (debug) {
          console.log(`[DEBUG] Mermaid validation: All diagrams fixed with auto-fixes in ${totalTime}ms, no AI needed`);
          console.log(`[DEBUG] Mermaid validation: Applied ${fixingResults.length} auto-fixes (HTML entities + subgraph quotes + node label quotes)`);
        }
        
        // Record auto-fix success in telemetry
        if (tracer) {
          tracer.recordMermaidValidationEvent('auto_fix_completed', {
            'mermaid_validation.success': true,
            'mermaid_validation.fix_method': 'auto_fixes',
            'mermaid_validation.diagrams_fixed': fixingResults.length,
            'mermaid_validation.duration_ms': totalTime
          });
        }
        return {
          ...revalidation,
          wasFixed: true,
          originalResponse: response,
          fixedResponse: fixedResponse,
          fixingResults: fixingResults,
          performanceMetrics: {
            totalTimeMs: totalTime,
            aiFixingTimeMs: 0,
            finalValidationTimeMs: 0,
            diagramsProcessed: fixingResults.length,
            diagramsFixed: fixingResults.length
          }
        };
      }
    }
    
    // Re-extract diagrams and re-validate after HTML entity fixes
    const { diagrams: updatedDiagrams } = extractMermaidFromMarkdown(fixedResponse);
    const updatedValidation = await validateMermaidResponse(fixedResponse);
    
    // Still have invalid diagrams after all auto-fixes, proceed with AI fixing
    if (debug) {
      const stillInvalidAfterHtml = updatedValidation?.diagrams?.filter(d => !d.isValid)?.length || invalidCount;
      console.log(`[DEBUG] Mermaid validation: ${stillInvalidAfterHtml} diagrams still invalid after HTML entity decoding, starting AI fixing...`);
      console.log(`[DEBUG] Mermaid validation: HTML entity fixes applied: ${fixingResults.length}`);
    }
    
    // Create specialized fixing agent for remaining invalid diagrams
    if (debug) {
      console.log(`[DEBUG] Mermaid validation: Creating specialized AI fixing agent...`);
    }
    const aiFixingStart = Date.now();
    const mermaidFixer = new MermaidFixingAgent({
      path, provider, model, debug, tracer
    });
    
    const stillInvalidDiagrams = updatedValidation.diagrams
      .map((result, index) => ({ ...result, originalIndex: index }))
      .filter(result => !result.isValid)
      .reverse();
      
    if (debug) {
      console.log(`[DEBUG] Mermaid validation: Found ${stillInvalidDiagrams.length} diagrams requiring AI fixing`);
    }

    for (const invalidDiagram of stillInvalidDiagrams) {
      if (debug) {
        console.log(`[DEBUG] Mermaid validation: Attempting AI fix for diagram ${invalidDiagram.originalIndex + 1}`);
        console.log(`[DEBUG] Mermaid validation: Diagram type: ${invalidDiagram.diagramType || 'unknown'}`);
        console.log(`[DEBUG] Mermaid validation: Error to fix: ${invalidDiagram.error}`);
      }
      
      const diagramFixStart = Date.now();
      try {
        const fixedContent = await mermaidFixer.fixMermaidDiagram(
          invalidDiagram.content,
          [invalidDiagram.error],
          { diagramType: invalidDiagram.diagramType }
        );
        const diagramFixTime = Date.now() - diagramFixStart;

        if (fixedContent && fixedContent !== invalidDiagram.content) {
          // Replace the diagram in the response
          const originalDiagram = updatedDiagrams[invalidDiagram.originalIndex];
          const attributesStr = originalDiagram.attributes ? ` ${originalDiagram.attributes}` : '';
          const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${fixedContent}\n\`\`\``;
          
          fixedResponse = fixedResponse.slice(0, originalDiagram.startIndex) + 
                         newCodeBlock + 
                         fixedResponse.slice(originalDiagram.endIndex);
          
          fixingResults.push({
            diagramIndex: invalidDiagram.originalIndex,
            wasFixed: true,
            originalContent: invalidDiagram.content,
            fixedContent: fixedContent,
            originalError: invalidDiagram.error,
            aiFixingTimeMs: diagramFixTime
          });

          if (debug) {
            console.log(`[DEBUG] Mermaid validation: Successfully fixed diagram ${invalidDiagram.originalIndex + 1} in ${diagramFixTime}ms`);
            console.log(`[DEBUG] Mermaid validation: Original error: ${invalidDiagram.error}`);
            console.log(`[DEBUG] Mermaid validation: Content changes: ${invalidDiagram.content.length} -> ${fixedContent.length} chars`);
          }
        } else {
          fixingResults.push({
            diagramIndex: invalidDiagram.originalIndex,
            wasFixed: false,
            originalContent: invalidDiagram.content,
            originalError: invalidDiagram.error,
            fixingError: 'No valid fix generated',
            aiFixingTimeMs: diagramFixTime
          });
          
          if (debug) {
            console.log(`[DEBUG] Mermaid validation: AI fix failed for diagram ${invalidDiagram.originalIndex + 1} - no valid fix generated`);
          }
        }
      } catch (error) {
        const diagramFixTime = Date.now() - diagramFixStart;
        fixingResults.push({
          diagramIndex: invalidDiagram.originalIndex,
          wasFixed: false,
          originalContent: invalidDiagram.content,
          originalError: invalidDiagram.error,
          fixingError: error.message,
          aiFixingTimeMs: diagramFixTime
        });

        if (debug) {
          console.log(`[DEBUG] Mermaid validation: AI fix failed for diagram ${invalidDiagram.originalIndex + 1} after ${diagramFixTime}ms: ${error.message}`);
        }
      }
    }

    // Re-validate the fixed response
    const finalValidationStart = Date.now();
    const finalValidation = await validateMermaidResponse(fixedResponse);
    const finalValidationTime = Date.now() - finalValidationStart;
    const totalTime = Date.now() - startTime;
    const aiFixingTime = Date.now() - aiFixingStart;

    // Check if any diagrams were actually fixed
    const wasActuallyFixed = fixingResults.some(result => result.wasFixed);
    const fixedCount = fixingResults.filter(result => result.wasFixed).length;
    const totalAttempts = fixingResults.length;

    if (debug) {
      console.log(`[DEBUG] Mermaid validation: Final validation completed in ${finalValidationTime}ms`);
      console.log(`[DEBUG] Mermaid validation: Total process time: ${totalTime}ms (AI fixing: ${aiFixingTime}ms)`);
      console.log(`[DEBUG] Mermaid validation: Fixed ${fixedCount}/${totalAttempts} diagrams with AI`);
      console.log(`[DEBUG] Mermaid validation: Final result - all valid: ${finalValidation.isValid}`);
      
      if (mermaidFixer.getTokenUsage) {
        const tokenUsage = mermaidFixer.getTokenUsage();
        console.log(`[DEBUG] Mermaid validation: AI token usage - prompt: ${tokenUsage?.promptTokens || 0}, completion: ${tokenUsage?.completionTokens || 0}`);
      }
    }
    
    // Record final completion in telemetry
    if (tracer) {
      tracer.recordMermaidValidationEvent('completed', {
        'mermaid_validation.success': finalValidation.isValid,
        'mermaid_validation.was_fixed': wasActuallyFixed,
        'mermaid_validation.diagrams_processed': totalAttempts,
        'mermaid_validation.diagrams_fixed': fixedCount,
        'mermaid_validation.total_duration_ms': totalTime,
        'mermaid_validation.ai_fixing_duration_ms': aiFixingTime,
        'mermaid_validation.final_validation_duration_ms': finalValidationTime,
        'mermaid_validation.token_usage': mermaidFixer.getTokenUsage ? mermaidFixer.getTokenUsage() : null
      });
    }

    return {
      ...finalValidation,
      wasFixed: wasActuallyFixed,
      originalResponse: response,
      fixedResponse: fixedResponse,
      fixingResults: fixingResults,
      performanceMetrics: {
        totalTimeMs: totalTime,
        aiFixingTimeMs: aiFixingTime,
        finalValidationTimeMs: finalValidationTime,
        diagramsProcessed: totalAttempts,
        diagramsFixed: fixedCount
      },
      tokenUsage: mermaidFixer.getTokenUsage()
    };

  } catch (error) {
    if (debug) {
      console.error(`[DEBUG] Mermaid fixing agent failed: ${error.message}`);
    }

    // Return original validation with fixing error
    return {
      ...validation,
      wasFixed: false,
      originalResponse: response,
      fixedResponse: response,
      fixingError: error.message
    };
  }
}