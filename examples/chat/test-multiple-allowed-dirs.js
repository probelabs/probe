import { promises as fs } from 'fs';
import { resolve } from 'path';
import { ProbeAgent } from '../../npm/src/agent/ProbeAgent.js';

// Test that multiple allowed directories work correctly
async function testMultipleAllowedDirectories() {
  console.log('🧪 Testing multiple allowed directories...\n');

  // Create test directories and files
  const testDirs = [
    './test-images-1',
    './test-images-2',
    './test-images-3'
  ];

  const testFiles = [
    { dir: './test-images-1', file: 'image1.png' },
    { dir: './test-images-2', file: 'image2.jpg' },
    { dir: './test-images-3', file: 'image3.gif' }
  ];

  try {
    // Create test directories and dummy image files
    console.log('📁 Creating test directories and files...');
    for (const dir of testDirs) {
      await fs.mkdir(dir, { recursive: true });
    }

    // Create small dummy image files (just text files with image extensions for testing)
    const dummyImageContent = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==';
    
    for (const testFile of testFiles) {
      await fs.writeFile(resolve(testFile.dir, testFile.file), dummyImageContent);
    }

    // Test 1: Create ProbeAgent with only first directory allowed
    console.log('\n🔒 Test 1: Single allowed directory');
    const agent1 = new ProbeAgent({
      debug: true,
      allowedFolders: ['./test-images-1']
    });

    const result1a = await agent1.loadImageIfValid('./test-images-1/image1.png');
    const result1b = await agent1.loadImageIfValid('./test-images-2/image2.jpg');
    
    console.log(`   ✓ Image in allowed dir 1: ${result1a} (expected: true)`);
    console.log(`   ✗ Image in disallowed dir 2: ${result1b} (expected: false)`);

    // Test 2: Create ProbeAgent with multiple allowed directories
    console.log('\n🔓 Test 2: Multiple allowed directories');
    const agent2 = new ProbeAgent({
      debug: true,
      allowedFolders: ['./test-images-1', './test-images-2']
    });

    const result2a = await agent2.loadImageIfValid('./test-images-1/image1.png');
    const result2b = await agent2.loadImageIfValid('./test-images-2/image2.jpg');
    const result2c = await agent2.loadImageIfValid('./test-images-3/image3.gif');
    
    console.log(`   ✓ Image in allowed dir 1: ${result2a} (expected: true)`);
    console.log(`   ✓ Image in allowed dir 2: ${result2b} (expected: true)`);
    console.log(`   ✗ Image in disallowed dir 3: ${result2c} (expected: false)`);

    // Test 3: Test with all three directories allowed
    console.log('\n🌐 Test 3: All directories allowed');
    const agent3 = new ProbeAgent({
      debug: true,
      allowedFolders: ['./test-images-1', './test-images-2', './test-images-3']
    });

    const result3a = await agent3.loadImageIfValid('./test-images-1/image1.png');
    const result3b = await agent3.loadImageIfValid('./test-images-2/image2.jpg');
    const result3c = await agent3.loadImageIfValid('./test-images-3/image3.gif');
    
    console.log(`   ✓ Image in allowed dir 1: ${result3a} (expected: true)`);
    console.log(`   ✓ Image in allowed dir 2: ${result3b} (expected: true)`);
    console.log(`   ✓ Image in allowed dir 3: ${result3c} (expected: true)`);

    // Validation
    const allTestsPassed = 
      result1a === true && result1b === false &&
      result2a === true && result2b === true && result2c === false &&
      result3a === true && result3b === true && result3c === true;

    if (allTestsPassed) {
      console.log('\n🎉 All tests passed! Multiple allowed directories working correctly.');
    } else {
      console.log('\n❌ Some tests failed. Check the implementation.');
    }

  } catch (error) {
    console.error('❌ Test failed with error:', error.message);
  } finally {
    // Clean up test files and directories
    console.log('\n🧹 Cleaning up test files...');
    try {
      for (const testFile of testFiles) {
        await fs.unlink(resolve(testFile.dir, testFile.file)).catch(() => {});
      }
      for (const dir of testDirs) {
        await fs.rmdir(dir).catch(() => {});
      }
      console.log('✅ Cleanup completed.');
    } catch (cleanupError) {
      console.log('⚠️  Some cleanup failed:', cleanupError.message);
    }
  }
}

// Run the test
testMultipleAllowedDirectories().catch(console.error);