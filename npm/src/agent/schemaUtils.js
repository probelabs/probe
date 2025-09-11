/**
 * Utility functions for cleaning and validating schema responses from AI models
 * Supports JSON and Mermaid diagram validation
 */

/**
 * Clean AI response by removing code blocks and format indicators
 * @param {string} response - Raw AI response
 * @returns {string} - Cleaned response
 */
export function cleanSchemaResponse(response) {
  if (!response || typeof response !== 'string') {
    return response;
  }

  let cleaned = response.trim();

  // Remove all markdown code blocks (including multiple blocks)
  // This handles ```json, ```xml, ```yaml, etc. and plain ```
  cleaned = cleaned.replace(/```\w*\n?/g, '');
  cleaned = cleaned.replace(/\n?```/g, '');

  // Remove any remaining leading/trailing whitespace
  cleaned = cleaned.trim();

  // Handle case where AI wrapped response in backticks without newlines
  // e.g., `{"result": "value"}` 
  if (cleaned.startsWith('`') && cleaned.endsWith('`')) {
    cleaned = cleaned.slice(1, -1).trim();
  }

  return cleaned;
}

/**
 * Validate that the cleaned response is valid JSON if expected
 * @param {string} response - Cleaned response
 * @returns {Object} - {isValid: boolean, parsed?: Object, error?: string}
 */
export function validateJsonResponse(response) {
  try {
    const parsed = JSON.parse(response);
    return { isValid: true, parsed };
  } catch (error) {
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

  // Clean the response
  const cleaned = cleanSchemaResponse(response);

  const result = { cleaned };

  if (debug) {
    result.debug = {
      originalLength: response.length,
      cleanedLength: cleaned.length,
      wasModified: response !== cleaned,
      removedContent: response !== cleaned ? {
        before: response.substring(0, 50) + (response.length > 50 ? '...' : ''),
        after: cleaned.substring(0, 50) + (cleaned.length > 50 ? '...' : '')
      } : null
    };
  }

  // Optional validation
  if (validateJson) {
    result.jsonValidation = validateJsonResponse(cleaned);
  }

  if (validateXml) {
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
 * Create a correction prompt for invalid JSON
 * @param {string} invalidResponse - The invalid JSON response
 * @param {string} schema - The original schema
 * @param {string} error - The JSON parsing error
 * @param {string} [detailedError] - Additional error details
 * @returns {string} - Correction prompt for the AI
 */
export function createJsonCorrectionPrompt(invalidResponse, schema, error, detailedError = '') {
  let prompt = `Your previous response is not valid JSON and cannot be parsed. Here's what you returned:

${invalidResponse}

Error: ${error}`;

  if (detailedError && detailedError !== error) {
    prompt += `\nDetailed Error: ${detailedError}`;
  }

  prompt += `

Please correct your response to be valid JSON that matches this schema:
${schema}

Return ONLY the corrected JSON, with no additional text or markdown formatting.`;

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
 * Extract Mermaid diagrams from markdown code blocks
 * @param {string} response - Response that may contain markdown with mermaid blocks
 * @returns {Object} - {diagrams: string[], cleanedResponse: string}
 */
export function extractMermaidFromMarkdown(response) {
  if (!response || typeof response !== 'string') {
    return { diagrams: [], cleanedResponse: response };
  }

  // Find all mermaid code blocks
  const mermaidBlockRegex = /```mermaid\s*\n([\s\S]*?)\n```/gi;
  const diagrams = [];
  let match;

  while ((match = mermaidBlockRegex.exec(response)) !== null) {
    diagrams.push(match[1].trim());
  }

  // Return cleaned response (original for now, could be modified if needed)
  return { diagrams, cleanedResponse: response };
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

    // Basic syntax validation based on diagram type
    const lines = trimmedDiagram.split('\n');
    
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (!line) continue;
      
      // Check for severely malformed syntax that would break parsing
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
    const validation = await validateMermaidDiagram(diagrams[i]);
    results.push({
      diagram: diagrams[i],
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
        prompt += `\n- Content: ${diagramResult.diagram.substring(0, 100)}${diagramResult.diagram.length > 100 ? '...' : ''}`;
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