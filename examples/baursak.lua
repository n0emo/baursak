local m = require("mod")

task("default", [[echo "Use --list to view a list of tasks"]])

task("bye", {
	run = [[echo "Bye, ${name:World}!"]],
	args = { "name" },
})

task("greet", {
	run = function(args)
		m.greet(args.name or "World")
	end,
	args = { "name" },
})

task("sleep-1", function()
	print("Sleeping...")
	sh("sleep 2")
	print("Done")
end)

task("build", function()
	print("Building...")
	sh("cargo build --release")
	print("Done")
end)

task("io", function()
	local p = io.popen('find "' .. "." .. '"')
	if p == nil then
		error("Could not open pipe 'find'")
	end

	for file in p:lines() do
		print(file)
	end
	p:close()
end)

task("all", {
	run = [[echo "Runned all"]],
	depends = { "bye", "greet", "sleep-1", "build" },
})

task("pwd", [[pwd]])
