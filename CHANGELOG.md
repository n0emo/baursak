# Change Log

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

## [Unreleased]

### Added

- Loading Lua task definition files
- Basic shell tasks
- Basic function tasks
- Lua runs with only the safe standard libraries loaded
- Task dependencies with parallel execution
- LuaLS definition file generation
- `sh` function that runs command in shell
- `run` function that runs task with specified argumants
- `is_plat` function that checks if current platform matches one of the argument
- `has_cmd` and `assert_cmd` for checking if command is available
- Discovery of parent configs
- Support for `baursak.lua`, `.baursak.lua`, `baursak/tasks.lua`, `.baursak/tasks.lua`
- Tasks run in the directory of the file that defines them
- Shell commands are printed before execution
