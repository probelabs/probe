# MCP Server Integration Tests

This directory contains integration tests for the code-search MCP server. These tests verify that the server correctly implements the MCP protocol and provides the expected functionality.

## Test Files

- `server_simple_test.rs`: Simple integration tests for the MCP server
  - Tests server initialization and capabilities
  - Tests the search_code tool functionality
  - Tests error handling for invalid requests
  - Tests edge cases like empty queries
  - Tests search with various options

## Running the Tests

You can run the integration tests using the provided script:

```bash
./scripts/run_integration_tests.sh
```

This script will:
1. Build the project in debug mode
2. Run all integration tests

Alternatively, you can run the tests directly using Cargo:

```bash
# Run all integration tests
cargo test --test server_simple_test

# Run a specific test
cargo test --test server_simple_test test_server_basic_functionality
```

## Test Structure

Each test follows a similar pattern:

1. Start the server process
2. Create a client and connect to the server
3. Initialize the client
4. Send requests to the server
5. Verify the responses
6. Clean up resources

The tests use the `TestServer` struct to manage the server process lifecycle, ensuring that the server is properly started before the test and terminated after the test.

## Adding New Tests

To add a new test:

1. Add a new test function to one of the existing test files, or create a new test file
2. Follow the pattern of existing tests
3. Make sure to properly initialize the client before sending requests
4. Add assertions to verify the expected behavior
5. Update the run_integration_tests.sh script if necessary

## Troubleshooting

If the tests fail, check the following:

- Make sure the server is built in debug mode (`cargo build`)
- Check the server logs for any errors
- Increase the sleep duration in the `TestServer::new()` function if the server needs more time to start up
- Set `RUST_BACKTRACE=1` to get more detailed error information
