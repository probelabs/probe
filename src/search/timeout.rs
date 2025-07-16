use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Starts a timeout thread that will terminate the process if the timeout is reached.
/// Returns a handle to the timeout thread that can be used to stop it.
pub fn start_timeout_thread(timeout_seconds: u64) -> Arc<AtomicBool> {
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = should_stop.clone();

    // For testing purposes, check if we're running in a test environment
    let is_test = std::env::var("RUST_TEST_THREADS").is_ok();

    // Use a shorter sleep interval for tests to make timeouts more reliable
    let sleep_interval = if is_test {
        Duration::from_millis(10) // 100ms for tests
    } else {
        Duration::from_secs(1) // 1 second for normal operation
    };

    thread::spawn(move || {
        let mut elapsed_time = Duration::from_secs(0);
        let timeout_duration = Duration::from_secs(timeout_seconds);

        while elapsed_time < timeout_duration {
            // Check if we should stop the timeout thread
            if should_stop_clone.load(Ordering::SeqCst) {
                return;
            }

            // Sleep for the interval
            thread::sleep(sleep_interval);
            elapsed_time += sleep_interval;
        }

        // Timeout reached, print a message and terminate the process
        eprintln!("Search operation timed out after {timeout_seconds} seconds");
        std::process::exit(1);
    });

    should_stop
}
