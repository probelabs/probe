import { ProbeChat } from './probeChat.js';

// Simple test to check if tracing works
async function testSimpleTracing() {
  console.log('Testing simple tracing...\n');
  
  try {
    // Create a ProbeChat instance with debug enabled
    const probeChat = new ProbeChat({
      debug: true
    });
    
    // Test just the extractImageUrls function directly
    const message = 'Here is an image: https://github.com/user-attachments/assets/example.png';
    
    console.log('🔍 Testing extractImageUrls function...');
    
    // Import the function to test it directly
    const { extractImageUrls } = await import('./probeChat.js');
    
    // This should create a span
    const result = await extractImageUrls(message, true);
    
    console.log('✅ extractImageUrls result:', result);
    console.log('🎉 Test completed!');
    
  } catch (error) {
    console.error('❌ Test failed:', error.message);
  }
}

testSimpleTracing().catch(console.error);