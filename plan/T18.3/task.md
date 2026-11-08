# T18.3. Terminal sessions

`kerf-terminal`: terminals the core owns: a PTY per tab (ConPTY on Windows), started only after the trust check, a VT screen model, input, resize, close, a 4 MiB output cap, and tabs a plugin opened through `ide.terminal` marked as such. Done when tests run a shell command through the PTY on macOS and Linux, read the screen back, hit the cap, and refuse a spawn in an untrusted folder; the same through `kerf_terminal_*`.
