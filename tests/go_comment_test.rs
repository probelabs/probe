use anyhow::Result;

#[test]
fn test_go_struct_comments() -> Result<()> {
    use probe_code::language::parser::parse_file_for_code_blocks;
    use std::collections::HashSet;

    let code = r#"
package main

// First struct represents something
type First struct {
    Field string `json:"field"`
}

// Second struct also represents something
type Second struct {
    Data string `json:"data"`
}
"#;

    // Create a set with the line numbers of the comments
    let mut line_numbers = HashSet::new();
    line_numbers.insert(4); // First comment line
    line_numbers.insert(9); // Second comment line

    // Parse the file for code blocks
    let blocks = parse_file_for_code_blocks(code, "go", &line_numbers, true, None)?;

    // We should have exactly 2 blocks
    assert_eq!(
        blocks.len(),
        2,
        "Expected exactly 2 blocks, got {}",
        blocks.len()
    );

    // First block should be a type_declaration for First struct
    assert_eq!(
        blocks[0].node_type, "type_declaration",
        "First block should be a type_declaration"
    );
    assert_eq!(
        blocks[0].start_row + 1,
        4,
        "First block should start at line 4"
    );
    assert_eq!(blocks[0].end_row + 1, 7, "First block should end at line 7");

    // Second block should be a type_declaration for Second struct
    assert_eq!(
        blocks[1].node_type, "type_declaration",
        "Second block should be a type_declaration"
    );
    assert_eq!(
        blocks[1].start_row + 1,
        9,
        "Second block should start at line 9"
    );
    assert_eq!(
        blocks[1].end_row + 1,
        12,
        "Second block should end at line 12"
    );

    // Verify the two blocks are different
    assert_ne!(blocks[0].start_row, blocks[1].start_row);

    Ok(())
}

#[test]
fn test_go_nested_structs() -> Result<()> {
    use probe_code::language::parser::parse_file_for_code_blocks;
    use std::collections::HashSet;

    let code = r#"
package main

// OuterType represents a container
type OuterType struct {
    // InnerType represents nested data
    InnerType struct {
        Field string `json:"field"`
    }
}
"#;

    // Create a set with the line numbers of the comments
    let mut line_numbers = HashSet::new();
    line_numbers.insert(4); // Outer comment line
    line_numbers.insert(6); // Inner comment line

    // Parse the file for code blocks
    let blocks = parse_file_for_code_blocks(code, "go", &line_numbers, true, None)?;

    // Due to deduplication of overlapping blocks, we only get one block
    // The inner struct is contained within the outer struct, so it's skipped
    assert_eq!(
        blocks.len(),
        1,
        "Expected exactly 1 block after deduplication, got {}",
        blocks.len()
    );

    // The block should be a type_declaration for OuterType that includes both comments
    assert_eq!(
        blocks[0].node_type, "type_declaration",
        "Block should be a type_declaration"
    );
    assert_eq!(blocks[0].start_row + 1, 4, "Block should start at line 4");
    assert_eq!(blocks[0].end_row + 1, 10, "Block should end at line 10");

    Ok(())
}

#[test]
fn test_go_mixed_declarations() -> Result<()> {
    use probe_code::language::parser::parse_file_for_code_blocks;
    use std::collections::HashSet;

    let code = r#"
package main

// CommentA describes interface
type InterfaceA interface {
    Method()
}

// CommentB describes struct
type StructB struct {
    Field string
}
"#;

    // Create a set with the line numbers of the comments
    let mut line_numbers = HashSet::new();
    line_numbers.insert(4); // Interface comment line
    line_numbers.insert(9); // Struct comment line

    // Parse the file for code blocks
    let blocks = parse_file_for_code_blocks(code, "go", &line_numbers, true, None)?;

    // We should have exactly 2 blocks
    assert_eq!(
        blocks.len(),
        2,
        "Expected exactly 2 blocks, got {}",
        blocks.len()
    );

    // First block should be a type_declaration for InterfaceA
    assert_eq!(
        blocks[0].node_type, "type_declaration",
        "First block should be a type_declaration"
    );
    assert_eq!(
        blocks[0].start_row + 1,
        4,
        "First block should start at line 4"
    );
    assert_eq!(blocks[0].end_row + 1, 7, "First block should end at line 7");

    // Second block should be a type_declaration for StructB
    assert_eq!(
        blocks[1].node_type, "type_declaration",
        "Second block should be a type_declaration"
    );
    assert_eq!(
        blocks[1].start_row + 1,
        9,
        "Second block should start at line 9"
    );
    assert_eq!(
        blocks[1].end_row + 1,
        12,
        "Second block should end at line 12"
    );

    // Verify they're different declarations
    assert_ne!(blocks[0].start_row, blocks[1].start_row);

    Ok(())
}

#[test]
fn test_go_comment_code_block_extraction() -> Result<()> {
    use probe_code::language::parser::parse_file_for_code_blocks;
    use std::collections::HashSet;

    // Sample code with a comment and struct
    let code = r#"
package main

// DatasourceResponse represents the response for datasource-related operations
// @Description Datasource response model
type DatasourceResponse struct {
    Type       string `json:"type"`
    ID         string `json:"id"`
    Attributes struct {
        Name string `json:"name"`
    }
}
"#;

    println!("Code lines:");
    for (i, line) in code.lines().enumerate() {
        println!("{}: {}", i + 1, line);
    }

    // Create a set with the line number of the comment
    let mut line_numbers = HashSet::new();
    line_numbers.insert(5); // Try the second comment line instead

    // Enable debug mode for this test
    std::env::set_var("DEBUG", "1");

    // Parse the file for code blocks
    let blocks = parse_file_for_code_blocks(code, "go", &line_numbers, true, None)?;

    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // We should have exactly 1 block: the merged comment and struct
    assert_eq!(
        blocks.len(),
        1,
        "Expected exactly 1 block, got {}",
        blocks.len()
    );

    // The block should be a type_declaration (the comment is merged with it)
    assert_eq!(
        blocks[0].node_type, "type_declaration",
        "Block should be a type_declaration"
    );
    assert_eq!(
        blocks[0].start_row + 1,
        5, // Should start at the second comment line
        "Block should start at line 5 (second comment line)"
    );
    assert_eq!(blocks[0].end_row + 1, 12, "Block should end at line 12");

    Ok(())
}
