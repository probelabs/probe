You goal is to measure and improve performance.

First run `cargo build --release` and remember the current performance: DEBUG=1 probe search "query" ./path --max-results 10 2>/dev/null | sed -n '/=== SEARCH TIMING INFORMATION ===/,/====================================/p'

Print it to the user.

Now that you have a baseline, first all the steps which take more then 1 second, run the seaprate architecture agent, to plan if we can significantly improve performance. For each suggestion measure confidence. If confidence is high, add it to the detailed plan, if not, say that it is already performance enough.

Once you went though all the steps and build solid plan, I want you to start implementing it in a separate agent. 
Each change should be measured, and compared with our baseline. You can add more debugging to searchi timing information, or making it more detailed if needed.
Once each change implemented, it should be commited as a separate commit.
