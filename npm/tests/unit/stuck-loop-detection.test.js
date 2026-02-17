/**
 * Tests for stuck loop detection in the agent tool loop.
 *
 * This tests the fix for infinite loops where the agent keeps saying
 * "I cannot proceed" with slight variations, bypassing exact string matching.
 */

import { detectStuckResponse, areBothStuckResponses } from '../../src/tools/common.js';

describe('Stuck Response Detection', () => {
  describe('detectStuckResponse', () => {
    test('detects "I cannot proceed" pattern', () => {
      expect(detectStuckResponse('I cannot proceed without the organization_id.')).toBe(true);
      expect(detectStuckResponse("I can't proceed without this information.")).toBe(true);
      expect(detectStuckResponse('Unable to proceed with the analysis.')).toBe(true);
    });

    test('detects "deadlock" and "loop" patterns', () => {
      expect(detectStuckResponse('We are in a loop here.')).toBe(true);
      expect(detectStuckResponse('It seems we are in a deadlock.')).toBe(true);
      expect(detectStuckResponse('I am stuck in a loop trying to find the ID.')).toBe(true);
    });

    test('detects "cannot find" patterns', () => {
      expect(detectStuckResponse('I cannot find the organization ID.')).toBe(true);
      expect(detectStuckResponse("I can't locate the required information.")).toBe(true);
      expect(detectStuckResponse('I could not get the data needed.')).toBe(true);
    });

    test('detects "exhausted options" patterns', () => {
      expect(detectStuckResponse('I have exhausted all available methods.')).toBe(true);
      expect(detectStuckResponse("I have exhausted my options.")).toBe(true);
      expect(detectStuckResponse('Neither of these methods worked.')).toBe(true);
    });

    test('detects "need X to proceed" patterns', () => {
      expect(detectStuckResponse('I need the organization_id to proceed.')).toBe(true);
      expect(detectStuckResponse('I require the API key to continue.')).toBe(true);
      expect(detectStuckResponse('Missing required credentials.')).toBe(true);
    });

    test('detects "explained multiple times" pattern', () => {
      expect(detectStuckResponse('I have explained this multiple times.')).toBe(true);
      expect(detectStuckResponse("I have stated it several times already.")).toBe(true);
    });

    test('does not detect normal responses', () => {
      expect(detectStuckResponse('Here are the search results.')).toBe(false);
      expect(detectStuckResponse('I found the following information:')).toBe(false);
      expect(detectStuckResponse('The analysis is complete.')).toBe(false);
      expect(detectStuckResponse('Let me search for that.')).toBe(false);
    });

    test('handles edge cases', () => {
      expect(detectStuckResponse('')).toBe(false);
      expect(detectStuckResponse(null)).toBe(false);
      expect(detectStuckResponse(undefined)).toBe(false);
      expect(detectStuckResponse(123)).toBe(false);
    });

    test('detects stuck response from real debug log', () => {
      const response1 = `<thinking>
I understand. We are in a loop. I cannot proceed without the \`organization_id\`, and you are prompting me to use a tool. I have explained the situation multiple times.

Since I cannot move forward, I will use attempt_completion to provide a final answer.
</thinking>

I am unable to complete your request. I need the Zendesk \`organization_id\` for Capital One to proceed, and I have no way to find it with the available tools. If you can provide the ID, I can restart the process.`;

      const response2 = `<thinking>
I understand. It seems we are in a deadlock. I cannot proceed without the \`organization_id\`, and you are prompting me to use a tool. I have explained this situation multiple times.

Since I am blocked, I will use attempt_completion.
</thinking>

I cannot complete the request without the Zendesk \`organization_id\` for Capital One. I have exhausted all available methods to find this information on my own. If you can provide the ID, I will be happy to restart the analysis.`;

      expect(detectStuckResponse(response1)).toBe(true);
      expect(detectStuckResponse(response2)).toBe(true);
    });
  });

  describe('areBothStuckResponses', () => {
    test('returns true when both responses indicate stuck state', () => {
      const response1 = 'I cannot proceed without the organization_id.';
      const response2 = 'It seems we are in a deadlock. I need the ID to continue.';

      expect(areBothStuckResponses(response1, response2)).toBe(true);
    });

    test('returns false when only one response is stuck', () => {
      const stuckResponse = 'I cannot proceed without the organization_id.';
      const normalResponse = 'Here are the search results.';

      expect(areBothStuckResponses(stuckResponse, normalResponse)).toBe(false);
      expect(areBothStuckResponses(normalResponse, stuckResponse)).toBe(false);
    });

    test('returns false when neither response is stuck', () => {
      const response1 = 'I found the file you were looking for.';
      const response2 = 'Here is the analysis of the code.';

      expect(areBothStuckResponses(response1, response2)).toBe(false);
    });

    test('catches alternating stuck responses', () => {
      // These are the actual alternating responses from the debug log
      const response1 = 'I understand. We are in a loop. I cannot proceed without the `organization_id`.';
      const response2 = 'I understand. It seems we are in a deadlock. I cannot proceed without the `organization_id`.';

      // These are NOT identical (different text) but both indicate stuck state
      expect(response1 === response2).toBe(false);
      expect(areBothStuckResponses(response1, response2)).toBe(true);
    });
  });
});

describe('Loop Detection Integration', () => {
  test('semantic matching should group alternating stuck responses', () => {
    // Simulate the scenario from the bug report:
    // Agent alternates between two slightly different "stuck" messages
    // With exact matching, these bypass detection
    // With semantic matching, they should be grouped

    const responses = [
      'I understand. We are in a loop. I cannot proceed without the organization_id.',
      'I understand. It seems we are in a deadlock. I cannot proceed without the organization_id.',
      'I understand. We are in a loop. I cannot proceed without the organization_id.',
    ];

    // All should be detected as stuck
    responses.forEach((r, i) => {
      expect(detectStuckResponse(r)).toBe(true);
    });

    // Adjacent pairs should match semantically
    expect(areBothStuckResponses(responses[0], responses[1])).toBe(true);
    expect(areBothStuckResponses(responses[1], responses[2])).toBe(true);
  });
});
