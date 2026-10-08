# Baursak - Tasty tasks written in Lua

Example `baursak.lua`:

```lua
-- Shell tasks
task("hello", [[echo "Hello world!"]])

-- Function tasks
task("bye", function(args)
    print("Bye, World!")
end)

-- Arguments and dependencies supported too!
task("run", {
    run = function(args)
        sh({"cargo", "run", args.name or "baursak"})
    end,
    depends = { "hello" },
    args = { "name" },
})
```

# Installation

```
cargo install --locked --git https://github.com/n0emo/baursak
```

# Usage

```
$ bk --help
Usage: bk [OPTIONS] [TASK] [ARGS]...

Arguments:
  [TASK]
  [ARGS]...

Options:
  -d, --directory <DIRECTORY>  [default: .]
  -f, --file <FILE>            [default: baursak.lua]
  -h, --help                   Print help

Commands:
      --list
      --definitions
```

# License

MIT, see [LICENSE](./LICENSE)
