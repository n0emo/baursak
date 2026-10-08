-- Raise an error if cargo or rustc are not available
assert_cmd("cargo", "rustc")

-- Define a task if command available
if has_cmd("lua-language-server") then
	task("doc", [[lua-language-server --doc ./meta]])
end
