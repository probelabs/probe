use anyhow::Result;
use serde_json::json;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
use url::Url;

/// Turn a JSON value into an LSP message
fn send_message(writer: &mut impl Write, value: &serde_json::Value) -> Result<()> {
    let bytes = value.to_string();
    write!(writer, "Content-Length: {}\r\n\r\n{}", bytes.len(), bytes)?;
    writer.flush()?;
    Ok(())
}

/// Read the next JSON-RPC message from rust-analyzer
fn read_message(reader: &mut BufReader<impl Read>) -> Result<serde_json::Value> {
    let mut header = String::new();
    reader.read_line(&mut header)?;
    if !header.starts_with("Content-Length:") {
        anyhow::bail!("unexpected header: {header}");
    }
    let len: usize = header["Content-Length:".len()..].trim().parse()?;
    reader.read_line(&mut String::new())?; // empty line after headers
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}

/// Find the position of a pattern in the file
fn find_pattern_position(content: &str, pattern: &str) -> Option<(u32, u32)> {
    for (line_idx, line) in content.lines().enumerate() {
        if let Some(col_idx) = line.find(pattern) {
            // Convert byte position to character position for LSP
            let char_col = line[..col_idx].chars().count() as u32;
            return Some((line_idx as u32, char_col));
        }
    }
    None
}

/// Try to read a message with a timeout
fn read_message_timeout(
    reader: &mut BufReader<impl Read>,
    timeout: Duration,
) -> Result<Option<serde_json::Value>> {
    use std::io::ErrorKind;
    
    let start = Instant::now();
    loop {
        // Check if we have data available
        match reader.fill_buf() {
            Ok(buf) if !buf.is_empty() => {
                // Data available, read the message
                return Ok(Some(read_message(reader)?));
            }
            Ok(_) => {
                // No data yet
                if start.elapsed() > timeout {
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                // Would block, check timeout
                if start.elapsed() > timeout {
                    return Ok(None);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e.into()),
        }
    }
}


fn main() -> Result<()> {
    println!("🚀 Starting simple LSP example...\n");

    // Get file and pattern from command line
    let path_str = std::env::args()
        .nth(1)
        .expect("usage: lsp-test <file.rs> <pattern>");
    let pattern = std::env::args()
        .nth(2)
        .expect("usage: lsp-test <file.rs> <pattern>");

    // Read file and find pattern
    let path = PathBuf::from(&path_str);
    let absolute_path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    
    let text = fs::read_to_string(&absolute_path)?;
    let (line, column) = find_pattern_position(&text, &pattern)
        .ok_or_else(|| anyhow::anyhow!("Pattern '{}' not found", pattern))?;
    
    println!("Found '{}' at line {}, column {}", pattern, line + 1, column);

    // Start rust-analyzer
    println!("Starting rust-analyzer...");
    let mut child = Command::new("rust-analyzer")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    // Find the project root (where Cargo.toml is located)
    let mut project_root = std::env::current_dir()?;
    let mut found_cargo_toml = false;
    
    // Search for Cargo.toml in current directory and parent directories
    loop {
        if project_root.join("Cargo.toml").exists() {
            found_cargo_toml = true;
            break;
        }
        if !project_root.pop() {
            break;
        }
    }
    
    // If no Cargo.toml found, use current directory
    if !found_cargo_toml {
        project_root = std::env::current_dir()?;
    }
    
    println!("Using project root: {}", project_root.display());
    
    // Initialize LSP
    println!("Initializing LSP...");
    let mut request_id = 1;
    
    send_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "initialize",
            "params": {
                "processId": null,
                "rootUri": Url::from_directory_path(&project_root)
                    .map_err(|_| anyhow::anyhow!("Failed to convert path"))?
                    .to_string(),
                "capabilities": {
                    "textDocument": {
                        "callHierarchy": {
                            "dynamicRegistration": false
                        }
                    },
                    "window": {
                        "workDoneProgress": true
                    },
                    "experimental": {
                        "statusNotification": true
                    }
                }
            }
        }),
    )?;

    // Wait for initialize response
    loop {
        let msg = read_message(&mut stdout)?;
        if msg["id"] == request_id {
            println!("Initialize response received");
            break;
        }
    }
    request_id += 1;

    // Send initialized notification
    send_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }),
    )?;

    // Open document
    println!("Opening document...");
    let uri = Url::from_file_path(&absolute_path)
        .map_err(|_| anyhow::anyhow!("Failed to convert file path"))?;
    
    send_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri.to_string(),
                    "languageId": "rust",
                    "version": 1,
                    "text": text
                }
            }
        }),
    )?;

    // Wait for rust-analyzer to analyze the workspace by monitoring progress
    // We use multiple strategies to detect when the server is ready:
    // 1. Wait for at least 2 cache priming completions (most reliable)
    // 2. Check for rust-analyzer/status "ready" notification (experimental, not always sent)
    // 3. Check for experimental/serverStatus with health "ok" (newer, not always sent)
    // 4. Monitor $/progress and window/workDoneProgress/create messages
    // 5. Wait for a period of silence after cache priming completes
    println!("Waiting for rust-analyzer to analyze the workspace...");
    
    let mut received_any_progress = false;
    let mut cache_priming_completed = false;
    let mut _flycheck_completed = false;
    let mut rust_analyzer_ready = false;
    let mut silence_start: Option<Instant> = None;
    let mut no_msg_count = 0;
    let start_wait = Instant::now();
    let max_wait = Duration::from_secs(30);
    let required_silence = Duration::from_secs(1);
    
    loop {
        let elapsed = start_wait.elapsed();
        
        // Try to read a message with a short timeout
        match read_message_timeout(&mut stdout, Duration::from_millis(100))? {
            Some(msg) => {
                // Debug: Print ALL messages from LSP server
                let _is_progress_msg = if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                    println!("\n[DEBUG] Received LSP notification/request: {}", method);
                    method == "$/progress" || method == "window/workDoneProgress/create"
                } else if msg.get("id").is_some() {
                    println!("\n[DEBUG] Received LSP response with id: {}", msg.get("id").unwrap());
                    // Responses to window/workDoneProgress/create are not progress messages themselves
                    false
                } else {
                    println!("\n[DEBUG] Received unknown LSP message type: {:?}", msg);
                    false
                };
                
                // Reset silence tracking when we receive any message  
                // (we'll start counting silence from the last message of any type)
                silence_start = None;
                
                // Check for rust-analyzer status notifications
                if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                    // Check for the older rust-analyzer/status notification
                    if method == "rust-analyzer/status" {
                        if let Some(params) = msg.get("params") {
                            if let Some(status) = params.as_str() {
                                println!("\n[INFO] rust-analyzer/status: {}", status);
                                if status == "ready" {
                                    println!("  ✅ rust-analyzer reports it is ready!");
                                    rust_analyzer_ready = true;
                                    // If we've already seen cache priming, we can proceed
                                    if cache_priming_completed {
                                        println!("  ✓ Cache priming already completed, server is fully ready!");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    
                    // Check for the newer experimental/serverStatus notification
                    if method == "experimental/serverStatus" {
                        println!("\n[INFO] experimental/serverStatus received");
                        if let Some(params) = msg.get("params") {
                            println!("  Status params: {:?}", params);
                            // Check if the server reports it's ready
                            if let Some(health) = params.get("health").and_then(|h| h.as_str()) {
                                if health == "ok" || health == "ready" {
                                    println!("  ✅ Server health is OK/ready!");
                                    rust_analyzer_ready = true;
                                    if cache_priming_completed {
                                        println!("  ✓ Cache priming already completed, server is fully ready!");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Check if it's a progress-related message
                if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                    if method == "$/progress" || method == "window/workDoneProgress/create" {
                        received_any_progress = true;
                        
                        // Print raw progress message for debugging
                        println!("\n[DEBUG] Raw progress message:");
                        println!("{}", serde_json::to_string_pretty(&msg).unwrap_or_else(|_| format!("{:?}", msg)));
                        
                        // Respond to window/workDoneProgress/create requests
                        if method == "window/workDoneProgress/create" {
                            if let Some(id) = msg.get("id") {
                                println!("[DEBUG] Responding to window/workDoneProgress/create request");
                                send_message(
                                    &mut stdin,
                                    &json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "result": null
                                    }),
                                )?;
                            }
                        }
                        
                        if let Some(params) = msg.get("params") {
                            if let Some(value) = params.get("value") {
                                if let Some(kind) = value.get("kind").and_then(|k| k.as_str()) {
                                    if let Some(title) = value.get("title").and_then(|t| t.as_str()) {
                                        if kind == "begin" {
                                            println!("  Started: {}", title);
                                        }
                                    }
                                    
                                    // Check for completion messages
                                    let token_str = if let Some(s) = params.get("token").and_then(|t| t.as_str()) {
                                        s
                                    } else {
                                        ""
                                    };
                                    
                                    if kind == "end" {
                                        println!("  Completed: {}", token_str);
                                        
                                        // Check if this is cachePriming completion (wait for second one for better stability)
                                        if token_str.contains("cachePriming") {
                                            if !cache_priming_completed {
                                                println!("  ✓ First cache priming completed, waiting for more...");
                                                cache_priming_completed = true;
                                                // If rust-analyzer already reported ready, we can proceed
                                                if rust_analyzer_ready {
                                                    println!("  ✓ rust-analyzer already reported ready, proceeding!");
                                                    break;
                                                }
                                            } else {
                                                println!("  ✓ Additional cache priming completed! Waiting 3 seconds for server to fully stabilize...");
                                                std::thread::sleep(Duration::from_secs(3));
                                                println!("  ✓ Server should be ready now!");
                                                break;
                                            }
                                        }
                                        
                                        // Check if this is flycheck completion
                                        if token_str.contains("flycheck") {
                                            println!("  ✓ Flycheck completed!");
                                            _flycheck_completed = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None => {
                no_msg_count += 1;
                
                // No message available - track silence period
                if silence_start.is_none() {
                    silence_start = Some(Instant::now());
                    if cache_priming_completed {
                        println!("[DEBUG] Starting post-cache-priming silence timer");
                    }
                }
                
                // Check if we've had enough silence after receiving progress messages
                if let Some(silence_time) = silence_start {
                    let silence_duration = silence_time.elapsed();
                    
                    // Print debug every 200ms
                    if no_msg_count % 2 == 0 {
                        println!("[DEBUG] Silence for {:?} (cache_priming: {})", 
                                 silence_duration, cache_priming_completed);
                    }
                    
                    // If cache priming completed and we've had 1 second of silence, proceed
                    // Note: rust-analyzer continues running background tasks indefinitely
                    if cache_priming_completed && silence_duration >= required_silence {
                        println!("\n✅ Cache priming completed with {:?} of silence - server is ready enough!", silence_duration);
                        break;
                    }
                    
                    // If we've received progress but cache priming didn't complete, wait for 2 seconds of silence
                    if received_any_progress && silence_duration >= Duration::from_secs(2) {
                        println!("\n⚠️ No cache priming completion detected, but no activity for {:?} - proceeding anyway", silence_duration);
                        break;
                    }
                }
                
                // Fallback: if we've waited long enough without any progress at all
                if elapsed > Duration::from_secs(5) && !received_any_progress {
                    println!("\n⚠️ No progress messages received after 5 seconds, proceeding...");
                    break;
                }
            }
        }
        
        // Check timeout
        if elapsed > max_wait {
            println!("Timeout reached, proceeding anyway...");
            break;
        }
    }
    
    // Small delay to ensure everything is settled
    std::thread::sleep(Duration::from_millis(100));

    // Prepare call hierarchy with retries
    println!("\nPreparing call hierarchy...");
    let mut prepare_response = None;
    let max_retries = 3;
    
    for retry in 0..max_retries {
        if retry > 0 {
            println!("  Retry #{} after waiting 1 second...", retry);
            std::thread::sleep(Duration::from_secs(1));
        }
        
        send_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "textDocument/prepareCallHierarchy",
                "params": {
                    "textDocument": { "uri": uri.to_string() },
                    "position": { "line": line, "character": column }
                }
            }),
        )?;

        // Get prepare response
        let response = loop {
            let msg = read_message(&mut stdout)?;
            if msg["id"] == request_id {
                break msg;
            }
        };
        request_id += 1;

        // Check if we got valid results
        if let Some(items) = response["result"].as_array() {
            if !items.is_empty() {
                prepare_response = Some(response);
                println!("  ✓ Call hierarchy prepared successfully!");
                break;
            }
        }
        
        if retry < max_retries - 1 {
            println!("  No results yet, will retry...");
        }
    }
    
    let response = match prepare_response {
        Some(r) => r,
        None => {
            println!("❌ No call hierarchy found at this position after {} attempts", max_retries);
            println!("   This might mean:");
            println!("   - The position is not on a function/method definition");
            println!("   - The LSP server needs more time to analyze the code");
            println!("   - Try searching for 'fn function_name' for better results");
            
            // Proper shutdown even when no results found
            println!("\nShutting down...");
            send_message(
                &mut stdin,
                &json!({
                    "jsonrpc": "2.0",
                    "method": "shutdown",
                    "id": request_id
                }),
            )?;
            
            // Wait for shutdown response
            loop {
                let msg = read_message(&mut stdout)?;
                if msg["id"] == request_id {
                    break;
                }
            }
            
            // Send exit notification
            send_message(
                &mut stdin,
                &json!({
                    "jsonrpc": "2.0",
                    "method": "exit"
                }),
            )?;
            
            return Ok(());
        }
    };

    let item = &response["result"][0];
    let function_name = item["name"].as_str().unwrap_or("<unknown>");
    println!("Found function: {}", function_name);

    // Get outgoing calls with retry
    println!("\nGetting outgoing calls...");
    let mut outgoing_response = None;
    
    for retry in 0..max_retries {
        if retry > 0 {
            println!("  Retry #{} for outgoing calls after 1 second...", retry);
            std::thread::sleep(Duration::from_secs(1));
        }
        
        send_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "callHierarchy/outgoingCalls",
                "params": { "item": item }
            }),
        )?;

        // Read outgoing calls response
        let response = loop {
            let msg = read_message(&mut stdout)?;
            if msg["id"] == request_id {
                break msg;
            }
        };
        request_id += 1;
        
        // Debug: show what we got
        if let Some(result) = response.get("result") {
            if let Some(calls) = result.as_array() {
                if !calls.is_empty() {
                    println!("  ✓ Found {} outgoing calls", calls.len());
                    outgoing_response = Some(response);
                    break;
                } else {
                    println!("  [DEBUG] Empty outgoing calls array returned");
                }
            } else {
                println!("  [DEBUG] Result is not an array: {:?}", result);
            }
        } else if let Some(error) = response.get("error") {
            println!("  [DEBUG] Error in response: {:?}", error);
        } else {
            println!("  [DEBUG] Unexpected response format: {:?}", response);
        }
    }

    // Get incoming calls with retry
    println!("\nGetting incoming calls...");
    let mut incoming_response = None;
    
    for retry in 0..max_retries {
        if retry > 0 {
            println!("  Retry #{} for incoming calls after 1 second...", retry);
            std::thread::sleep(Duration::from_secs(1));
        }
        
        send_message(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "callHierarchy/incomingCalls",
                "params": { "item": item }
            }),
        )?;

        // Read incoming calls response
        let response = loop {
            let msg = read_message(&mut stdout)?;
            if msg["id"] == request_id {
                break msg;
            }
        };
        request_id += 1;
        
        // Debug: show what we got
        if let Some(result) = response.get("result") {
            if let Some(calls) = result.as_array() {
                if !calls.is_empty() {
                    println!("  ✓ Found {} incoming calls", calls.len());
                    incoming_response = Some(response);
                    break;
                } else {
                    println!("  [DEBUG] Empty incoming calls array returned");
                }
            } else {
                println!("  [DEBUG] Result is not an array: {:?}", result);
            }
        } else if let Some(error) = response.get("error") {
            println!("  [DEBUG] Error in response: {:?}", error);
        } else {
            println!("  [DEBUG] Unexpected response format: {:?}", response);
        }
    };

    // Print results
    println!("\n📊 Call hierarchy for '{}':", function_name);
    
    let mut has_any_calls = false;
    
    if let Some(outgoing) = outgoing_response {
        if let Some(calls) = outgoing["result"].as_array() {
            if !calls.is_empty() {
                has_any_calls = true;
                println!("\n  Outgoing calls (this function calls):");
                for call in calls {
                    if let Some(to) = call["to"].as_object() {
                        let name = to["name"].as_str().unwrap_or("unknown");
                        println!("    → {}", name);
                    }
                }
            } else {
                println!("\n  Outgoing calls: (none found after {} retries)", max_retries);
            }
        }
    } else {
        println!("\n  Outgoing calls: (none found after {} retries)", max_retries);
    }

    if let Some(incoming) = incoming_response {
        if let Some(calls) = incoming["result"].as_array() {
            if !calls.is_empty() {
                has_any_calls = true;
                println!("\n  Incoming calls (functions that call this):");
                for call in calls {
                    if let Some(from) = call["from"].as_object() {
                        let name = from["name"].as_str().unwrap_or("unknown");
                        println!("    ← {}", name);
                    }
                }
            } else {
                println!("\n  Incoming calls: (none found after {} retries)", max_retries);
            }
        }
    } else {
        println!("\n  Incoming calls: (none found after {} retries)", max_retries);
    }
    
    if !has_any_calls {
        println!("\n  ℹ️  No calls found. This could mean:");
        println!("     - The function is not used/called anywhere");
        println!("     - The function doesn't call other functions");
        println!("     - The LSP server is still indexing the codebase");
    }

    // Shutdown
    println!("\nShutting down...");
    request_id += 1;
    send_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "shutdown",
            "id": request_id
        }),
    )?;
    
    // Wait for shutdown response
    loop {
        let msg = read_message(&mut stdout)?;
        if msg["id"] == request_id {
            println!("Shutdown acknowledged");
            break;
        }
    }

    // Send exit notification (no response expected)
    send_message(
        &mut stdin,
        &json!({
            "jsonrpc": "2.0",
            "method": "exit"
        }),
    )?;

    println!("Done!");
    Ok(())
}