-- Do different things in different platforms

task("x", function()
	if is_plat("windows") then
		print("We are on Windows!")
	elseif is_plat("linux", "macos") then
		print("We are on Linux or MacOS!")
	elseif is_plat("unix") then
		print("We are on UNIX!")
	end
end)

-- Register different tasks on different platforms

if is_plat("unix") then
	task("set-permissions", "chmod +x script.sh")
end

if is_plat("windows") then
	task("download-linux", "wsl --install")
end
