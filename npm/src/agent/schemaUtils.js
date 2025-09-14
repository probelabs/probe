/**
 * Utility functions for cleaning and validating schema responses from AI models
 * Supports JSON and Mermaid diagram validation
 */

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
  
  // Find JSON boundaries
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
    // Check if the response likely starts with JSON (directly or after markdown)
    const beforeFirstBracket = trimmed.substring(0, firstBracket).trim();
    
    // If there's minimal content before the first bracket (just markdown wrapper),
    // extract the JSON. Otherwise, return original to preserve non-JSON content.
    if (beforeFirstBracket === '' || beforeFirstBracket.match(/^```\w*$/)) {
      return trimmed.substring(firstBracket, lastBracket + 1);
    }
  }
  
  return response; // Return original if no extractable JSON found
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
4. **Preserve semantic meaning** - never change the intended flow or relationships
5. **Use proper escaping** for special characters and spaces
6. **Ensure consistency** in naming conventions and formatting

CRITICAL RULES:
- ALWAYS output only the corrected Mermaid code within a \`\`\`mermaid code block
- NEVER add explanations, comments, or additional text outside the code block
- PRESERVE the original diagram's intended meaning and flow
- FIX syntax errors without changing the logical structure
- ENSURE the output is valid, parseable Mermaid syntax

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
    await this.initializeAgent();

    const errorContext = originalErrors.length > 0 
      ? `\n\nDetected errors: ${originalErrors.join(', ')}` 
      : '';

    const diagramTypeHint = diagramInfo.diagramType 
      ? `\n\nExpected diagram type: ${diagramInfo.diagramType}` 
      : '';

    const prompt = `Analyze and fix the following Mermaid diagram.${errorContext}${diagramTypeHint}

Broken Mermaid diagram:
\`\`\`mermaid
${diagramContent}
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
  
  // First, run standard validation
  const validation = await validateMermaidResponse(response);
  
  if (validation.isValid) {
    // All diagrams are valid, no fixing needed
    return {
      ...validation,
      wasFixed: false,
      originalResponse: response,
      fixedResponse: response
    };
  }

  // If no diagrams found at all, return without attempting to fix
  if (!validation.diagrams || validation.diagrams.length === 0) {
    return {
      ...validation,
      wasFixed: false,
      originalResponse: response,
      fixedResponse: response
    };
  }

  // Some diagrams are invalid, try to fix them with specialized agent
  if (debug) {
    console.error('[DEBUG] Invalid Mermaid diagrams detected, starting specialized fixing agent...');
  }

  try {
    // Create specialized fixing agent
    const mermaidFixer = new MermaidFixingAgent({
      path, provider, model, debug, tracer
    });

    let fixedResponse = response;
    const fixingResults = [];
    
    // Extract diagrams with position information for replacement
    const { diagrams } = extractMermaidFromMarkdown(response);
    
    // Fix invalid diagrams in reverse order to preserve indices
    const invalidDiagrams = validation.diagrams
      .map((result, index) => ({ ...result, originalIndex: index }))
      .filter(result => !result.isValid)
      .reverse();

    for (const invalidDiagram of invalidDiagrams) {
      try {
        const fixedContent = await mermaidFixer.fixMermaidDiagram(
          invalidDiagram.content,
          [invalidDiagram.error],
          { diagramType: invalidDiagram.diagramType }
        );

        if (fixedContent && fixedContent !== invalidDiagram.content) {
          // Replace the diagram in the response
          const originalDiagram = diagrams[invalidDiagram.originalIndex];
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
            originalError: invalidDiagram.error
          });

          if (debug) {
            console.error(`[DEBUG] Fixed diagram ${invalidDiagram.originalIndex + 1}: ${invalidDiagram.error}`);
          }
        } else {
          fixingResults.push({
            diagramIndex: invalidDiagram.originalIndex,
            wasFixed: false,
            originalContent: invalidDiagram.content,
            originalError: invalidDiagram.error,
            fixingError: 'No valid fix generated'
          });
        }
      } catch (error) {
        fixingResults.push({
          diagramIndex: invalidDiagram.originalIndex,
          wasFixed: false,
          originalContent: invalidDiagram.content,
          originalError: invalidDiagram.error,
          fixingError: error.message
        });

        if (debug) {
          console.error(`[DEBUG] Failed to fix diagram ${invalidDiagram.originalIndex + 1}: ${error.message}`);
        }
      }
    }

    // Re-validate the fixed response
    const finalValidation = await validateMermaidResponse(fixedResponse);

    // Check if any diagrams were actually fixed
    const wasActuallyFixed = fixingResults.some(result => result.wasFixed);

    return {
      ...finalValidation,
      wasFixed: wasActuallyFixed,
      originalResponse: response,
      fixedResponse: fixedResponse,
      fixingResults: fixingResults,
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