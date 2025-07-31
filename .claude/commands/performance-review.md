You goal is to measure and improve performance.

First run `cargo build --release` and remember the current performance: DEBUG=1 ./target/release/probe search "yaml workflow agent multi-agent user input" ~/go/src/semantic-kernel/ --max-tokens 10000 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'

Print it to the user.

Now that you have a baseline, find all the steps which take more then 1 second, and run the seaprate @architecture-agent for each, to plan if we can significantly improve performance. For each suggestion measure confidence. If confidence is high, add it to the detailed plan, if not, say that it is already performance enough.

Once you went though all the steps and build solid plan, I want you to start implementing it in a separate agent. 
But always explicitly ask user before each next implementation.

Each change should be measured, and compared with our baseline. You can add more debugging to search timing information, or making it more detailed if needed.
Once each change implemented, it should be commited as a separate commit.

We do care about backward compatibility, about determenistic outputs as well. Be careful. Validate each change by re-running all the tests..
