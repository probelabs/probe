#!/usr/bin/env node

/**
 * Demo script showing local image file support in probe agent
 */

import { writeFileSync, unlinkSync, readFileSync, existsSync } from 'fs';
import { resolve, isAbsolute } from 'path';

// Standalone image extraction function for demo
async function extractImageUrls(message, debug = false) {
  const imageUrlPattern = /(?:data:image\/[a-zA-Z]*;base64,[A-Za-z0-9+/=]+|https?:\/\/(?:(?:private-user-images\.githubusercontent\.com|github\.com\/user-attachments\/assets)\/[^\s"'<>]+|[^\s"'<>]+\.(?:png|jpg|jpeg|webp|bmp|svg)(?:\?[^\s"'<>]*)?)|(?:\.?\.?\/)?[^\s"'<>]*\.(?:png|jpg|jpeg|webp|bmp|svg))/gi;

  const urls = [];
  const foundPatterns = [];
  let match;

  while ((match = imageUrlPattern.exec(message)) !== null) {
    foundPatterns.push(match[0]);
  }

  // Process each found pattern
  for (const pattern of foundPatterns) {
    if (pattern.startsWith('http') || pattern.startsWith('data:image/')) {
      urls.push(pattern);
    } else {
      // Local file path - convert to base64
      try {
        const absolutePath = isAbsolute(pattern) ? pattern : resolve(process.cwd(), pattern);
        
        if (!existsSync(absolutePath)) continue;

        const extension = absolutePath.toLowerCase().split('.').pop();
        const mimeTypes = {
          'png': 'image/png', 'jpg': 'image/jpeg', 'jpeg': 'image/jpeg',
          'webp': 'image/webp', 'bmp': 'image/bmp', 'svg': 'image/svg+xml'
        };
        
        const mimeType = mimeTypes[extension];
        if (!mimeType) continue;

        const fileBuffer = readFileSync(absolutePath);
        const base64Data = fileBuffer.toString('base64');
        const dataUrl = `data:${mimeType};base64,${base64Data}`;
        urls.push(dataUrl);
      } catch (error) {
        // Skip failed files
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

async function demo() {
  console.log('🖼️  Probe Agent - Local Image Support Demo\n');

  // Create a simple test image
  const testImage = './demo-image.png';
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
  writeFileSync(testImage, simplePng);

  console.log('📝 Example user message:');
  const userMessage = `
    Can you analyze this architecture diagram: ${testImage} 
    and compare it with this online example: https://example.com/architecture.png
  `.trim();
  
  console.log(`"${userMessage}"\n`);

  console.log('🔍 Processing with extractImageUrls...\n');
  
  try {
    const result = await extractImageUrls(userMessage, false);
    
    console.log('📊 Results:');
    console.log(`✅ Found ${result.urls.length} images`);
    console.log(`📝 Cleaned message: "${result.cleanedMessage}"`);
    console.log('\n🖼️  Image details:');
    
    result.urls.forEach((url, index) => {
      if (url.startsWith('data:image/')) {
        console.log(`  ${index + 1}. Local file converted to base64 (${url.length} chars)`);
        console.log(`     Preview: ${url.substring(0, 50)}...`);
      } else {
        console.log(`  ${index + 1}. URL: ${url}`);
      }
    });

    console.log('\n✨ The probe agent can now seamlessly handle:');
    console.log('  • Local image files (./image.png)');
    console.log('  • Remote URLs (https://example.com/image.jpg)');
    console.log('  • Base64 data URLs (data:image/...)');
    console.log('  • Mixed content in the same message');
    
  } catch (error) {
    console.error('❌ Error:', error.message);
  } finally {
    // Cleanup
    unlinkSync(testImage);
    console.log('\n🧹 Demo cleanup completed');
  }
}

demo().catch(console.error);