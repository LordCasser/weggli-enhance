# weggli-enhance

![weggli-enhance example](example.png)

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE-APACHE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

## Why weggli-enhance

- **Feature Enhance:** YAML-based multi-pattern rule input with regex constraints — addressing [weggli-rs#76](https://github.com/weggli-rs/weggli/issues/76)
- **Accuracy Enhance:** Corrected argument count enforcement in AST matching; added `__` variadic wildcard for flexible argument position matching
- **Output Enhance:** SARIF output support for CI/CD integration
- **Cross Platform:** Windows, macOS, Linux

---

## Introduction

weggli is a fast and robust semantic search tool for C and C++ codebases. It is designed to help security researchers identify interesting functionality in large codebases.

weggli performs pattern matching on Abstract Syntax Trees based on user-provided queries. Its query language resembles C and C++ code, making it easy to turn interesting code patterns into queries.

weggli-enhance extends the original [weggli](https://github.com/googleprojectzero/weggli) with YAML-based multi-pattern rules, regex constraints, SARIF output, and additional query language features.

weggli is inspired by great tools like [Semgrep](https://semgrep.dev/), [Coccinelle](https://coccinelle.gitlabpages.inria.fr/website/), [joern](https://joern.readthedocs.io/en/latest/) and [CodeQL](https://securitylab.github.com/tools/codeql), but makes some different design decisions:

- **Minimal setup**: weggli should work _out-of-the-box_ against most software you will encounter. It does not require the ability to build the software and can work with incomplete sources or missing dependencies.

- **Interactive**: Designed for interactive usage and fast query performance. Most of the time, a weggli query will be faster than a grep search. The goal is to enable an interactive workflow where quick switching between code review and query creation/improvement is possible.

- **Greedy**: Pattern matching is designed to find as many useful matches as possible. While this increases the risk of false positives it simplifies query creation. For example, `$x = 10;` will match both assignment expressions (`foo = 10;`) and declarations (`int bar = 10;`).

---

## Quick Start

### Install

```sh
cargo install weggli-enhance
```

### Build from Source

```sh
git clone https://github.com/LordCasser/weggli-enhance.git
cd weggli-enhance
cargo build --release
./target/release/weggli-enhance --help
```

---

## Usage

```
USAGE: weggli-enhance [OPTIONS] <RULES> <PATH>

ARGS:
    <RULES>    A YAML rule file (or directory of YAML files) defining search patterns.
    <PATH>     Input directory or file to search.

OPTIONS:
    -e, --extensions <ext>...   File extensions to include (default: c,h)
    -o, --output <path>         Output results in SARIF format
    -u, --unique                Enforce uniqueness of variable matches
    -l, --limit                 Only show the first match in each function
    -n, --line-numbers          Enable line numbers
    -C, --color                 Force enable color output
    -v, --verbose               Verbose output
    -V, --version               Print version
    -h, --help                  Print help
```

### YAML Rule File Format

Each rule file contains an `issue` identifier and one or more `rules`:

```yaml
issue: "my-rule-name"
description: "What this rule detects"
rules:
  - reason: "CVE-2024-XXXXX"
    regexes:
      - "func=^decode_"
    patterns:
      - |
        _ $func(_ $buf) {
            memcpy($buf, _, _);
        }
  - reason: "variadic-argument-tracking"
    regexes: []
    patterns:
      - |
        _ $wrapper(_* $param) {
            $callee(__, $param, __);
        }
```

- **regexes**: Optional constraints on variable bindings (e.g., `func=^decode_` enforces `$func` to match identifiers starting with `decode_`; `!buf=^user_` enforces `$buf` does NOT start with `user_`).
- **patterns**: One or more weggli query patterns. Multi-pattern rules require coherent variable bindings across all patterns.
- **reason**: Human-readable label shown in output for each match.

---

## Query Language Reference

weggli's query language closely resembles C and C++ with the following extensions:

| Syntax    | Description |
|-----------|-------------|
| `_`       | **Wildcard.** Matches any single AST node. In argument lists, matches exactly one argument at a specific position. |
| `__`      | **Variadic wildcard** (argument list only). Matches **zero or more** arguments. Switches argument count checking from exact to minimum mode. Supports multiple `__` in a single argument list. |
| `$var`    | **Variable.** Matches identifiers, types, field names, or namespaces. `--unique` enforces `$x != $y != $z`. Regex constraints can be applied per-variable. |
| `_(..)`   | **Subexpression.** Recursively matches arbitrary sub-expressions. `_(test)` matches `test+10`, `buf[test->size]`, or `f(g(&test))`. |
| `not:`    | **Negative subquery.** Filters out results that match the following subquery. |
| `strict:` | **Strict mode.** Disables statement unwrapping and greedy function name matching. |

### Wildcard Semantics

`_` vs `__` comparison in argument lists:

```c
// Source code:
my_func(para, x1, x2, x3);   // 4 arguments
```

| Query Pattern | Matches? | Reason |
|---------------|----------|--------|
| `my_func(para, _, _, _)` | Only pos 0 | `_` is position-specific |
| `my_func(_, para, _, _)` | Only pos 1 | `_` is position-specific |
| `my_func(__, para, __)` | **Pos 0, 1, 2, 3** | `__` matches zero-or-more |
| `my_func(_(para))` | Also matches `x->para` | Subexpression is recursive |

### Variadic Wildcard `__` in Depth

The `__` wildcard enables matching a specific argument at **any position** in a function call while maintaining precision:

```c
// Source code:
void func(void *para) {
    my_func(para, x1, x2, x3);   // [1] para at position 0
    my_func(x1, para, x2, x3);   // [2] para at position 1
}
```

**Query** (YAML rule):

```yaml
patterns:
  - |
    _ $func(_* $param) {
        $func2(__, $param, __);
    }
```

Both [1] and [2] are matched — `$param` is found as a **direct argument** at any position.

**Multiple `__`** is supported for multi-parameter scenarios:

```c
// Matches calls where a appears before b, with any arguments in between:
ordered_func(a, __, b);

// Matches calls where x and y appear in order, separated by any args:
$f(x, __, y, __);
```

**Important:** `__` only matches **direct arguments**, not sub-expressions. For matching inside complex expressions, use `_(...)` subexpression wildcards.

---

## Examples

### Basic: Stack-buffer memcpy

```yaml
# rules/memcpy_stack.yaml
issue: "stack-buffer-memcpy"
description: "Calls to memcpy that write directly into a stack buffer"
rules:
  - reason: "potential overflow"
    patterns:
      - |
        {
            _ $buf[_];
            memcpy($buf, _, _);
        }
```

```sh
weggli-enhance rules/memcpy_stack.yaml ./target/src
```

### Argument Tracking with `__`

```yaml
# rules/arg_track.yaml
issue: "argument-tracking"
description: "Track a parameter through multiple function calls at arbitrary positions"
rules:
  - reason: "data-flow"
    patterns:
      - |
        _ $wrapper(_* $data) {
            $callee_a(__, $data, __);
            $callee_b(__, $data, __);
        }
```

### Negative Matching

```yaml
# rules/null_check.yaml
issue: "missing-null-check"
description: "Pointer dereferences without a preceding NULL check"
rules:
  - reason: "potential NPD"
    patterns:
      - |
        {
            not: $fv == NULL;
            not: $fv != NULL;
            *$v;
        }
```

### Strict Mode & Regex Constraints

```yaml
# rules/decode_funcs.yaml
issue: "decode-functions"
description: "Functions with 'decode' in their name"
rules:
  - reason: "decode-interesting"
    regexes:
      - "func=decode"
    patterns:
      - |
        _ $func(_) {
            _;
        }
```

### Multi-Pattern Across Functions

```yaml
# rules/snprintf_misuse.yaml
issue: "snprintf-misuse"
description: "Potentially vulnerable snprintf usage"
rules:
  - reason: "buffer-overflow"
    patterns:
      - |
        $ret = snprintf($b, _, _);
        $b[$ret] = _;
```

---

## Hacking Weggli

> Adapted from [@carstein](https://github.com/carstein)'s original weggli documentation.

### Architecture Overview

weggli is built on top of the [`tree-sitter`](https://tree-sitter.github.io/tree-sitter/) parsing library and its C and C++ grammars.

**Key modules:**

| Module | Purpose |
|--------|---------|
| `src/builder.rs` | Translates C AST of query pattern into tree-sitter S-expression queries |
| `src/query.rs` | Matching engine — `QueryTree`, `match_internal`, `process_match` |
| `src/capture.rs` | `Capture` enum — variables, wildcards, subqueries, argument count enforcement |
| `src/pipeline.rs` | Parallel file parsing and query execution (rayon) |
| `src/rules.rs` | YAML rule file loading and regex constraint processing |
| `src/result.rs` | Query result merging, deduplication, and display formatting |
| `src/output/` | Terminal and SARIF output formatting |
| `src/cli.rs` | CLI argument parsing |

### Life of a Query

1. **Rule Loading**: YAML files are parsed into `Rule` structs with patterns and regex constraints.
2. **Pattern Parsing**: Each pattern is parsed by tree-sitter's C grammar into an AST.
3. **Query Building** (`builder.rs`): The pattern AST is recursively translated into tree-sitter query S-expressions. Captures are created for variables (`$x`), wildcards (`_`, `__`), subexpressions (`_(...)`), and argument count checks (`CallExpQuery`).
4. **File Pipeline** (`pipeline.rs`): Target files are first filtered by identifier presence, then parsed by tree-sitter. Matching files are dispatched to worker threads.
5. **Query Execution** (`query.rs`): Tree-sitter queries are executed against each file's AST. Results are filtered through negative subqueries, variable coherence, and ordering constraints.
6. **Result Display**: Matches are merged, deduplicated, and output to terminal (with syntax highlighting) or SARIF format.

### Query Building Deep Dive

The `builder.rs` module is the core of weggli's query compilation. Key functions:

- **`build_identifier`**: Handles `_` (wildcard → `(_)`), `$var` (variable → capture with type alternatives), `__` (variadic → skipped, triggers minimum mode), and bare names (literal check capture).
- **`build_call_expr`**: Handles function calls, including `_(...)` subexpression wildcards (returns `SubWildQuery` capture).
- **`build`**: Core recursive function that walks the pattern AST and generates tree-sitter query S-expressions. Handles anchoring (`.` operator for argument ordering), argument count capture (`CallExpQuery`), and variadic mode.

### Argument Count Enforcement

The `CallExpQuery` capture enforces argument count consistency:

- **Exact mode** (no `__`): `source_arg_count == query_arg_count`
- **Minimum mode** (with `__`): `source_arg_count >= query_fixed_arg_count`

This is implemented in `query.rs` `process_match` using `named_child_count()`.

---

## License

- weggli-rs code: [Apache 2.0](LICENSE-APACHE)
- weggli-enhance code: See [Terms and Conditions](LICENSE)
