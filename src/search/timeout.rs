use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Starts a timeout thread that will terminate the process if the timeout is reached.
/// Returns a handle to the timeout thread that can be used to stop it.
pub fn start_timeout_thread(timeout_seconds: u64) -> Arc<AtomicBool> {
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = should_stop.clone();

    thread::spawn(move || {
        let mut elapsed_seconds = 0;
        while elapsed_seconds < timeout_seconds {
            // Check if we should stop the timeout thread
            if should_stop_clone.load(Ordering::SeqCst) {
                return;
            }

            // Sleep for 1 second
            thread::sleep(Duration::from_secs(1));
            elapsed_seconds += 1;
        }

        // Timeout reached, print a message and terminate the process
        eprintln!(
            "Search operation timed out after {} seconds",
            timeout_seconds
        );
        std::process::exit(1);
    });

    should_stop
}
