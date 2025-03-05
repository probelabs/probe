# Detailed Implementation Plan: Moving Code Block Merging After Ranking and Limiting

Based on our analysis of the codebase, this plan outlines a comprehensive approach to move the code block merging logic so it occurs AFTER reranking and limiting. This represents a significant architectural change to the search flow.

## Current Flow
1. Extract code blocks from files using AST parsing
2. Merge overlapping code blocks
3. Calculate term statistics for each merged block
4. Rank all search results
5. Apply limits (max results, bytes, tokens)

## Desired Flow
1. Extract code blocks from files using AST parsing
2. Calculate term statistics for each individual block
3. Rank all individual search results
4. Apply limits (max results, bytes, tokens)
5. Merge overlapping code blocks among the limited results

## Milestone 1: Refactor Code Block Processing
**Objective**: Separate code block extraction from merging

### Action Steps:
1. Modify `process_file_with_results` in `file_processing.rs` to skip the merging step:
   - Keep the call to `parse_file_for_code_blocks`
   - Remove the call to `merge_code_blocks`
   - Process each individual code block

2. Add a new field to `SearchResult` to track block adjacency:
   - Add `parent_file_id: Option<String>` or similar to identify blocks from same file
   - Add `block_id: Option<usize>` to identify individual blocks for later merging

3. Create unit tests to verify individual blocks are correctly processed:
   - Test that overlapping blocks remain separate
   - Test that term statistics are calculated correctly for individual blocks

## Milestone 2: Create Post-Ranking Merging Function
**Objective**: Implement new function to merge blocks after ranking

### Action Steps:
1. Create a new function `merge_ranked_blocks` in a suitable module:
   ```rust
   /// Merges ranked search results that are adjacent or overlapping
   pub fn merge_ranked_blocks(results: Vec<SearchResult>) -> Vec<SearchResult>
   ```

2. Implement the merging logic:
   - Group results by file
   - Sort blocks within each file by line range
   - Merge adjacent blocks using similar logic to current `merge_code_blocks`
   - Recalculate combined block statistics (term matches, scores)

3. Create unit tests for the new merging function:
   - Test merging blocks from same file
   - Test preserving blocks from different files
   - Test handling of ranking scores in merged blocks

## Milestone 3: Integrate New Flow in Search Runner
**Objective**: Update main search flow to use new approach

### Action Steps:
1. Modify `perform_probe` in `search_runner.rs`:
   - Keep collection of individual blocks
   - Proceed with ranking individual blocks
   - Apply limits to ranked individual blocks
   - Add call to new `merge_ranked_blocks` after limiting

2. Update debug logging in `search_runner.rs`:
   - Add timing for the new post-rank merging step
   - Log statistics about merging (before/after block counts)

3. Create integration tests for the complete flow:
   - Test search with multiple overlapping blocks
   - Verify correct ranking and merging order

## Milestone 4: Handle Edge Cases and Optimizations
**Objective**: Address special cases and ensure performance

### Action Steps:
1. Update score calculation for merged blocks:
   - Determine how to combine scores from individual blocks
   - Options: max score, weighted average, or new calculation

2. Handle special cases:
   - Single-line matches
   - Filename-only matches
   - Test files with specialized filtering

3. Optimize performance:
   - Profile the new flow to identify bottlenecks
   - Optimize the new merging function for large result sets

## Milestone 5: Update Documentation and UI Integration
**Objective**: Ensure all documentation and interfaces reflect new behavior

### Action Steps:
1. Update code comments and documentation:
   - Add clear descriptions of the new merging process
   - Update any diagrams or flowcharts of the search process

2. Test UI presentation:
   - Verify that merged blocks display correctly
   - Check that code highlighting works with merged blocks

3. Update READMEs and other user-facing documentation:
   - Note changes in behavior if visible to users
   - Describe any new configuration options

## Implementation Notes

1. **Backward Compatibility**: We should maintain the existing `merge_code_blocks` function for other potential use cases, but modify its usage in the search flow.

2. **Performance Considerations**: Moving merging after limiting could significantly improve performance for large code bases, as we'll process fewer blocks.

3. **Scoring Changes**: We'll need to decide how to handle ranking scores when merging blocks. Options include:
   - Taking the maximum score among merged blocks
   - Computing a weighted average based on block sizes
   - Recalculating scores for the new merged content

4. **Debugging Support**: Maintain or enhance the current debug logging to help understand the new merging process.

## Risks and Mitigation

1. **Risk**: Merging after ranking could change search result ordering that users expect.
   **Mitigation**: Add configuration to switch between old and new behavior during transition.

2. **Risk**: Post-limit merging might create larger chunks than expected.
   **Mitigation**: Implement additional checks to ensure merged blocks don't exceed reasonable size.

3. **Risk**: The number of results after merging might be smaller than the requested limit.
   **Mitigation**: Consider adjusting the initial limit to account for this reduction.
