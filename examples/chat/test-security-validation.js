import { promises as fs } from 'fs';
import { resolve, isAbsolute } from 'path';

// Copy of the security validation function from probeChat.js to test it directly
function isSecureFilePath(filePath, allowedDir) {
  try {
    const resolvedFilePath = isAbsolute(filePath) ? filePath : resolve(allowedDir, filePath);
    const resolvedAllowedDir = resolve(allowedDir);
    
    // Check if the resolved path is within the allowed directory
    return resolvedFilePath.startsWith(resolvedAllowedDir);
  } catch (error) {
    return false;
  }
}

// Test the multiple allowed directories logic
function testMultipleAllowedDirsLogic() {
  console.log('🧪 Testing multiple allowed directories security validation...\n');

  const testCases = [
    {
      name: 'Single allowed directory - valid path',
      filePath: './test-dir-1/image.png',
      allowedDirs: ['./test-dir-1'],
      expected: true
    },
    {
      name: 'Single allowed directory - invalid path', 
      filePath: './test-dir-2/image.png',
      allowedDirs: ['./test-dir-1'],
      expected: false
    },
    {
      name: 'Multiple allowed directories - valid in first dir',
      filePath: './test-dir-1/image.png', 
      allowedDirs: ['./test-dir-1', './test-dir-2'],
      expected: true
    },
    {
      name: 'Multiple allowed directories - valid in second dir',
      filePath: './test-dir-2/image.png',
      allowedDirs: ['./test-dir-1', './test-dir-2'],
      expected: true
    },
    {
      name: 'Multiple allowed directories - invalid path',
      filePath: './test-dir-3/image.png',
      allowedDirs: ['./test-dir-1', './test-dir-2'],
      expected: false
    },
    {
      name: 'Three allowed directories - valid in third dir',
      filePath: './test-dir-3/image.png',
      allowedDirs: ['./test-dir-1', './test-dir-2', './test-dir-3'],
      expected: true
    }
  ];

  let allPassed = true;
  
  for (const testCase of testCases) {
    console.log(`🔍 ${testCase.name}`);
    
    // Test the new logic: check if path is allowed in ANY of the directories
    const isPathAllowed = testCase.allowedDirs.some(dir => isSecureFilePath(testCase.filePath, dir));
    
    const passed = isPathAllowed === testCase.expected;
    allPassed = allPassed && passed;
    
    console.log(`   Path: ${testCase.filePath}`);
    console.log(`   Allowed dirs: [${testCase.allowedDirs.join(', ')}]`);
    console.log(`   Result: ${isPathAllowed} | Expected: ${testCase.expected} | ${passed ? '✅ PASS' : '❌ FAIL'}`);
    console.log();
  }

  if (allPassed) {
    console.log('🎉 All security validation tests passed!');
  } else {
    console.log('❌ Some tests failed - check implementation.');
  }

  return allPassed;
}

// Test path traversal attacks are still blocked
function testPathTraversalProtection() {
  console.log('\n🛡️  Testing path traversal attack protection...\n');

  const attackCases = [
    {
      name: 'Directory traversal with ../',
      filePath: '../../../etc/passwd',
      allowedDirs: ['./safe-dir'],
      expected: false
    },
    {
      name: 'Directory traversal within allowed path',
      filePath: './safe-dir/../../../etc/passwd', 
      allowedDirs: ['./safe-dir'],
      expected: false
    },
    {
      name: 'Valid nested path',
      filePath: './safe-dir/nested/image.png',
      allowedDirs: ['./safe-dir'],
      expected: true
    }
  ];

  let allPassed = true;
  
  for (const testCase of attackCases) {
    console.log(`🔍 ${testCase.name}`);
    
    const isPathAllowed = testCase.allowedDirs.some(dir => isSecureFilePath(testCase.filePath, dir));
    
    const passed = isPathAllowed === testCase.expected;
    allPassed = allPassed && passed;
    
    console.log(`   Path: ${testCase.filePath}`);
    console.log(`   Result: ${isPathAllowed} | Expected: ${testCase.expected} | ${passed ? '✅ PASS' : '❌ FAIL'}`);
    console.log();
  }

  if (allPassed) {
    console.log('🎉 All path traversal protection tests passed!');
  } else {
    console.log('❌ Some security tests failed - check implementation.');
  }

  return allPassed;
}

// Run all tests
console.log('🚀 Starting security validation tests...\n');

const test1Passed = testMultipleAllowedDirsLogic();
const test2Passed = testPathTraversalProtection();

if (test1Passed && test2Passed) {
  console.log('\n🏆 All tests passed! Multiple allowed directories security is working correctly.');
} else {
  console.log('\n💥 Some tests failed. Please review the implementation.');
}