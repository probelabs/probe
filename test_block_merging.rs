// A test file for block merging
// With smaller gaps between blocks

struct TestStructA {
    // Block 1
    field1: String,
    field2: String, // test_merge_keyword
}

// Small gap (1 line)
// This should merge

impl TestStructA {
    // Block 2 - should merge with Block 1
    fn new() -> Self {
        Self {
            field1: "test_merge_keyword".to_string(),
            field2: "value2".to_string(),
        }
    }
}

// Small gap (2 lines)
// This should also merge

struct TestStructB {
    // Block 3 - should merge with Block 2
    field1: String, // test_merge_keyword
    field2: String,
}

// Larger gap (8 lines) - should NOT be included
// Line 1
// Line 2
// Line 3
// Line 4
// Line 5
// Line 6
// Line 7

struct TestStructC {
    // Block 4 - should NOT merge with Block 3
    field1: String,
    field2: String, // test_merge_keyword
}
