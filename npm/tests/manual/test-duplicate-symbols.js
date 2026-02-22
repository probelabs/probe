// Test file with duplicate symbol names at different scopes

// Top-level function named "process"
function process(data) {
  return data.map(x => x * 2);
}

// Class with a method also named "process"
class DataProcessor {
  constructor(name) {
    this.name = name;
  }

  process(data) {
    return data.filter(x => x > 0);
  }

  validate(data) {
    return data.every(x => typeof x === 'number');
  }
}

// Another class with "process" method
class StreamProcessor {
  process(stream) {
    return stream.pipe(this.transform);
  }
}

// Top-level function named "validate" (same as class method)
function validate(input) {
  return input !== null && input !== undefined;
}

// Nested: function inside function
function outer() {
  function inner() {
    return 42;
  }
  return inner();
}

module.exports = { process, DataProcessor, StreamProcessor, validate, outer };
