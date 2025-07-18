import { ProbeChat } from './probeChat.js';

// Test chat function tracing
async function testChatTracing() {
  console.log('Testing chat tracing...\n');
  
  try {
    // Create a ProbeChat instance with debug enabled
    const probeChat = new ProbeChat({
      debug: true,
      noApiKeysMode: true
    });
    
    // Test message with images
    const testMessage = 'Here is an image: https://github.com/user-attachments/assets/example.png and some text.';
    
    console.log('🔍 Testing chat function with tracing...');
    console.log('Message:', testMessage);
    
    // Call the chat function - this should create spans
    const result = await probeChat.chat(testMessage);
    
    console.log('✅ Chat completed successfully');
    console.log('📄 Response length:', result.response.length);
    console.log('📄 Response preview:', result.response.substring(0, 100) + '...');
    
    console.log('🎉 Test completed! Check simple-traces.jsonl for trace data.');
    
    // Wait a bit for telemetry to flush
    console.log('⏳ Waiting for telemetry to flush...');
    await new Promise(resolve => setTimeout(resolve, 2000));
    
  } catch (error) {
    console.error('❌ Test failed:', error.message);
  }
}

testChatTracing().catch(console.error);