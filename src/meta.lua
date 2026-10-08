---@meta

---@class Task
---@field run string|function
---@field depends string[]|nil
---@field args string[]|nil
local Task = {}

---@param name string
---@param task string|Task|function
function task(name, task) end

---@param command string[]
function sh(command) end

---@param name string
---@param args table|nil
function run(name, args) end
