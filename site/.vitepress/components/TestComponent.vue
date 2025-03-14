<template>
  <div class="test-component">
    <div class="test-content">
      <h3>Content Slot:</h3>
      <div class="content-box">
        <slot name="content"></slot>
      </div>
    </div>
    <div class="test-code">
      <h3>Code Slot:</h3>
      <div class="code-box">
        <slot name="code"></slot>
      </div>
    </div>
  </div>
</template>

<style scoped>
.test-component {
  border: 2px solid #ccc;
  padding: 20px;
  margin: 20px 0;
  border-radius: 8px;
}

.test-content, .test-code {
  margin-bottom: 20px;
}

.content-box, .code-box {
  border: 1px dashed #999;
  padding: 15px;
  border-radius: 4px;
  background-color: #f5f5f5;
}

h3 {
  margin-top: 0;
  margin-bottom: 10px;
}
</style> 