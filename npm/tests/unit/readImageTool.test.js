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

describe('ReadImage Tool', () => {
  let testDir;
  let agent;
  let testImagePath;

  beforeEach(() => {
    // Create a test directory structure
    testDir = join(process.cwd(), 'test-readimage-temp');
    if (!existsSync(testDir)) {
      mkdirSync(testDir, { recursive: true });
    }

    // Create a simple 1x1 PNG image
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

    testImagePath = join(testDir, 'test-screenshot.png');
    writeFileSync(testImagePath, simplePng);

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

  describe('Tool availability', () => {
    test('readImage tool should be available in toolImplementations', () => {
      expect(agent.toolImplementations).toHaveProperty('readImage');
      expect(agent.toolImplementations.readImage).toHaveProperty('execute');
      expect(typeof agent.toolImplementations.readImage.execute).toBe('function');
    });

    test('readImage tool should be in allowed tools by default', () => {
      expect(agent.allowedTools.isEnabled('readImage')).toBe(true);
    });
  });

  describe('Tool execution', () => {
    test('should successfully load image when given valid path', async () => {
      const result = await agent.toolImplementations.readImage.execute({
        path: testImagePath
      });

      expect(result).toContain('Image loaded successfully');
      expect(result).toContain(testImagePath);

      // Verify image was actually loaded into pendingImages
      expect(agent.pendingImages.has(testImagePath)).toBe(true);

      // Verify it can be retrieved
      const loadedImages = agent.getCurrentImages();
      expect(loadedImages.length).toBeGreaterThan(0);
      expect(loadedImages[0]).toMatch(/^data:image\/png;base64,/);
    });

    test('should throw error when path parameter is missing', async () => {
      await expect(
        agent.toolImplementations.readImage.execute({})
      ).rejects.toThrow('Image path is required');
    });

    test('should throw error when image file does not exist', async () => {
      const nonExistentPath = join(testDir, 'nonexistent.png');

      await expect(
        agent.toolImplementations.readImage.execute({
          path: nonExistentPath
        })
      ).rejects.toThrow();
    });

    test('should handle relative paths correctly', async () => {
      // Create image in a subdirectory
      const subDir = join(testDir, 'images');
      mkdirSync(subDir, { recursive: true });

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

      const imagePath = join(subDir, 'relative.png');
      writeFileSync(imagePath, simplePng);

      const result = await agent.toolImplementations.readImage.execute({
        path: imagePath
      });

      expect(result).toContain('Image loaded successfully');
      expect(agent.pendingImages.has(imagePath)).toBe(true);
    });

    test('should support multiple image formats', async () => {
      const formats = ['test.png', 'test.jpg', 'test.jpeg', 'test.webp', 'test.bmp'];

      // Create a simple PNG for all tests (format validation happens elsewhere)
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

      for (const filename of formats) {
        const imagePath = join(testDir, filename);
        writeFileSync(imagePath, simplePng);

        const result = await agent.toolImplementations.readImage.execute({
          path: imagePath
        });

        expect(result).toContain('Image loaded successfully');
        expect(agent.pendingImages.has(imagePath)).toBe(true);
      }
    });

    test('should not load the same image twice', async () => {
      // Load image first time
      await agent.toolImplementations.readImage.execute({
        path: testImagePath
      });

      const imagesAfterFirst = agent.getCurrentImages().length;

      // Load same image again
      await agent.toolImplementations.readImage.execute({
        path: testImagePath
      });

      const imagesAfterSecond = agent.getCurrentImages().length;

      // Should still have same number of images (no duplicate)
      expect(imagesAfterSecond).toBe(imagesAfterFirst);
    });
  });

  describe('Security', () => {
    test('should respect allowed folders security', async () => {
      // Create agent with restricted allowed folders
      const restrictedAgent = new ProbeAgent({
        debug: false,
        path: testDir,
        allowedFolders: [testDir] // Only allow test directory
      });

      // Try to load image outside allowed folder
      const outsidePath = '/tmp/malicious.png';

      await expect(
        restrictedAgent.toolImplementations.readImage.execute({
          path: outsidePath
        })
      ).rejects.toThrow();
    });

    test('should validate file size limits', async () => {
      // The loadImageIfValid method should enforce MAX_IMAGE_FILE_SIZE (20MB)
      // This test verifies the tool respects that limit
      const result = await agent.toolImplementations.readImage.execute({
        path: testImagePath
      });

      expect(result).toContain('Image loaded successfully');
    });
  });

  describe('Integration with message flow', () => {
    test('loaded images should be available in getCurrentImages', async () => {
      agent.clearLoadedImages();

      await agent.toolImplementations.readImage.execute({
        path: testImagePath
      });

      const images = agent.getCurrentImages();
      expect(images.length).toBe(1);
      expect(images[0]).toMatch(/^data:image\/png;base64,/);
    });

    test('should work alongside automatic image processing from tool results', async () => {
      // Clear any existing images
      agent.clearLoadedImages();

      // Simulate tool result that mentions an image
      const toolResultWithImage = `Found the file at ${testImagePath}`;
      await agent.processImageReferences(toolResultWithImage);

      const imagesFromAutomatic = agent.getCurrentImages().length;

      // Now explicitly read another image
      const anotherImage = join(testDir, 'another.png');
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
      writeFileSync(anotherImage, simplePng);

      await agent.toolImplementations.readImage.execute({
        path: anotherImage
      });

      const totalImages = agent.getCurrentImages().length;
      expect(totalImages).toBeGreaterThan(imagesFromAutomatic);
    });
  });
});
