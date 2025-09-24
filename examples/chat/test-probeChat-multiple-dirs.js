import { promises as fs } from 'fs';
import { resolve } from 'path';
import { extractImageUrls } from './probeChat.js';

// Test that multiple allowed directories work correctly in probeChat.js
async function testMultipleAllowedDirectoriesInProbeChat() {
  console.log('🧪 Testing multiple allowed directories in probeChat.js...\n');

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

    // Create small dummy PNG files (1x1 pixel transparent PNG)
    const dummyImageBuffer = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==', 'base64');
    
    for (const testFile of testFiles) {
      await fs.writeFile(resolve(testFile.dir, testFile.file), dummyImageBuffer);
    }

    // Test 1: Single allowed directory
    console.log('\n🔒 Test 1: Single allowed directory');
    const message1 = `Check image ./test-images-1/image1.png and ./test-images-2/image2.jpg`;
    
    // Mock the global allowedFolders for this test
    global.allowedFolders = ['./test-images-1'];
    
    const result1 = await extractImageUrls(message1, true);
    
    console.log('   Results:', result1);
    console.log(`   Expected: Should load image1.png but not image2.jpg`);

    // Test 2: Multiple allowed directories  
    console.log('\n🔓 Test 2: Multiple allowed directories');
    const message2 = `Check image ./test-images-1/image1.png and ./test-images-2/image2.jpg and ./test-images-3/image3.gif`;
    
    // Mock the global allowedFolders for this test
    global.allowedFolders = ['./test-images-1', './test-images-2'];
    
    const result2 = await extractImageUrls(message2, true);
    
    console.log('   Results:', result2);
    console.log(`   Expected: Should load image1.png and image2.jpg but not image3.gif`);

    // Test 3: All directories allowed
    console.log('\n🌐 Test 3: All directories allowed');
    const message3 = `Check image ./test-images-1/image1.png and ./test-images-2/image2.jpg and ./test-images-3/image3.gif`;
    
    // Mock the global allowedFolders for this test
    global.allowedFolders = ['./test-images-1', './test-images-2', './test-images-3'];
    
    const result3 = await extractImageUrls(message3, true);
    
    console.log('   Results:', result3);  
    console.log(`   Expected: Should load all three images`);

    console.log('\n🎉 Manual validation required - check the results above match expectations.');

  } catch (error) {
    console.error('❌ Test failed with error:', error.message);
    console.error(error.stack);
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
testMultipleAllowedDirectoriesInProbeChat().catch(console.error);