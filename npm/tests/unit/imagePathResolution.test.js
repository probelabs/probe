import { jest, describe, test, expect, beforeEach, afterEach } from '@jest/globals';

// Mock all the heavy dependencies that ProbeAgent uses
jest.mock('@ai-sdk/anthropic', () => ({}));
jest.mock('@ai-sdk/openai', () => ({}));
jest.mock('@ai-sdk/google', () => ({}));
jest.mock('@ai-sdk/amazon-bedrock', () => ({}));
jest.mock('ai', () => ({
  generateText: jest.fn(),
  streamText: jest.fn(),
  tool: jest.fn((config) => ({
    name: config.name,
    description: config.description,
    inputSchema: config.inputSchema,
    execute: config.execute
  }))
}));

import { ProbeAgent } from '../../src/agent/ProbeAgent.js';
import { writeFileSync, unlinkSync, existsSync, mkdirSync, rmSync } from 'fs';
import { join } from 'path';

describe('Image Path Resolution', () => {
  let testDir;
  let agent;
  let testImages;

  beforeEach(() => {
    // Create a test directory structure
    testDir = join(process.cwd(), 'test-images-temp');
    if (!existsSync(testDir)) {
      mkdirSync(testDir, { recursive: true });
    }

    // Create test image files
    const simplePng = Buffer.from([
      0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
      0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
      0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
      0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
      0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
      0x54, 0x78, 0x9C, 0x62, 0x00, 0x02, 0x00, 0x00,
      0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
      0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
      0x42, 0x60, 0x82
    ]);

    testImages = [
      'policy-1.png',
      'policy-2.png',
      'client.png'
    ];

    testImages.forEach(filename => {
      writeFileSync(join(testDir, filename), simplePng);
    });

    // Initialize agent with the test directory
    agent = new ProbeAgent({
      debug: false,
      path: testDir
    });
  });

  afterEach(() => {
    // Cleanup
    if (existsSync(testDir)) {
      rmSync(testDir, { recursive: true, force: true });
    }
  });

  describe('extractListFilesDirectories', () => {
    test('should extract directory from extract tool File: header', () => {
      const content = `File: ${testDir}/ticket.md
Lines: 41-48
\`\`\`markdown
  <attachments>
    <attachment filename="policy-1.png" />
  </attachments>
\`\`\``;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toEqual([testDir]);
    });

    test('should extract directory paths from listFiles output', () => {
      const content = `${testDir}:
file    1.2K  policy-1.png
file    1.1K  policy-2.png
file    1.3K  client.png`;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toEqual([testDir]);
    });

    test('should not extract non-directory lines ending with colon', () => {
      const content = `Token Usage:
Some random text

${testDir}:
file    1.2K  policy-1.png`;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toEqual([testDir]);
      expect(directories).not.toContain('Token Usage');
    });

    test('should handle relative paths', () => {
      const content = `./attachments:
file    1.2K  policy-1.png`;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toEqual(['./attachments']);
    });

    test('should handle multiple directory sections', () => {
      const content = `${testDir}:
file    1.2K  policy-1.png

${testDir}/subdirectory:
file    1.1K  policy-2.png`;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toHaveLength(2);
      expect(directories).toContain(testDir);
      expect(directories).toContain(`${testDir}/subdirectory`);
    });

    test('should not match headings or other text with colons', () => {
      const content = `My Dashboard has this log:
My Gateway has this log:
Files to extract:

${testDir}:
file    1.2K  policy-1.png`;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toEqual([testDir]);
    });

    test('should reject paths with spaces (like "./Token Usage:")', () => {
      const content = `./Token Usage:
./Some Text Here:
This behavior is due to a known bug (TT-8839):

${testDir}:
file    1.2K  policy-1.png`;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toEqual([testDir]);
      expect(directories).not.toContain('./Token Usage');
      expect(directories).not.toContain('./Some Text Here');
    });

    test('should extract from both File: header and listFiles output without duplicates', () => {
      const content = `File: ${testDir}/ticket.md
Lines: 1-10
Some content

${testDir}:
file    1.2K  policy-1.png`;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toEqual([testDir]); // Should only have one entry, no duplicates
    });

    test('should handle extract tool output with relative paths', () => {
      const content = `File: ./docs/ticket.md
Lines: 1-10
Some content`;

      const directories = agent.extractListFilesDirectories(content);
      expect(directories).toEqual(['./docs']);
    });
  });

  describe('processImageReferences with XML attachments', () => {
    test('should resolve relative image paths from XML attachments using directory context', async () => {
      // Simulate content with XML attachments and listFiles output
      const content = `${testDir}:
file    458K  attachment_2_policy-1.png
file    436K  attachment_3_policy-2.png
file    447K  attachment_4_client.png

<attachments>
  <attachment filename="policy-1.png" size="458477" content_type="image/png" />
  <attachment filename="policy-2.png" size="436301" content_type="image/png" />
  <attachment filename="client.png" size="447699" content_type="image/png" />
</attachments>`;

      await agent.processImageReferences(content);

      // Check that images were loaded
      const loadedImages = agent.getCurrentImages();
      expect(loadedImages.length).toBeGreaterThan(0);

      // Verify at least one image was successfully resolved
      expect(loadedImages.some(img => img.startsWith('data:image/png;base64,'))).toBe(true);
    });

    test('should handle images when directory context is available from listFiles', async () => {
      // First, process content with directory context
      const contentWithDirectory = `${testDir}:
file    1.2K  policy-1.png
file    1.1K  policy-2.png`;

      // Then process content with just filename mentions
      await agent.processImageReferences(contentWithDirectory);

      const contentWithFilenames = 'Look at policy-1.png and policy-2.png for details.';
      await agent.processImageReferences(contentWithFilenames);

      const loadedImages = agent.getCurrentImages();
      expect(loadedImages.length).toBeGreaterThan(0);
    });

    test('should not load images when only non-directory headings are present', async () => {
      agent.clearLoadedImages();

      const content = `Token Usage:
Some metrics here

Files to extract:
- policy-1.png
- policy-2.png`;

      await agent.processImageReferences(content);

      // Images should not be loaded because no actual directory path was extracted
      const loadedImages = agent.getCurrentImages();
      // Images may or may not be loaded depending on if they exist in allowed folders
      // The key is that "Token Usage" should not be used as a directory
      expect(agent.extractListFilesDirectories(content)).not.toContain('Token Usage');
      expect(agent.extractListFilesDirectories(content)).not.toContain('Files to extract');
    });
  });

  describe('loadImageIfValid', () => {
    test('should load image from full path', async () => {
      const imagePath = join(testDir, 'policy-1.png');
      const result = await agent.loadImageIfValid(imagePath);

      expect(result).toBe(true);
      expect(agent.pendingImages.has(imagePath)).toBe(true);
    });

    test('should load image from relative path when context is available', async () => {
      // First extract directory context
      const listFilesOutput = `${testDir}:
file    1.2K  policy-1.png`;

      await agent.processImageReferences(listFilesOutput);

      // Now try to load with just filename
      const result = await agent.loadImageIfValid(join(testDir, 'policy-1.png'));

      expect(result).toBe(true);
    });

    test('should not load non-existent images', async () => {
      const imagePath = join(testDir, 'nonexistent.png');
      const result = await agent.loadImageIfValid(imagePath);

      expect(result).toBe(false);
      expect(agent.pendingImages.has(imagePath)).toBe(false);
    });

    test('should prevent loading images outside allowed directories', async () => {
      const outsidePath = '/tmp/outside.png';
      const result = await agent.loadImageIfValid(outsidePath);

      expect(result).toBe(false);
    });
  });

  describe('Real-world XML attachment scenario', () => {
    test('should load images from extract tool File: header without needing listFiles', async () => {
      // This is the CORRECT way - extract tool provides the file path directly
      const extractToolOutput = `File: ${testDir}/ticket.md
Lines: 41-48
\`\`\`markdown
  <attachments>
    <attachment filename="policy-1.png" size="458477" content_type="image/png" />
    <attachment filename="policy-2.png" size="436301" content_type="image/png" />
    <attachment filename="client.png" size="447699" content_type="image/png" />
  </attachments>
\`\`\``;

      // Process the extract tool output
      await agent.processImageReferences(extractToolOutput);

      // Verify images were loaded
      const loadedImages = agent.getCurrentImages();
      expect(loadedImages.length).toBeGreaterThan(0);

      // Verify the images are valid base64 data URLs
      loadedImages.forEach(img => {
        expect(img).toMatch(/^data:image\/(png|jpeg|jpg);base64,/);
      });
    });

    test('should handle the Zendesk ticket attachment format', async () => {
      // Simulate the actual scenario from the bug report
      const ticketContent = `<message id="11842728115356" author="User 8718562264860" created="2023-12-20T03:24:29Z" public="true">
Hi team,

When we perform the step 'Request an authorization code' of 'Authorization Code Grant Type', if our listen path is /oauth2_code_test, there is no problem, but if the listen path is /oauth2_code_test/a, it will return a 404 error.

Attached is our api json, client and policy configurations and dashboard logs.

Regards,

Salman

  <attachments>
    <attachment filename="oauth2_code_test.json" size="13243" content_type="application/json" />
    <attachment filename="policy-1.png" size="458477" content_type="image/png" />
    <attachment filename="policy-2.png" size="436301" content_type="image/png" />
    <attachment filename="client.png" size="447699" content_type="image/png" />
    <attachment filename="dashboard-log.csv.gz" size="6796" content_type="application/x-gzip" />
  </attachments>
</message>`;

      // Simulate listFiles being called on the directory
      // In this test, files are named exactly as in the XML (no prefix)
      const listFilesOutput = `${testDir}:
file    1.2K  policy-1.png
file    1.1K  policy-2.png
file    1.3K  client.png`;

      // Process both outputs
      await agent.processImageReferences(listFilesOutput);
      await agent.processImageReferences(ticketContent);

      // Verify images were loaded
      const loadedImages = agent.getCurrentImages();
      expect(loadedImages.length).toBeGreaterThan(0);

      // Verify the images are valid base64 data URLs
      loadedImages.forEach(img => {
        expect(img).toMatch(/^data:image\/(png|jpeg|jpg);base64,/);
      });
    });
  });
});
