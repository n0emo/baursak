local m = {}

function m.greet(name)
	print(string.format("Hello, module %s!", name))
end

return m
