/**
 * Utility functions for cleaning and validating schema responses from AI models
 * Supports JSON and Mermaid diagram validation
 */

import { createMessagePreview } from '../tools/common.js';
import { validate, fixText, extractMermaidBlocks } from '@probelabs/maid';
import Ajv from 'ajv';

/**
 * Generate an example JSON object from a JSON schema
 * @param {Object|string} schema - JSON schema object or string
 * @param {Object} options - Options for generation
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {Object|null} - Example object, or null if schema cannot be processed
 */
export function generateExampleFromSchema(schema, options = {}) {
  const { debug = false } = options;

  try {
    const parsedSchema = typeof schema === 'string' ? JSON.parse(schema) : schema;

    if (parsedSchema.type !== 'object' || !parsedSchema.properties) {
      return null;
    }

    const exampleObj = {};
    for (const [key, value] of Object.entries(parsedSchema.properties)) {
      if (value.type === 'boolean') {
        exampleObj[key] = false;
      } else if (value.type === 'number') {
        exampleObj[key] = 0;
      } else if (value.type === 'string') {
        exampleObj[key] = value.description || 'your answer here';
      } else if (value.type === 'array') {
        exampleObj[key] = [];
      } else {
        exampleObj[key] = {};
      }
    }

    return exampleObj;
  } catch (e) {
    if (debug) {
      console.error('[DEBUG] generateExampleFromSchema: Failed to parse schema:', e.message);
    }
    return null;
  }
}

/**
 * Generate schema instructions for AI to follow
 * @param {Object|string} schema - JSON schema object or string
 * @param {Object} options - Options for generation
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {string} - Formatted schema instructions
 */
export function generateSchemaInstructions(schema, options = {}) {
  const { debug = false } = options;

  let instructions = '\n\nIMPORTANT: When you provide your final answer using attempt_completion, you MUST format it as valid JSON matching this schema:\n\n';

  try {
    const parsedSchema = typeof schema === 'string' ? JSON.parse(schema) : schema;
    instructions += `${JSON.stringify(parsedSchema, null, 2)}\n\n`;
  } catch (e) {
    if (debug) {
      console.error('[DEBUG] generateSchemaInstructions: Failed to parse schema:', e.message);
    }
    instructions += `${schema}\n\n`;
  }

  instructions += 'Your response inside attempt_completion must be ONLY valid JSON - no plain text, no explanations, no markdown.\n\nIMPORTANT: First complete the requested analysis/task thoroughly, then provide your final answer in the JSON format above.';

  return instructions;
}

/**
 * Recursively apply additionalProperties: false to all object schemas
 * This ensures strict validation at all nesting levels
 * @param {Object} schema - JSON schema object
 * @returns {Object} - Modified schema with additionalProperties enforced
 */
function enforceNoAdditionalProperties(schema) {
  if (!schema || typeof schema !== 'object') {
    return schema;
  }

  // Create a deep clone to avoid modifying the original
  const cloned = JSON.parse(JSON.stringify(schema));

  function applyRecursively(obj) {
    if (!obj || typeof obj !== 'object') {
      return;
    }

    // If this is an object type schema and doesn't have additionalProperties set
    if (obj.type === 'object' && obj.additionalProperties === undefined) {
      obj.additionalProperties = false;
    }

    // Recursively process properties
    if (obj.properties && typeof obj.properties === 'object') {
      Object.values(obj.properties).forEach(applyRecursively);
    }

    // Process items for arrays
    if (obj.items) {
      if (Array.isArray(obj.items)) {
        obj.items.forEach(applyRecursively);
      } else {
        applyRecursively(obj.items);
      }
    }

    // Process nested schemas in oneOf, anyOf, allOf
    ['oneOf', 'anyOf', 'allOf'].forEach(key => {
      if (Array.isArray(obj[key])) {
        obj[key].forEach(applyRecursively);
      }
    });

    // Process definitions/defs
    if (obj.definitions && typeof obj.definitions === 'object') {
      Object.values(obj.definitions).forEach(applyRecursively);
    }
    if (obj.$defs && typeof obj.$defs === 'object') {
      Object.values(obj.$defs).forEach(applyRecursively);
    }
  }

  applyRecursively(cloned);
  return cloned;
}

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
 *
 * NOTE: This function handles both JSON extraction AND content stripping.
 * Future improvement: Consider splitting into separate functions for better separation of concerns:
 * - extractJsonContent() - Find and extract JSON from response
 * - stripNonJsonContent() - Remove explanatory text while preserving validation-relevant content
 * This would allow validation to run on full responses before content is discarded.
 *
 * @param {string} response - Raw AI response
 * @returns {string} - Cleaned response with JSON boundaries extracted if applicable
 */
export function cleanSchemaResponse(response) {
  if (!response || typeof response !== 'string') {
    return response;
  }

  const trimmed = response.trim();

  // First, look for JSON after code block markers - similar to mermaid extraction
  // Try with json language specifier
  const jsonBlockMatch = trimmed.match(/```json\s*\n([\s\S]*?)\n```/);
  if (jsonBlockMatch) {
    return jsonBlockMatch[1].trim();
  }

  // Try any code block with JSON content
  const anyBlockMatch = trimmed.match(/```\s*\n([{\[][\s\S]*?[}\]])\s*```/);
  if (anyBlockMatch) {
    return anyBlockMatch[1].trim();
  }

  // Legacy patterns for more specific matching
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

  // Fallback: Check if response is JSON after removing code block markers and whitespace
  // First, remove common code block markers from top and bottom
  let cleaned = trimmed;

  // Remove opening code block markers (```json, ```, etc.)
  cleaned = cleaned.replace(/^```(?:json)?\s*\n?/i, '');

  // Remove closing code block markers
  cleaned = cleaned.replace(/\n?```\s*$/, '');

  // Trim whitespace and newlines
  cleaned = cleaned.trim();

  // Now check if first and last characters are valid JSON boundaries
  const firstChar = cleaned[0];
  const lastChar = cleaned[cleaned.length - 1];

  const isJsonObject = firstChar === '{' && lastChar === '}';
  const isJsonArray = firstChar === '[' && lastChar === ']';

  if (isJsonObject || isJsonArray) {
    return cleaned;
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
  const { debug = false, schema = null, strictSchema = true } = options;

  if (debug) {
    console.log(`[DEBUG] JSON validation: Starting validation for response (${response.length} chars)`);
    const preview = createMessagePreview(response);
    console.log(`[DEBUG] JSON validation: Preview: ${preview}`);
    if (schema) {
      console.log(`[DEBUG] JSON validation: Schema validation enabled`);
    }
  }

  try {
    const parseStart = Date.now();
    const parsed = JSON.parse(response);
    const parseTime = Date.now() - parseStart;

    if (debug) {
      console.log(`[DEBUG] JSON validation: Successfully parsed in ${parseTime}ms`);
      console.log(`[DEBUG] JSON validation: Object type: ${typeof parsed}, keys: ${Object.keys(parsed || {}).length}`);
    }

    // If schema provided, validate against it
    if (schema) {
      const schemaStart = Date.now();
      let schemaObj;

      try {
        // Parse schema if it's a string
        schemaObj = typeof schema === 'string' ? JSON.parse(schema) : schema;

        // Apply strict mode: enforce additionalProperties: false on all nested objects
        // unless explicitly disabled via strictSchema: false option
        if (strictSchema) {
          schemaObj = enforceNoAdditionalProperties(schemaObj);
          if (debug) {
            console.log(`[DEBUG] JSON validation: Applied strict mode - additionalProperties: false enforced on all levels`);
          }
        }
      } catch (schemaParseError) {
        if (debug) {
          console.log(`[DEBUG] JSON validation: Failed to parse schema: ${schemaParseError.message}`);
        }
        return {
          isValid: false,
          error: 'Invalid schema provided',
          schemaError: schemaParseError.message
        };
      }

      // Create AJV validator with strict mode
      const ajv = new Ajv({
        strict: false,  // Don't fail on unknown schema keywords
        allErrors: true,  // Return all errors, not just the first
        verbose: true  // Include schema and data in errors
      });

      let validate;
      try {
        validate = ajv.compile(schemaObj);
      } catch (compileError) {
        if (debug) {
          console.log(`[DEBUG] JSON validation: Schema compilation failed: ${compileError.message}`);
        }
        return {
          isValid: false,
          error: 'Schema compilation failed',
          schemaError: compileError.message
        };
      }

      const valid = validate(parsed);
      const schemaTime = Date.now() - schemaStart;

      if (debug) {
        console.log(`[DEBUG] JSON validation: Schema validation completed in ${schemaTime}ms, valid: ${valid}`);
      }

      if (!valid) {
        // Format schema errors for better readability and actionability
        const formattedErrors = validate.errors.map(err => {
          // Convert JSON Pointer path to dot notation for readability
          // /user/profile/age -> user.profile.age
          const path = err.instancePath
            ? err.instancePath.substring(1).replace(/\//g, '.')
            : '<root>';

          let message = '';
          let suggestion = '';

          // Create crisp, actionable error messages based on error type
          if (err.keyword === 'additionalProperties') {
            const extraField = err.params.additionalProperty;
            message = `Extra field '${extraField}' is not allowed`;
            suggestion = `Remove '${extraField}' or add it to the schema`;
          } else if (err.keyword === 'required') {
            const missingField = err.params.missingProperty;
            message = `Missing required field '${missingField}'`;
            suggestion = `Add '${missingField}' to this object`;
          } else if (err.keyword === 'type') {
            const expected = err.params.type;
            const actual = typeof err.data;
            const value = JSON.stringify(err.data);
            message = `Wrong type: expected ${expected}, got ${actual} (value: ${value})`;
            suggestion = `Change value to ${expected} type`;
          } else if (err.keyword === 'enum') {
            const allowed = err.params.allowedValues;
            const value = JSON.stringify(err.data);
            message = `Invalid value ${value}. Allowed: ${allowed.map(v => JSON.stringify(v)).join(', ')}`;
            suggestion = `Use one of the allowed values`;
          } else if (err.keyword === 'minimum' || err.keyword === 'maximum') {
            const limit = err.params.limit;
            const comparison = err.params.comparison;
            message = `Value ${err.data} ${err.message} (${comparison} ${limit})`;
            suggestion = `Adjust value to meet constraint`;
          } else if (err.keyword === 'minLength' || err.keyword === 'maxLength') {
            const limit = err.params.limit;
            message = `String length ${err.message} (current: ${err.data.length}, ${err.keyword}: ${limit})`;
            suggestion = `Adjust string length`;
          } else if (err.keyword === 'pattern') {
            message = `Value doesn't match required pattern: ${err.params.pattern}`;
            suggestion = `Update value to match the pattern`;
          } else {
            // Fallback for other error types
            message = err.message;
            suggestion = '';
          }

          // Format: "path: message | suggestion"
          const location = path ? `at '${path}'` : 'at root';
          return suggestion
            ? `${location}: ${message} → ${suggestion}`
            : `${location}: ${message}`;
        });

        const errorSummary = formattedErrors.join('\n  ');

        if (debug) {
          console.log(`[DEBUG] JSON validation: Schema validation errors:\n  ${errorSummary}`);
        }

        return {
          isValid: false,
          error: 'Schema validation failed',
          schemaErrors: validate.errors,
          formattedErrors: formattedErrors,
          errorSummary: `Schema validation failed:\n  ${errorSummary}`,
          parsed: parsed  // Still return parsed data for debugging
        };
      }

      if (debug) {
        console.log(`[DEBUG] JSON validation: Schema validation passed`);
      }
    }

    return { isValid: true, parsed };
  } catch (error) {
    // Extract error position from error message if available
    // Old format: "Unexpected token < in JSON at position 0"
    // New format: "Unexpected token '<', \"...\" is not valid JSON"
    const positionMatch = error.message.match(/position (\d+)/);
    let errorPosition = positionMatch ? parseInt(positionMatch[1], 10) : null;

    // If position not found in old format, try to extract from new format
    if (errorPosition === null) {
      // Try to find the problematic token in the new error format
      const tokenMatch = error.message.match(/Unexpected token '(.)', /);
      if (tokenMatch && tokenMatch[1]) {
        const problematicToken = tokenMatch[1];
        // Find first occurrence of this token in the response
        errorPosition = response.indexOf(problematicToken);
      }
    }

    // Create enhanced error message with context snippet
    let enhancedError = error.message;
    let errorContext = null;

    if (errorPosition !== null && errorPosition >= 0 && response && response.length > 0) {
      // Calculate context window (50 chars before and after)
      const contextRadius = 50;
      const startPos = Math.max(0, errorPosition - contextRadius);
      const endPos = Math.min(response.length, errorPosition + contextRadius);

      // Extract context snippet
      const beforeError = response.substring(startPos, errorPosition);
      const atError = response[errorPosition] || '';
      const afterError = response.substring(errorPosition + 1, endPos);

      // Build error context with visual pointer
      const snippet = beforeError + atError + afterError;
      const pointerOffset = beforeError.length;
      const pointer = ' '.repeat(pointerOffset) + '^';

      errorContext = {
        position: errorPosition,
        snippet: snippet,
        pointer: pointer,
        beforeError: beforeError,
        atError: atError,
        afterError: afterError
      };

      // Create human-readable error context for display
      enhancedError = `${error.message}

Error location (position ${errorPosition}):
${snippet}
${pointer} here`;
    }

    if (debug) {
      console.log(`[DEBUG] JSON validation: Parse failed with error: ${error.message}`);
      console.log(`[DEBUG] JSON validation: Error at position: ${errorPosition !== null ? errorPosition : 'unknown'}`);

      if (errorContext) {
        console.log(`[DEBUG] JSON validation: Error context:\n${errorContext.snippet}\n${errorContext.pointer}`);
      }

      // Try to identify common JSON issues
      if (error.message.includes('Unexpected token')) {
        console.log(`[DEBUG] JSON validation: Likely syntax error - unexpected character`);
      } else if (error.message.includes('Unexpected end')) {
        console.log(`[DEBUG] JSON validation: Likely incomplete JSON - missing closing brackets`);
      } else if (error.message.includes('property name')) {
        console.log(`[DEBUG] JSON validation: Likely unquoted property names`);
      }
    }

    return {
      isValid: false,
      error: error.message,
      enhancedError: enhancedError,
      errorContext: errorContext
    };
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
 * @param {string|Object} errorOrValidation - The JSON parsing error string or validation result object
 * @param {number} [retryCount=0] - The current retry attempt (0-based)
 * @returns {string} - Correction prompt for the AI
 */
export function createJsonCorrectionPrompt(invalidResponse, schema, errorOrValidation, retryCount = 0) {
  // Extract error information from validation result or string
  let errorMessage;
  let enhancedError;

  if (typeof errorOrValidation === 'object' && errorOrValidation !== null) {
    // It's a validation result object
    errorMessage = errorOrValidation.error;
    enhancedError = errorOrValidation.enhancedError || errorMessage;
  } else {
    // It's a plain error string (backwards compatibility)
    errorMessage = errorOrValidation;
    enhancedError = errorMessage;
  }

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

Error: ${enhancedError}

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
 * Extract Mermaid diagrams from JSON string values
 * Handles escaped newlines and backticks within JSON strings
 * @param {string} response - Response that may contain JSON with mermaid blocks in string values
 * @returns {Object} - {diagrams: Array, jsonPaths: Array, parsedJson: Object|null}
 */
export function extractMermaidFromJson(response) {
  if (!response || typeof response !== 'string') {
    return { diagrams: [], jsonPaths: [], parsedJson: null };
  }

  // Try to extract JSON from code blocks first
  let jsonContent = response.trim();
  const jsonBlockMatch = jsonContent.match(/```json\s*\n([\s\S]*?)\n```/);
  if (jsonBlockMatch) {
    jsonContent = jsonBlockMatch[1].trim();
  } else {
    const anyBlockMatch = jsonContent.match(/```\s*\n([{\[][\s\S]*?[}\]])\s*```/);
    if (anyBlockMatch) {
      jsonContent = anyBlockMatch[1].trim();
    }
  }

  // Try to parse as JSON
  let parsedJson;
  try {
    parsedJson = JSON.parse(jsonContent);
  } catch (e) {
    return { diagrams: [], jsonPaths: [], parsedJson: null };
  }

  const diagrams = [];
  const jsonPaths = [];

  // Recursively search for mermaid diagrams in JSON string values
  function searchObject(obj, path = []) {
    if (typeof obj === 'string') {
      // Look for mermaid code blocks in the string value
      // Handle both escaped (\n) and literal newlines
      const mermaidPattern = /```mermaid([^\n`]*?)(?:\n|\\n)([\s\S]*?)```/gi;
      let match;

      while ((match = mermaidPattern.exec(obj)) !== null) {
        const attributes = match[1] ? match[1].trim() : '';
        // Unescape the content (replace \\n with actual newlines)
        const content = match[2].replace(/\\n/g, '\n');

        diagrams.push({
          content: content,
          fullMatch: match[0],
          startIndex: match.index,
          endIndex: match.index + match[0].length,
          attributes: attributes,
          isInJson: true,
          jsonPath: path.join('.')
        });
        jsonPaths.push(path.join('.'));
      }
    } else if (Array.isArray(obj)) {
      obj.forEach((item, index) => searchObject(item, [...path, `[${index}]`]));
    } else if (obj && typeof obj === 'object') {
      Object.entries(obj).forEach(([key, value]) => searchObject(value, [...path, key]));
    }
  }

  searchObject(parsedJson);

  return { diagrams, jsonPaths, parsedJson };
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

  // First check if this looks like a JSON response - if so, use JSON-aware extraction
  const trimmed = response.trim();
  if ((trimmed.startsWith('{') || trimmed.startsWith('[')) ||
      trimmed.includes('```json')) {
    const jsonResult = extractMermaidFromJson(response);
    if (jsonResult.diagrams.length > 0) {
      return { diagrams: jsonResult.diagrams, cleanedResponse: response };
    }
  }

  // Find all mermaid code blocks with enhanced regex to capture more variations
  // This regex captures optional attributes on same line as ```mermaid, and all diagram content
  const mermaidBlockRegex = /```mermaid([^\n]*)\n([\s\S]*?)```/gi;
  const diagrams = [];
  let match;

  while ((match = mermaidBlockRegex.exec(response)) !== null) {
    const attributes = match[1] ? match[1].trim() : '';
    // Don't trim the content - maid 0.0.6 requires trailing newlines for sequence diagrams
    const fullContent = match[2];

    // If attributes exist, they were captured separately, so fullContent is just the diagram
    // If no attributes, the first line of fullContent might be diagram type or actual content
    diagrams.push({
      content: fullContent,
      fullMatch: match[0],
      startIndex: match.index,
      endIndex: match.index + match[0].length,
      attributes: attributes,
      isInJson: false
    });
  }

  // Return cleaned response (original for now, could be modified if needed)
  return { diagrams, cleanedResponse: response };
}

/**
 * Replace mermaid diagrams in JSON string values with corrected versions
 * @param {string} originalResponse - Original response with JSON
 * @param {Array} correctedDiagrams - Array of corrected diagram objects with jsonPath
 * @returns {string} - Response with corrected diagrams properly escaped in JSON
 */
export function replaceMermaidDiagramsInJson(originalResponse, correctedDiagrams) {
  if (!originalResponse || typeof originalResponse !== 'string') {
    return originalResponse;
  }

  if (!correctedDiagrams || correctedDiagrams.length === 0) {
    return originalResponse;
  }

  // Extract and parse JSON
  const jsonResult = extractMermaidFromJson(originalResponse);
  if (!jsonResult.parsedJson) {
    return originalResponse;
  }

  let modifiedJson = jsonResult.parsedJson;

  // Replace diagrams in the JSON object
  for (const diagram of correctedDiagrams) {
    if (!diagram.jsonPath || !diagram.isInJson) {
      continue;
    }

    // Navigate to the path and replace the content
    const pathParts = diagram.jsonPath.split('.').filter(p => p);
    let current = modifiedJson;

    for (let i = 0; i < pathParts.length - 1; i++) {
      const part = pathParts[i];
      if (part.startsWith('[') && part.endsWith(']')) {
        const index = parseInt(part.slice(1, -1), 10);
        current = current[index];
      } else {
        current = current[part];
      }
    }

    // Get the last key/index
    const lastPart = pathParts[pathParts.length - 1];
    const attributesStr = diagram.attributes ? ` ${diagram.attributes}` : '';
    const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${diagram.content}\n\`\`\``;

    if (lastPart.startsWith('[') && lastPart.endsWith(']')) {
      const index = parseInt(lastPart.slice(1, -1), 10);
      const originalString = current[index];
      // The fullMatch from extraction has unescaped newlines, so we need to match that
      current[index] = originalString.replace(diagram.fullMatch, newCodeBlock);
    } else {
      const originalString = current[lastPart];
      // The fullMatch from extraction has unescaped newlines, so we need to match that
      current[lastPart] = originalString.replace(diagram.fullMatch, newCodeBlock);
    }
  }

  // Reconstruct the response with modified JSON
  const modifiedJsonString = JSON.stringify(modifiedJson, null, 2);

  // Check if original was in a code block
  const trimmed = originalResponse.trim();
  if (trimmed.match(/```json\s*\n([\s\S]*?)\n```/)) {
    return originalResponse.replace(/```json\s*\n([\s\S]*?)\n```/, `\`\`\`json\n${modifiedJsonString}\n\`\`\``);
  } else if (trimmed.match(/```\s*\n([{\[][\s\S]*?[}\]])\s*```/)) {
    return originalResponse.replace(/```\s*\n([{\[][\s\S]*?[}\]])\s*```/, `\`\`\`\n${modifiedJsonString}\n\`\`\``);
  }

  return modifiedJsonString;
}

/**
 * Replace mermaid diagrams in original markdown with corrected versions
 * Automatically detects JSON vs markdown format and uses appropriate replacement
 * @param {string} originalResponse - Original response with markdown or JSON
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

  // Check if any diagrams are in JSON format
  const hasJsonDiagrams = correctedDiagrams.some(d => d.isInJson);
  if (hasJsonDiagrams) {
    return replaceMermaidDiagramsInJson(originalResponse, correctedDiagrams);
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
    // Don't trim the diagram - maid 0.0.6 requires trailing newlines for sequence diagrams
    // and handles leading/trailing whitespace correctly

    // Check for markdown code block markers
    if (diagram.includes('```')) {
      return {
        isValid: false,
        error: 'Diagram contains markdown code block markers',
        detailedError: 'Mermaid diagram should not contain ``` markers when extracted from markdown'
      };
    }

    // Use maid to validate the diagram
    const result = validate(diagram);

    // Maid returns { type: string, errors: array }
    // Only count actual errors (severity: 'error'), not warnings
    const actualErrors = (result.errors || []).filter(err => err.severity === 'error');

    // Valid if no actual errors (warnings are OK)
    if (actualErrors.length === 0) {
      return {
        isValid: true,
        diagramType: result.type || 'unknown'
      };
    } else {
      // Format maid errors into a readable error message
      const errorMessages = actualErrors.map(err => {
        const location = err.line ? `line ${err.line}${err.column ? `:${err.column}` : ''}` : '';
        return location ? `${location} - ${err.message}` : err.message;
      });

      return {
        isValid: false,
        diagramType: result.type || 'unknown',
        error: errorMessages[0] || 'Validation failed',
        detailedError: errorMessages.join('\n'),
        errors: actualErrors // Include only actual errors for AI fixing
      };
    }

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

// Counter to ensure unique session IDs even when created in the same millisecond
let sessionIdCounter = 0;

/**
 * Specialized JSON fixing agent
 * Uses a separate ProbeAgent instance optimized for JSON syntax correction
 */
export class JsonFixingAgent {
  constructor(options = {}) {
    // Import ProbeAgent dynamically to avoid circular dependencies
    this.ProbeAgent = null;
    this.options = {
      sessionId: options.sessionId || `json-fixer-${Date.now()}-${sessionIdCounter++}`,
      path: options.path || process.cwd(),
      provider: options.provider,
      model: options.model,
      debug: options.debug,
      tracer: options.tracer,
      // Set to false since we're only fixing JSON syntax, not implementing code
      allowEdit: false
    };
  }

  /**
   * Get the specialized prompt for JSON fixing
   */
  getJsonFixingPrompt() {
    return `You are a world-class JSON syntax correction specialist. Your expertise lies in analyzing and fixing JSON syntax errors while preserving the original data structure and intent.

CORE RESPONSIBILITIES:
- Analyze JSON for syntax errors and structural issues
- Fix syntax errors while maintaining the original data's semantic meaning
- Ensure JSON follows proper RFC 8259 specification
- Handle all JSON structures: objects, arrays, primitives, nested structures

JSON SYNTAX RULES:
1. **Property names**: Must be enclosed in double quotes
2. **String values**: Must use double quotes (not single quotes)
3. **Numbers**: Can be integers or decimals, no quotes needed
4. **Booleans**: true or false (lowercase, no quotes)
5. **Null**: null (lowercase, no quotes)
6. **Arrays**: Comma-separated values in square brackets [...]
7. **Objects**: Comma-separated key-value pairs in curly braces {...}
8. **No trailing commas**: Last item in array/object must not have a trailing comma
9. **Escape sequences**: Special characters must be escaped (\\n, \\t, \\", \\\\, etc.)

COMMON ERRORS TO FIX:
1. **Unquoted property names**: {name: "value"} → {"name": "value"}
2. **Single quotes**: {'key': 'value'} → {"key": "value"}
3. **Trailing commas**: {"a": 1,} → {"a": 1}
4. **Unquoted strings**: {key: value} → {"key": "value"}
5. **Missing commas**: {"a": 1 "b": 2} → {"a": 1, "b": 2}
6. **Extra commas**: {"a": 1,, "b": 2} → {"a": 1, "b": 2}
7. **Unclosed brackets/braces**: {"key": "value" → {"key": "value"}
8. **Invalid escape sequences**: Fix or remove
9. **Comments**: Remove // or /* */ comments (not allowed in JSON)
10. **Undefined values**: Replace undefined with null

FIXING METHODOLOGY:
1. **Identify the error location** from the error message
2. **Analyze the context** around the error
3. **Apply the appropriate fix** based on JSON syntax rules
4. **Preserve data intent** - never change the meaning of the data
5. **Validate the result** - ensure it's parseable JSON

CRITICAL RULES:
- ALWAYS output only the corrected JSON
- NEVER add explanations, comments, or additional text
- NEVER wrap in markdown code blocks (no \`\`\`json)
- PRESERVE the original data structure and values
- FIX only syntax errors, don't modify the data itself
- ENSURE the output is valid, parseable JSON

When presented with broken JSON, analyze it thoroughly and provide the corrected version that maintains the original intent while fixing all syntax issues.`;
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
        customPrompt: this.getJsonFixingPrompt(),
        path: this.options.path,
        provider: this.options.provider,
        model: this.options.model,
        debug: this.options.debug,
        tracer: this.options.tracer,
        allowEdit: this.options.allowEdit,
        maxIterations: 5,  // Allow multiple iterations for JSON fixing
        disableJsonValidation: true  // CRITICAL: Disable JSON validation in nested agent to prevent infinite recursion
      });
    }

    return this.agent;
  }

  /**
   * Fix invalid JSON using the specialized agent
   * @param {string} invalidJson - The broken JSON string
   * @param {string} schema - The original schema for context
   * @param {Object} validationResult - Validation result with error details
   * @param {number} attemptNumber - Current attempt number (for logging)
   * @returns {Promise<string>} - The corrected JSON
   */
  async fixJson(invalidJson, schema, validationResult, attemptNumber = 1) {
    await this.initializeAgent();

    // Build error context from validation result
    let errorContext = validationResult.error;
    if (validationResult.enhancedError) {
      errorContext = validationResult.enhancedError;
    }

    // Add schema validation errors if present
    let schemaErrorDetails = '';
    if (validationResult.errorSummary) {
      schemaErrorDetails = `\n\nSchema Validation Errors:\n${validationResult.errorSummary}`;
    } else if (validationResult.schemaErrors && validationResult.schemaErrors.length > 0) {
      const errors = validationResult.schemaErrors.map(err => {
        const path = err.instancePath || '(root)';
        return `  ${path}: ${err.message}`;
      }).join('\n');
      schemaErrorDetails = `\n\nSchema Validation Errors:\n${errors}`;
    }

    const prompt = `Fix the following invalid JSON.

Error: ${errorContext}${schemaErrorDetails}

Invalid JSON:
${invalidJson}

Expected schema structure:
${schema}

${schemaErrorDetails ? 'CRITICAL: Pay special attention to the schema validation errors above. The JSON may be syntactically valid but does not conform to the required schema. Make sure to:\n- Include all required fields\n- Use correct data types\n- Remove any additional properties not defined in the schema (if additionalProperties is false)\n- Ensure all values match their schema constraints\n\n' : ''}Provide only the corrected JSON without any markdown formatting or explanations.`;

    try {
      if (this.options.debug) {
        console.log(`[DEBUG] JSON fixing: Attempt ${attemptNumber} to fix JSON with separate agent`);
      }

      // Call the specialized JSON fixing agent
      const result = await this.agent.answer(prompt, []);

      // Clean the result (in case AI added markdown despite instructions)
      const cleaned = cleanSchemaResponse(result);

      if (this.options.debug) {
        console.log(`[DEBUG] JSON fixing: Agent returned ${cleaned.length} chars`);
      }

      return cleaned;
    } catch (error) {
      if (this.options.debug) {
        console.error(`[DEBUG] JSON fixing failed: ${error.message}`);
      }
      throw new Error(`Failed to fix JSON: ${error.message}`);
    }
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
 * Use maid to attempt auto-fixing of mermaid diagrams
 * @param {string} diagramContent - The diagram content to fix
 * @param {Object} options - Fix options
 * @returns {Object} - {fixed: string, wasFixed: boolean, errors: Array}
 */
export async function tryMaidAutoFix(diagramContent, options = {}) {
  const { debug = false } = options;

  try {
    // Always use 'all' level fixes (most aggressive)
    if (debug) {
      console.log(`[DEBUG] Mermaid maid: Trying 'all' level auto-fixes...`);
    }
    const result = fixText(diagramContent, { level: 'all' });
    const validation = validate(result.fixed);

    // Maid validation returns { type, errors }
    // Valid if errors array is empty
    if (validation.errors && validation.errors.length === 0) {
      if (debug) {
        console.log(`[DEBUG] Mermaid maid: 'All' level fixes succeeded`);
      }
      return {
        fixed: result.fixed,
        wasFixed: result.fixed !== diagramContent,
        errors: [],
        fixLevel: 'all'
      };
    }

    // Maid couldn't fix it completely, return the best attempt with remaining errors
    if (debug) {
      console.log(`[DEBUG] Mermaid maid: Auto-fixes couldn't resolve all issues, ${validation.errors?.length || 0} errors remain`);
    }
    return {
      fixed: result.fixed,
      wasFixed: result.fixed !== diagramContent,
      errors: validation.errors || [], // Pass maid's structured errors for AI fixing
      fixLevel: 'all'
    };

  } catch (error) {
    if (debug) {
      console.error(`[DEBUG] Mermaid maid: Auto-fix error: ${error.message}`);
    }
    return {
      fixed: diagramContent,
      wasFixed: false,
      errors: [{ message: error.message }],
      fixLevel: null
    };
  }
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
      sessionId: options.sessionId || `mermaid-fixer-${Date.now()}-${sessionIdCounter++}`,
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
   - **Edge/Arrow labels with spaces**: MUST use pipe syntax like "A --|Label Text|--> B" or "A -- |Label Text| --> B". NEVER use double quotes like "A -- \\"Label\\" --> B" which is INVALID
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
        allowEdit: this.options.allowEdit,
        maxIterations: 10,  // Allow more iterations for mermaid fixing to handle complex diagrams
        disableMermaidValidation: true  // CRITICAL: Disable mermaid validation in nested agent to prevent infinite recursion
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

    // Format error context - handle both simple strings and maid's structured errors
    let errorContext = '';
    if (originalErrors.length > 0) {
      const formattedErrors = originalErrors.map(err => {
        // Check if this is a maid structured error object
        if (typeof err === 'object' && err.message) {
          const location = err.line ? `line ${err.line}${err.column ? `:${err.column}` : ''}` : '';
          const hint = err.hint ? `\n  Hint: ${err.hint}` : '';
          return location ? `- ${location}: ${err.message}${hint}` : `- ${err.message}${hint}`;
        }
        // Handle simple string errors
        return `- ${err}`;
      }).join('\n');

      errorContext = `\n\nDetected errors:\n${formattedErrors}`;
    }

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
      // Don't pass schema to avoid infinite loop where AI returns raw mermaid code
      // instead of using attempt_completion tool. The custom prompt already instructs
      // to return only mermaid code blocks.
      const result = await this.agent.answer(prompt, []);

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
 * Validate and fix Mermaid diagrams using maid
 * Replaces old manual fix logic with maid's auto-fix capabilities
 * @param {string} response - Response containing mermaid diagrams
 * @param {Object} options - Validation options
 * @returns {Promise<Object>} - Validation and fixing results
 */
export async function validateAndFixMermaidResponse(response, options = {}) {
  const { schema, debug, path, provider, model, tracer } = options;
  const startTime = Date.now();

  if (debug) {
    console.log(`[DEBUG] Mermaid validation: Starting maid-based validation for response (${response.length} chars)`);
  }

  // Record mermaid validation start in telemetry
  if (tracer) {
    tracer.recordMermaidValidationEvent('started', {
      'mermaid_validation.response_length': response.length,
      'mermaid_validation.provider': provider,
      'mermaid_validation.model': model,
      'mermaid_validation.method': 'maid'
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

  // If all valid, return early
  if (validation.isValid) {
    if (debug) {
      console.log(`[DEBUG] Mermaid validation: All diagrams valid, no fixing needed`);
    }

    if (tracer) {
      tracer.recordMermaidValidationEvent('completed', {
        'mermaid_validation.success': true,
        'mermaid_validation.diagrams_found': validation.diagrams?.length || 0,
        'mermaid_validation.fixes_needed': false,
        'mermaid_validation.duration_ms': Date.now() - startTime
      });
    }

    return {
      ...validation,
      wasFixed: false,
      originalResponse: response,
      fixedResponse: response
    };
  }

  // If no diagrams found, return without fixing
  if (!validation.diagrams || validation.diagrams.length === 0) {
    if (debug) {
      console.log(`[DEBUG] Mermaid validation: No mermaid diagrams found in response`);
    }
    return {
      ...validation,
      wasFixed: false,
      originalResponse: response,
      fixedResponse: response
    };
  }

  // Try maid auto-fix for invalid diagrams
  const invalidCount = validation.diagrams.filter(d => !d.isValid).length;
  if (debug) {
    console.log(`[DEBUG] Mermaid validation: ${invalidCount} invalid diagrams, trying maid auto-fix...`);
  }

  try {
    let fixedResponse = response;
    const fixingResults = [];
    let maidFixesApplied = false;

    // Extract diagrams with position information
    const { diagrams } = extractMermaidFromMarkdown(response);

    // Process invalid diagrams in reverse to maintain indices
    const invalidDiagrams = validation.diagrams
      .map((result, index) => ({ ...result, originalIndex: index }))
      .filter(result => !result.isValid)
      .reverse();

    for (const invalidDiagram of invalidDiagrams) {
      const originalContent = invalidDiagram.content;

      // Try maid auto-fix
      const maidResult = await tryMaidAutoFix(originalContent, { debug });

      if (maidResult.errors.length === 0) {
        // Maid fixed it completely
        const originalDiagram = diagrams[invalidDiagram.originalIndex];
        const attributesStr = originalDiagram.attributes ? ` ${originalDiagram.attributes}` : '';
        const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${maidResult.fixed}\n\`\`\``;

        fixedResponse = fixedResponse.slice(0, originalDiagram.startIndex) +
                       newCodeBlock +
                       fixedResponse.slice(originalDiagram.endIndex);

        fixingResults.push({
          diagramIndex: invalidDiagram.originalIndex,
          wasFixed: true,
          originalContent: originalContent,
          fixedContent: maidResult.fixed,
          originalError: invalidDiagram.error,
          fixedWithMaid: true,
          fixLevel: maidResult.fixLevel
        });

        maidFixesApplied = true;

        if (debug) {
          console.log(`[DEBUG] Mermaid validation: Maid fixed diagram ${invalidDiagram.originalIndex + 1}`);
        }
      } else if (maidResult.wasFixed) {
        // Maid improved it but didn't fix everything - update content for AI fixing
        const originalDiagram = diagrams[invalidDiagram.originalIndex];
        const attributesStr = originalDiagram.attributes ? ` ${originalDiagram.attributes}` : '';
        const newCodeBlock = `\`\`\`mermaid${attributesStr}\n${maidResult.fixed}\n\`\`\``;

        fixedResponse = fixedResponse.slice(0, originalDiagram.startIndex) +
                       newCodeBlock +
                       fixedResponse.slice(originalDiagram.endIndex);

        fixingResults.push({
          diagramIndex: invalidDiagram.originalIndex,
          wasFixed: false,
          originalContent: originalContent,
          partiallyFixedContent: maidResult.fixed,
          originalError: invalidDiagram.error,
          remainingErrors: maidResult.errors,
          fixedWithMaid: 'partial',
          fixLevel: maidResult.fixLevel
        });

        maidFixesApplied = true;

        if (debug) {
          console.log(`[DEBUG] Mermaid validation: Maid partially fixed diagram ${invalidDiagram.originalIndex + 1}, ${maidResult.errors.length} errors remain`);
        }
      } else {
        // Maid couldn't fix it, keep track for AI fixing
        fixingResults.push({
          diagramIndex: invalidDiagram.originalIndex,
          wasFixed: false,
          originalContent: originalContent,
          originalError: invalidDiagram.error,
          maidErrors: maidResult.errors,
          fixedWithMaid: false
        });
      }
    }

    // Re-validate after maid fixes
    const revalidation = await validateMermaidResponse(fixedResponse);
    if (revalidation.isValid) {
      // All diagrams fixed with maid
      const totalTime = Date.now() - startTime;
      if (debug) {
        console.log(`[DEBUG] Mermaid validation: All diagrams fixed with maid in ${totalTime}ms, no AI needed`);
      }

      if (tracer) {
        tracer.recordMermaidValidationEvent('maid_fix_completed', {
          'mermaid_validation.success': true,
          'mermaid_validation.fix_method': 'maid',
          'mermaid_validation.diagrams_fixed': fixingResults.filter(r => r.wasFixed).length,
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
          diagramsProcessed: fixingResults.length,
          diagramsFixed: fixingResults.filter(r => r.wasFixed).length
        }
      };
    }

    // Still have invalid diagrams, proceed with AI fixing
    if (debug) {
      const stillInvalid = revalidation.diagrams.filter(d => !d.isValid).length;
      console.log(`[DEBUG] Mermaid validation: ${stillInvalid} diagrams still invalid after maid, starting AI fixing...`);
    }

    const aiFixingStart = Date.now();
    const mermaidFixer = new MermaidFixingAgent({
      path, provider, model, debug, tracer
    });

    // Extract updated diagrams and validation
    const { diagrams: updatedDiagrams } = extractMermaidFromMarkdown(fixedResponse);
    const stillInvalidDiagrams = revalidation.diagrams
      .map((result, index) => ({ ...result, originalIndex: index }))
      .filter(result => !result.isValid)
      .reverse();

    if (debug) {
      console.log(`[DEBUG] Mermaid validation: Found ${stillInvalidDiagrams.length} diagrams requiring AI fixing`);
    }

    for (const invalidDiagram of stillInvalidDiagrams) {
      if (debug) {
        console.log(`[DEBUG] Mermaid validation: Attempting AI fix for diagram ${invalidDiagram.originalIndex + 1}`);
      }

      const diagramFixStart = Date.now();
      try {
        // Pass maid's structured errors if available
        const errorsToPass = invalidDiagram.errors && invalidDiagram.errors.length > 0
          ? invalidDiagram.errors
          : [invalidDiagram.error];

        const fixedContent = await mermaidFixer.fixMermaidDiagram(
          invalidDiagram.content,
          errorsToPass,
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

          // Find existing result or create new one
          const existingResultIndex = fixingResults.findIndex(r => r.diagramIndex === invalidDiagram.originalIndex);
          if (existingResultIndex >= 0) {
            fixingResults[existingResultIndex] = {
              ...fixingResults[existingResultIndex],
              wasFixed: true,
              fixedContent: fixedContent,
              aiFixingTimeMs: diagramFixTime
            };
          } else {
            fixingResults.push({
              diagramIndex: invalidDiagram.originalIndex,
              wasFixed: true,
              originalContent: invalidDiagram.content,
              fixedContent: fixedContent,
              originalError: invalidDiagram.error,
              aiFixingTimeMs: diagramFixTime
            });
          }

          if (debug) {
            console.log(`[DEBUG] Mermaid validation: Successfully fixed diagram ${invalidDiagram.originalIndex + 1} with AI in ${diagramFixTime}ms`);
          }
        } else {
          if (debug) {
            console.log(`[DEBUG] Mermaid validation: AI fix failed for diagram ${invalidDiagram.originalIndex + 1} - no valid fix generated`);
          }
        }
      } catch (error) {
        if (debug) {
          console.log(`[DEBUG] Mermaid validation: AI fix failed for diagram ${invalidDiagram.originalIndex + 1}: ${error.message}`);
        }
      }
    }

    // Final validation
    const finalValidation = await validateMermaidResponse(fixedResponse);
    const totalTime = Date.now() - startTime;
    const aiFixingTime = Date.now() - aiFixingStart;

    const wasActuallyFixed = fixingResults.some(result => result.wasFixed);
    const fixedCount = fixingResults.filter(result => result.wasFixed).length;

    if (debug) {
      console.log(`[DEBUG] Mermaid validation: Total process time: ${totalTime}ms (AI fixing: ${aiFixingTime}ms)`);
      console.log(`[DEBUG] Mermaid validation: Fixed ${fixedCount}/${fixingResults.length} diagrams`);
      console.log(`[DEBUG] Mermaid validation: Final result - all valid: ${finalValidation.isValid}`);
    }

    if (tracer) {
      tracer.recordMermaidValidationEvent('completed', {
        'mermaid_validation.success': finalValidation.isValid,
        'mermaid_validation.was_fixed': wasActuallyFixed,
        'mermaid_validation.diagrams_processed': fixingResults.length,
        'mermaid_validation.diagrams_fixed': fixedCount,
        'mermaid_validation.total_duration_ms': totalTime,
        'mermaid_validation.ai_fixing_duration_ms': aiFixingTime
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
        diagramsProcessed: fixingResults.length,
        diagramsFixed: fixedCount
      },
      tokenUsage: mermaidFixer.getTokenUsage()
    };

  } catch (error) {
    if (debug) {
      console.error(`[DEBUG] Mermaid fixing failed: ${error.message}`);
    }

    return {
      ...validation,
      wasFixed: false,
      originalResponse: response,
      fixedResponse: response,
      fixingError: error.message
    };
  }
}
