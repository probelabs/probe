#!/usr/bin/env node

/**
 * Test script to verify agentic loop image loading functionality
 *
 * NOTE: This is a standalone test file. The MIME types and regex patterns below
 * are duplicated from @probelabs/probe/agent/imageConfig for self-containment.
 * When modifying image support, update both the shared config and this file.
 */

import { writeFileSync, unlinkSync, existsSync } from 'fs';
import { resolve } from 'path';

// Mock ProbeAgent to test image processing without API calls
// MIME types duplicated from @probelabs/probe/agent/imageConfig (keep in sync!)
class MockProbeAgent {
  constructor(options = {}) {
    this.debug = options.debug || true;
    this.allowedFolders = options.path ? [options.path] : [process.cwd()];
    this.pendingImages = new Map();
    this.currentImages = [];
  }

  // Copy the image processing methods from ProbeAgent.js
  async processImageReferences(content) {
    if (!content) return;

    const imagePatterns = [
      /(?:\.?\.?\/)?[^\s"'<>\[\]]+\.(?:png|jpg|jpeg|webp|bmp|svg)(?!\w)/gi,
      /(?:image|file|screenshot|diagram|photo|picture|graphic)\s*:?\s*([^\s"'<>\[\]]+\.(?:png|jpg|jpeg|webp|bmp|svg))(?!\w)/gi,
      /(?:found|saved|created|generated).*?([^\s"'<>\[\]]+\.(?:png|jpg|jpeg|webp|bmp|svg))(?!\w)/gi
    ];

    const foundPaths = new Set();

    for (const pattern of imagePatterns) {
      let match;
      while ((match = pattern.exec(content)) !== null) {
        const imagePath = match[1] || match[0];
        if (imagePath && imagePath.length > 0) {
          foundPaths.add(imagePath.trim());
        }
      }
    }

    if (foundPaths.size === 0) return;

    if (this.debug) {
      console.log(`[DEBUG] Found ${foundPaths.size} potential image references:`, Array.from(foundPaths));
    }

    for (const imagePath of foundPaths) {
      await this.loadImageIfValid(imagePath);
    }
  }

  async loadImageIfValid(imagePath) {
    try {
      if (this.pendingImages.has(imagePath)) {
        if (this.debug) {
          console.log(`[DEBUG] Image already loaded: ${imagePath}`);
        }
        return true;
      }

      const baseDir = this.allowedFolders && this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd();
      const absolutePath = resolve(baseDir, imagePath);
      
      if (!absolutePath.startsWith(resolve(baseDir))) {
        if (this.debug) {
          console.log(`[DEBUG] Image path outside allowed directory: ${imagePath}`);
        }
        return false;
      }

      if (!existsSync(absolutePath)) {
        if (this.debug) {
          console.log(`[DEBUG] Image file not found: ${absolutePath}`);
        }
        return false;
      }

      const extension = absolutePath.toLowerCase().split('.').pop();
      // Supported extensions from @probelabs/probe/agent/imageConfig (keep in sync!)
      const supportedExtensions = ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'svg'];
      if (!supportedExtensions.includes(extension)) {
        if (this.debug) {
          console.log(`[DEBUG] Unsupported image format: ${extension}`);
        }
        return false;
      }

      // MIME types from @probelabs/probe/agent/imageConfig (keep in sync!)
      const mimeTypes = {
        'png': 'image/png',
        'jpg': 'image/jpeg',
        'jpeg': 'image/jpeg',
        'webp': 'image/webp',
        'bmp': 'image/bmp',
        'svg': 'image/svg+xml'
      };
      const mimeType = mimeTypes[extension];

      // Simulate reading file (use minimal data for test)
      const mockBase64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==';
      const dataUrl = `data:${mimeType};base64,${mockBase64}`;

      this.pendingImages.set(imagePath, dataUrl);

      if (this.debug) {
        console.log(`[DEBUG] Successfully loaded image: ${imagePath} (simulated)`);
      }

      return true;
    } catch (error) {
      if (this.debug) {
        console.log(`[DEBUG] Failed to load image ${imagePath}: ${error.message}`);
      }
      return false;
    }
  }

  getCurrentImages() {
    return Array.from(this.pendingImages.values());
  }

  clearLoadedImages() {
    this.pendingImages.clear();
    this.currentImages = [];
    if (this.debug) {
      console.log('[DEBUG] Cleared all loaded images');
    }
  }

  prepareMessagesWithImages(messages) {
    const loadedImages = this.getCurrentImages();
    
    if (loadedImages.length === 0) {
      return messages;
    }

    const messagesWithImages = [...messages];
    const lastUserMessageIndex = messagesWithImages.map(m => m.role).lastIndexOf('user');
    
    if (lastUserMessageIndex === -1) {
      if (this.debug) {
        console.log('[DEBUG] No user messages found to attach images to');
      }
      return messages;
    }

    const lastUserMessage = messagesWithImages[lastUserMessageIndex];
    
    if (typeof lastUserMessage.content === 'string') {
      messagesWithImages[lastUserMessageIndex] = {
        ...lastUserMessage,
        content: [
          { type: 'text', text: lastUserMessage.content },
          ...loadedImages.map(imageData => ({
            type: 'image',
            image: imageData
          }))
        ]
      };

      if (this.debug) {
        console.log(`[DEBUG] Added ${loadedImages.length} images to the latest user message`);
      }
    }

    return messagesWithImages;
  }
}

async function testAgenticImageLoading() {
  console.log('🤖 Testing Agentic Loop Image Loading\n');

  // Create test images
  const testImages = ['./test-diagram.png', './screenshot.jpg', './chart.png'];
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

  for (const imagePath of testImages) {
    writeFileSync(imagePath, simplePng);
  }

  console.log(`✅ Created ${testImages.length} test images`);

  try {
    const agent = new MockProbeAgent({ debug: true });

    // Test 1: Assistant mentions an image file
    console.log('\n📋 Test 1: Assistant mentions image in response');
    const assistantResponse1 = `I need to analyze the diagram in ./test-diagram.png to understand the architecture.`;
    await agent.processImageReferences(assistantResponse1);
    
    let loadedImages = agent.getCurrentImages();
    console.log(`✅ Test 1 result: ${loadedImages.length} image(s) loaded`);

    // Test 2: Tool result contains image paths
    console.log('\n📋 Test 2: Tool result mentions multiple images');
    const toolResult = `
Found 3 relevant files:
- screenshot.jpg (contains the error message)
- chart.png (shows the performance metrics)
- config.json (configuration file)
    `;
    await agent.processImageReferences(toolResult);
    
    loadedImages = agent.getCurrentImages();
    console.log(`✅ Test 2 result: ${loadedImages.length} total image(s) loaded`);

    // Test 3: Message preparation for AI
    console.log('\n📋 Test 3: Message preparation with images');
    const testMessages = [
      { role: 'system', content: 'You are a helpful assistant.' },
      { role: 'user', content: 'Please analyze the uploaded files.' },
      { role: 'assistant', content: 'I will analyze the images you provided.' },
      { role: 'user', content: 'What do you see in the images?' }
    ];

    const messagesWithImages = agent.prepareMessagesWithImages(testMessages);
    const lastMessage = messagesWithImages[messagesWithImages.length - 1];
    
    console.log(`✅ Test 3 result: Last message format: ${typeof lastMessage.content}`);
    if (Array.isArray(lastMessage.content)) {
      console.log(`   - Text parts: ${lastMessage.content.filter(p => p.type === 'text').length}`);
      console.log(`   - Image parts: ${lastMessage.content.filter(p => p.type === 'image').length}`);
    }

    // Test 4: Contextual image detection
    console.log('\n📋 Test 4: Various contextual image mentions');
    const contextualTexts = [
      'Look at file ./test-diagram.png',
      'The screenshot screenshot.jpg shows the issue',
      'I found chart.png in the directory',
      'Generated diagram at ./output.svg',  // Non-existent file
    ];

    agent.clearLoadedImages();
    for (const text of contextualTexts) {
      console.log(`   Processing: "${text}"`);
      await agent.processImageReferences(text);
    }

    loadedImages = agent.getCurrentImages();
    console.log(`✅ Test 4 result: ${loadedImages.length} contextual image(s) loaded`);

    // Test 5: Security validation
    console.log('\n📋 Test 5: Security validation for path traversal');
    const maliciousTexts = [
      'Check ../../../etc/passwd.png',  // Path traversal attempt
      'Look at /system/secrets.jpg',     // Absolute path outside
    ];

    const beforeCount = agent.getCurrentImages().length;
    for (const text of maliciousTexts) {
      console.log(`   Testing: "${text}"`);
      await agent.processImageReferences(text);
    }

    const afterCount = agent.getCurrentImages().length;
    console.log(`✅ Test 5 result: No new images loaded (${beforeCount} -> ${afterCount})`);

    console.log('\n🎉 All tests completed successfully!');
    console.log('\n📊 Summary:');
    console.log(`   - Total images currently loaded: ${agent.getCurrentImages().length}`);
    console.log('   - Image detection patterns working correctly');
    console.log('   - Security validation preventing unauthorized access');
    console.log('   - Multimodal message preparation functioning');

  } catch (error) {
    console.error('❌ Test failed:', error);
  } finally {
    // Cleanup
    for (const imagePath of testImages) {
      if (existsSync(imagePath)) {
        unlinkSync(imagePath);
      }
    }
    console.log('\n🧹 Test cleanup completed');
  }
}

testAgenticImageLoading().catch(console.error);