# Roadmap

## Done

- [x] Tasks defined in Lua (shell and function tasks)
- [x] Task arguments and dependencies, run in parallel
- [x] Sandboxed Lua runtime
- [x] Builtin helpers: running commands, calling tasks, platform and command checks
- [x] Config discovery in parent directories
- [x] Editor support via generated LuaLS definitions

## Next

- [ ] Proper error handling
- [ ] Find Git Bash on Windows, falling back to PowerShell and cmd
- [ ] Write documentation for common use cases
- [ ] Load variables from `.env` files
- [ ] Private tasks
- [ ] Quiet tasks
- [ ] Allow `require`-ing Lua modules from baursak directories
- [ ] Workspace support (design TBD)

## Future

- [ ] Validate task arguments (unknown and missing required args)
- [ ] Make `sh` more convenient to use
- [ ] Show task descriptions in `--list`
- [ ] Run multiple tasks in one invocation
- [ ] Test on all supported OSes
- [ ] Make working directory configurable per task
- [ ] Shell completions
- [ ] Default values for arguments
- [ ] Task caching: declare inputs and outputs, skip tasks whose inputs haven't changed
- [ ] Builtin filesystem functions
- [ ] Prefix output of parallel tasks with the task name
- [ ] Make shell configurable globally and per task
- [ ] Watch mode: rerun tasks when their inputs change
- [ ] Call tasks from parent configs
- [ ] Variadic arguments
- [ ] Generate API documentation using lua-language-server
- [ ] Host documentation
- [ ] `--dry-run` flag that shows what would run without executing it
- [ ] Support shebangs in tasks
- [ ] Interactive fuzzy task picker
- [ ] Task aliases
- [ ] Run tasks by glob pattern
- [ ] Task templates
- [ ] Task confirmation prompt
- [ ] `--dump` flag that prints the current baursak file
