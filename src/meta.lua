---@meta

---@class Task
---@field run string|function
---@field depends string[]|nil
---@field args string[]|nil
local Task = {}

---Define a new task
---@param name string
---@param definition string|Task|function
function task(name, definition) end

---@param command string[]
function sh(command) end

---Run another task
---
---Example:
---
---```lua
---task("x", [{
---  run = [[echo "Hello, ${name}!"]],
---  args = { "name" },
---})
---
---task("y", function()
---  run("x", { name = "World" })
---end)
---```
---
---@param name string
---@param args table|nil
function run(name, args) end

---Checks if current platform is in the list of arguments
---
---Example:
---
---```lua
---if is_plat("unix") then
---  -- do UNIX stuff
---elseif is_plat("windows") then
--- -- do Windows stuff
---end
---```
---
---@param ... string
---@return boolean
function is_plat(...) end
