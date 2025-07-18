import { ProbeChat } from './probeChat.js';

// Test image extraction with OpenTelemetry spans
async function testImageExtraction() {
  console.log('Testing image extraction with OpenTelemetry spans...\n');
  
  try {
    // Create a ProbeChat instance with no API keys mode
    const probeChat = new ProbeChat({
      debug: true,
      noApiKeysMode: true
    });
    
    // Test message with images
    const testMessage = `
      Here are some images:
      - GitHub asset: https://github.com/user-attachments/assets/example.png
      - Private image: https://private-user-images.githubusercontent.com/123/example.jpg
      - Regular image: https://example.com/photo.jpeg
      
      And some text without images.
    `;
    
    console.log('🔍 Testing chat with images (no API keys mode)...');
    const result = await probeChat.chat(testMessage);
    console.log('✅ Chat completed successfully');
    console.log('📄 Response:', result.response.substring(0, 100) + '...');
    
    // Test completed
    console.log('\n🎉 Test completed! Check test-image-spans.jsonl for trace data.');
    
  } catch (error) {
    console.error('❌ Test failed:', error.message);
  }
}

testImageExtraction().catch(console.error);