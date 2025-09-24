#!/usr/bin/env node

/**
 * Test script to verify file size validation prevents OOM attacks
 */

import { writeFileSync, unlinkSync } from 'fs';
import { readFile, stat } from 'fs/promises';
import { resolve } from 'path';

// Same constants as in the implementation
const MAX_IMAGE_FILE_SIZE = 20 * 1024 * 1024;

// Simulate the file size validation logic
async function testFileSizeCheck(filePath) {
  try {
    const absolutePath = resolve(process.cwd(), filePath);
    const fileStats = await stat(absolutePath);
    
    if (fileStats.size > MAX_IMAGE_FILE_SIZE) {
      console.log(`[DEBUG] Image file too large: ${absolutePath} (${fileStats.size} bytes, max: ${MAX_IMAGE_FILE_SIZE})`);
      return false;
    }
    
    console.log(`[DEBUG] File size OK: ${absolutePath} (${fileStats.size} bytes)`);
    return true;
  } catch (error) {
    console.log(`[DEBUG] File check failed: ${error.message}`);
    return false;
  }
}

async function testFileSizeValidation() {
  console.log('🔒 Testing File Size Validation\n');

  const testImagePath = './large-test-image.png';
  
  // Create a simple PNG header
  const pngHeader = Buffer.from([
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
    0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89
  ]);

  try {
    // Test 1: Create a file larger than 20MB (simulate large file)
    console.log('📋 Test 1: File size validation');
    
    // Create a "large" file by padding with zeros (simulates large file)
    const largeFileSize = 25 * 1024 * 1024; // 25MB
    const largeBuffer = Buffer.alloc(largeFileSize);
    
    // Copy PNG header to start
    pngHeader.copy(largeBuffer, 0);
    
    // Write the large file
    writeFileSync(testImagePath, largeBuffer);
    console.log(`✅ Created large test file: ${testImagePath} (${largeFileSize} bytes)`);

    // Try to validate it
    const sizeCheckPassed = await testFileSizeCheck(testImagePath);
    
    if (!sizeCheckPassed) {
      console.log('✅ File size validation working: Large file rejected');
    } else {
      console.log('❌ File size validation failed: Large file was accepted');
    }

    // Test 2: Small file should work
    console.log('\n📋 Test 2: Small file acceptance');
    
    unlinkSync(testImagePath);
    
    // Create a small valid file
    writeFileSync(testImagePath, pngHeader);
    console.log(`✅ Created small test file: ${testImagePath} (${pngHeader.length} bytes)`);
    
    const sizeCheckPassed2 = await testFileSizeCheck(testImagePath);
    
    if (sizeCheckPassed2) {
      console.log('✅ Small file accepted successfully');
    } else {
      console.log('❌ Small file validation failed');
    }

    console.log('\n🎉 File size validation tests completed!');

  } catch (error) {
    console.error('❌ Test failed:', error);
  } finally {
    // Cleanup
    try {
      unlinkSync(testImagePath);
      console.log('\n🧹 Test cleanup completed');
    } catch (e) {
      // Ignore cleanup errors
    }
  }
}

testFileSizeValidation().catch(console.error);