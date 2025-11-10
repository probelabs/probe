/**
 * Context Window Compactor
 *
 * Handles context window overflow by intelligently removing intermediate agentic
 * monologue sections while preserving user messages, final answers, and the most
 * recent monologue.
 */

/**
 * Regex pattern to detect context window limit errors from various AI providers
 * Matches patterns like:
 * - "context length exceeded"
 * - "input token count exceeds limit"
 * - "maximum context window"
 * - "tokens exceed limit"
 * - "maximum context length is X tokens"
 * etc.
 *
 * Flags: i = case insensitive, s = dot matches newlines
 */
const CONTEXT_LIMIT_ERROR_REGEX = /(context(?:\s+(?:length|window))|input\s+token|(?:number|total)\s+of\s+tokens|tokens?|prompt|maximum\s+context\s+length).{0,100}?(context_length_exceeded|exceeds?|exceeded|exceeding|too\s+(?:long|large)|over(?:\s+the)?\s+limit|maximum|allowed|tokens)/is;

/**
 * Check if an error message indicates a context window limit was exceeded
 * @param {Error|string} error - The error object or error message
 * @returns {boolean} - True if the error indicates context limit exceeded
 */
export function isContextLimitError(error) {
  const errorMessage = typeof error === 'string' ? error : (error?.message || '');
  return CONTEXT_LIMIT_ERROR_REGEX.test(errorMessage);
}

/**
 * Identify message boundaries in conversation history
 * Structure: <user> -> <internal agentic monologue> -> <final-agent-answer>
 *
 * A "segment" is:
 * - user message (role: 'user')
 * - followed by 0+ assistant messages (internal monologue)
 * - ending with tool_result or attempt_completion (final answer)
 *
 * @param {Array} messages - Array of message objects with {role, content}
 * @returns {Array} - Array of segments, each containing {userIndex, monologueIndices, finalIndex}
 */
export function identifyMessageSegments(messages) {
  const segments = [];
  let currentSegment = null;

  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];

    // Skip system messages
    if (msg.role === 'system') {
      continue;
    }

    // User message starts a new segment
    if (msg.role === 'user') {
      // Check if this is a tool_result (final answer from previous segment)
      const content = typeof msg.content === 'string' ? msg.content : '';
      const isToolResult = content.includes('<tool_result>');

      if (isToolResult && currentSegment) {
        // This is the final answer for the current segment
        currentSegment.finalIndex = i;
        segments.push(currentSegment);
        currentSegment = null;
      } else {
        // Save previous segment if it exists
        if (currentSegment) {
          segments.push(currentSegment);
        }

        // Start new segment
        currentSegment = {
          userIndex: i,
          monologueIndices: [],
          finalIndex: null
        };
      }
    }

    // Assistant message is part of monologue
    if (msg.role === 'assistant' && currentSegment) {
      const content = typeof msg.content === 'string' ? msg.content : '';

      // Check if this contains attempt_completion (marks end of segment)
      if (content.includes('<attempt_completion>') || content.includes('attempt_completion')) {
        currentSegment.monologueIndices.push(i);
        currentSegment.finalIndex = i;
        segments.push(currentSegment);
        currentSegment = null;
      } else {
        // Regular monologue message
        currentSegment.monologueIndices.push(i);
      }
    }
  }

  // Save any remaining segment
  if (currentSegment) {
    segments.push(currentSegment);
  }

  return segments;
}

/**
 * Compact messages by removing intermediate monologues
 *
 * Strategy:
 * 1. Keep all user messages
 * 2. Keep all final answers (tool_results, attempt_completion)
 * 3. Remove intermediate monologue messages from completed segments
 * 4. Keep the most recent (active) segment intact
 *
 * @param {Array} messages - Array of message objects
 * @param {Object} options - Compaction options
 * @param {boolean} [options.keepLastSegment=true] - Keep the most recent segment intact
 * @param {number} [options.minSegmentsToKeep=1] - Minimum number of recent segments to preserve fully
 * @returns {Array} - Compacted message array
 */
export function compactMessages(messages, options = {}) {
  const {
    keepLastSegment = true,
    minSegmentsToKeep = 1
  } = options;

  if (!messages || messages.length === 0) {
    return messages;
  }

  // Identify segments
  const segments = identifyMessageSegments(messages);

  if (segments.length === 0) {
    return messages;
  }

  // Determine which segments to keep fully vs compact
  const segmentsToPreserve = keepLastSegment
    ? Math.max(minSegmentsToKeep, 1)
    : minSegmentsToKeep;

  const compactableSegments = segments.slice(0, -segmentsToPreserve);
  const preservedSegments = segments.slice(-segmentsToPreserve);

  // Build set of indices to keep
  const indicesToKeep = new Set();

  // Keep system messages
  messages.forEach((msg, idx) => {
    if (msg.role === 'system') {
      indicesToKeep.add(idx);
    }
  });

  // For compactable segments: keep user message and final answer only
  compactableSegments.forEach(segment => {
    indicesToKeep.add(segment.userIndex);
    if (segment.finalIndex !== null) {
      indicesToKeep.add(segment.finalIndex);
    }
  });

  // For preserved segments: keep everything
  preservedSegments.forEach(segment => {
    indicesToKeep.add(segment.userIndex);
    segment.monologueIndices.forEach(idx => indicesToKeep.add(idx));
    if (segment.finalIndex !== null) {
      indicesToKeep.add(segment.finalIndex);
    }
  });

  // Filter messages
  const compactedMessages = messages.filter((_, idx) => indicesToKeep.has(idx));

  return compactedMessages;
}

/**
 * Calculate reduction statistics
 * @param {Array} originalMessages - Original message array
 * @param {Array} compactedMessages - Compacted message array
 * @returns {Object} - Statistics about the compaction
 */
export function calculateCompactionStats(originalMessages, compactedMessages) {
  const originalCount = originalMessages.length;
  const compactedCount = compactedMessages.length;
  const removed = originalCount - compactedCount;
  const reductionPercent = originalCount > 0
    ? ((removed / originalCount) * 100).toFixed(1)
    : 0;

  // Estimate token savings (rough approximation)
  const estimateTokens = (msgs) => {
    return msgs.reduce((sum, msg) => {
      const content = typeof msg.content === 'string'
        ? msg.content
        : JSON.stringify(msg.content);
      // Rough estimate: 1 token ≈ 4 characters
      return sum + Math.ceil(content.length / 4);
    }, 0);
  };

  const originalTokens = estimateTokens(originalMessages);
  const compactedTokens = estimateTokens(compactedMessages);
  const tokensSaved = originalTokens - compactedTokens;

  return {
    originalCount,
    compactedCount,
    removed,
    reductionPercent: parseFloat(reductionPercent),
    originalTokens,
    compactedTokens,
    tokensSaved
  };
}

/**
 * Main compaction handler for ProbeAgent
 * Detects context limit errors and performs intelligent compaction
 *
 * @param {Error} error - The error from the AI provider
 * @param {Array} messages - Current message array
 * @param {Object} options - Compaction options
 * @returns {Object|null} - { compacted: true, messages, stats } or null if not applicable
 */
export function handleContextLimitError(error, messages, options = {}) {
  if (!isContextLimitError(error)) {
    return null;
  }

  const compactedMessages = compactMessages(messages, options);
  const stats = calculateCompactionStats(messages, compactedMessages);

  return {
    compacted: true,
    messages: compactedMessages,
    stats
  };
}
