#!/usr/bin/env node

import { writeFileSync, unlinkSync, existsSync, readFileSync } from 'fs';
import { resolve, isAbsolute } from 'path';

// Mock the extractImageUrls function for testing (to avoid API key requirements)
async function extractImageUrls(message, debug = false) {
  const imageUrlPattern = /(?:data:image\/[a-zA-Z]*;base64,[A-Za-z0-9+/=]+|https?:\/\/(?:(?:private-user-images\.githubusercontent\.com|github\.com\/user-attachments\/assets)\/[^\s"'<>]+|[^\s"'<>]+\.(?:png|jpg|jpeg|webp|bmp|svg)(?:\?[^\s"'<>]*)?)|(?:\.?\.?\/)?[^\s"'<>]*\.(?:png|jpg|jpeg|webp|bmp|svg))/gi;

  const urls = [];
  const foundPatterns = [];
  let match;

  while ((match = imageUrlPattern.exec(message)) !== null) {
    foundPatterns.push(match[0]);
    if (debug) {
      console.log(`[DEBUG] Found image pattern: ${match[0]}`);
    }
  }

  // Process each found pattern
  for (const pattern of foundPatterns) {
    if (pattern.startsWith('http') || pattern.startsWith('data:image/')) {
      urls.push(pattern);
      if (debug) {
        console.log(`[DEBUG] Using URL/base64 as-is: ${pattern.substring(0, 50)}...`);
      }
    } else {
      // Local file path - convert to base64
      try {
        const absolutePath = isAbsolute(pattern) ? pattern : resolve(process.cwd(), pattern);
        
        if (!existsSync(absolutePath)) {
          if (debug) console.log(`[DEBUG] File not found: ${absolutePath}`);
          continue;
        }

        const extension = absolutePath.toLowerCase().split('.').pop();
        const mimeTypes = {
          'png': 'image/png',
          'jpg': 'image/jpeg',
          'jpeg': 'image/jpeg',
          'webp': 'image/webp',
          'bmp': 'image/bmp',
          'svg': 'image/svg+xml'
        };
        
        const mimeType = mimeTypes[extension];
        if (!mimeType) {
          if (debug) console.log(`[DEBUG] Unsupported format: ${extension}`);
          continue;
        }

        const fileBuffer = readFileSync(absolutePath);
        const base64Data = fileBuffer.toString('base64');
        const dataUrl = `data:${mimeType};base64,${base64Data}`;
        urls.push(dataUrl);
        
        if (debug) {
          console.log(`[DEBUG] Converted ${pattern} to base64 (${fileBuffer.length} bytes)`);
        }
      } catch (error) {
        if (debug) {
          console.log(`[DEBUG] Error processing ${pattern}: ${error.message}`);
        }
      }
    }
  }

  // Clean message
  let cleanedMessage = message;
  foundPatterns.forEach(pattern => {
    cleanedMessage = cleanedMessage.replace(pattern, '').trim();
  });
  cleanedMessage = cleanedMessage.replace(/\s+/g, ' ').trim();

  return { urls, cleanedMessage };
}

/**
 * Test to verify local image file reading functionality
 */

console.log('🖼️  Testing Local Image File Reading\n');

async function testLocalImageReading() {
  try {
    // Create a simple test image file (1x1 PNG)
    const testImagePath = './test-image.png';
    const simplePngBuffer = Buffer.from([
      0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
      0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
      0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1 dimensions
      0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
      0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, // IDAT chunk
      0x54, 0x78, 0x9C, 0x62, 0x00, 0x02, 0x00, 0x00,
      0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
      0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, // IEND chunk
      0x42, 0x60, 0x82
    ]);

    // Write test image
    writeFileSync(testImagePath, simplePngBuffer);
    console.log(`✅ Created test image: ${testImagePath}`);

    // Test 1: Local file path extraction
    console.log('\n📋 Test 1: Local file path in message');
    const message1 = `Please analyze this image: ${testImagePath} and tell me about it.`;
    const result1 = await extractImageUrls(message1, true);
    
    console.log(`Found ${result1.urls.length} images`);
    if (result1.urls.length > 0) {
      console.log(`✅ Successfully converted local file to base64`);
      console.log(`   Original: ${testImagePath}`);
      console.log(`   Base64 length: ${result1.urls[0].length} characters`);
      console.log(`   Data URL prefix: ${result1.urls[0].substring(0, 30)}...`);
    } else {
      console.log('❌ No images found');
    }
    console.log(`Cleaned message: "${result1.cleanedMessage}"`);

    // Test 2: Mixed URLs and local files
    console.log('\n📋 Test 2: Mixed URLs and local files');
    const message2 = `Compare ${testImagePath} with https://example.com/remote.png`;
    const result2 = await extractImageUrls(message2, true);
    
    console.log(`Found ${result2.urls.length} images`);
    console.log(`Cleaned message: "${result2.cleanedMessage}"`);

    // Test 3: Relative paths
    console.log('\n📋 Test 3: Relative path variations');
    const messages = [
      `Image at ./test-image.png`,
      `Image at ../examples/chat/test-image.png`,
      `Image at test-image.png`
    ];
    
    for (const msg of messages) {
      const result = await extractImageUrls(msg, true);
      console.log(`"${msg}" -> ${result.urls.length} images found`);
    }

    // Test 4: Security - try to access file outside allowed directory
    console.log('\n📋 Test 4: Security validation');
    const securityTestMessage = 'Look at this image: /etc/passwd.png';
    const securityResult = await extractImageUrls(securityTestMessage, true);
    console.log(`Security test: ${securityResult.urls.length} images found (should be 0)`);

    // Test 5: Non-existent file
    console.log('\n📋 Test 5: Non-existent file');
    const missingFileMessage = 'Check this image: ./missing-image.png';
    const missingResult = await extractImageUrls(missingFileMessage, true);
    console.log(`Missing file test: ${missingResult.urls.length} images found (should be 0)`);

    // Cleanup
    if (existsSync(testImagePath)) {
      unlinkSync(testImagePath);
      console.log('\n🧹 Cleaned up test image');
    }

    console.log('\n🎉 Local image reading tests completed!');

  } catch (error) {
    console.error('❌ Test failed:', error);
  }
}

testLocalImageReading().catch(console.error);