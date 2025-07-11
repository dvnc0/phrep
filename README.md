# phrep

A powerful grep-like tool for searching PHP codebases with function/method context awareness.

## What is phrep?

Phrep lets PHP developers search PHP codebases more effectively by providing context about the functions and methods where matches are found. It speeds up debugging and code analysis by showing you exactly where in your code structure a search term appears.

## Installation

Download the compiled binary for your platform:

```bash
# Replace [version] with the desired version
wget https://tools.dvnc0.com/[version]
mv [version] phrep
chmod +x phrep
```

Prebuilt binaries are available in the build directory at https://tools.dvnc0.com/[version]

### Current Versions
- phrep-v0.0.0-poc-x86_64-linux
- phrep-v0.1.0-x86_64-linux
- phrep-v0.1.1-x86_64-linux
- phrep-v0.1.2-x86_64-linux
- phrep-v0.1.3-x86_64-linux
- phrep-v0.2.0-x86_64-linux

To download the newest version use:

```bash
wget https://tools.dvnc0.com/phrep-v0.2.0-x86_64-linux
mv phrep-v0.2.0-x86_64-linux /usr/local/bin/phrep
chmod +x /usr/local/bin/phrep
```

Or you can clone this repo and compile yourself.

*Note: This tool is currently in beta.*

## Usage

```
phrep [OPTIONS] <QUERY>
```

### Arguments

- `<QUERY>`: The string or pattern to search for (supports regex)

### Search Modes

Phrep offers three different search modes:

#### 1. Basic Search (Default)

Searches for matches within PHP functions and class methods, showing the function name and context.

```bash
# Basic search
phrep "someString"

# Basic search in a specific directory
phrep "someString" --dir /path/to/project

# Basic search with full method bodies shown
phrep "someString" --print-method
```

Output format: `filename:line: function_name() → matching line`

#### 2. Grep Style Search

Works like traditional grep, finding matches in files without function context.

```bash
phrep "someString" --grep
# or
phrep "someString" -g
```

Output format: `filename:line → matching line`

#### 3. Method Search

Searches for method/function names that match the query and prints their entire body.

```bash
phrep "someMethodName" --method-search
# or
phrep "someMethodName" -m
```

Output format: `filename:line: function_name(parameters):return_type → function body`

### Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--dir` | `-d` | Directory to search recursively | Current directory (`.`) |
| `--file` | `-f` | File pattern to search | `.php` (all PHP files) |
| `--print-method` | `-p` | Print full method body in basic search | `false` |
| `--grep` | `-g` | Mimic grep search | `false` |
| `--method-search` | `-m` | Search for method names matching the query | `false` |
| `--exclude-dirs` | `-e` | Comma-separated list of directories to exclude | `vendor,cache,logs` |
| `--help` | `-h` | Print help information | |
| `--version` | `-V` | Print version information | |

## Examples

### Find all usages of a variable inside functions

```bash
phrep "$someVar"
```

### Find all controller methods that handle POST requests

```bash
phrep "POST" --dir app/Controllers
```

### Locate all methods that perform database queries

```bash
phrep "query" --dir app
```

### Find all methods that might handle file uploads

```bash
phrep "upload" --method-search
```

### Search only in model files

```bash
phrep "save" --file Model.php
```

### Exclude additional directories

```bash
phrep "config" --exclude-dirs "vendor,cache,logs,tests,node_modules"
```

## Features

- **Function context** - See which function/method contains your search term
- **Regex support** - Use regular expressions for advanced pattern matching
- **Color-coded output** - Files in blue, function names in yellow, matches in red
- **Smart home directory handling** - Paths starting with your home directory are displayed with `~`
- **Flexible search scopes** - Target specific files or directories, exclude others