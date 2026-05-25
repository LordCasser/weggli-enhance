/*
Copyright 2021 Google LLC

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use clap::{App, Arg};
use simplelog::*;
use std::path::{Path, PathBuf};

pub struct Args {
    pub code_path: PathBuf,
    pub rule_path: PathBuf,
    pub output_path: Option<String>,
    pub extensions: Vec<String>,
    pub limit: bool,
    #[allow(dead_code)]
    pub cpp: bool, // C++ mode reserved for future cross-platform support
    pub unique: bool,
    pub force_color: bool,
    pub force_query: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub enable_line_numbers: bool,
}

const NAME: &str = "weggli-enhance";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Parse command arguments and return them inside the Args structure.
/// The clap crate handles program exit and error messages for invalid arguments.
pub fn parse_arguments() -> Args {
    let matches = App::new(NAME.to_owned() + " " + VERSION)
        .about(help::ABOUT)
        .setting(clap::AppSettings::ArgRequiredElseHelp)
        .setting(clap::AppSettings::UnifiedHelpMessage)
        .template(help::TEMPLATE)
        .help_message("Prints help information.")
        .version_message("Prints version information.")
        .arg(
            Arg::with_name("RULES")
                .help("A file or directory to search rules.")
                .long_help(help::RULES)
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("PATH")
                .help("A file or directory to search.")
                .long_help(help::PATH)
                .required(true)
                .index(2),
        )
        .arg(
            Arg::with_name("output")
                .long("output")
                .short("o")
                .help("Output results to <path> in SARIF format.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("v")
                .long("verbose")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity."),
        )
        .arg(
            Arg::with_name("extensions")
                .long("extensions")
                .short("e")
                .takes_value(true)
                .multiple(true)
                .help("File extensions to include in the search."),
        )
        .arg(
            Arg::with_name("limit")
                .long("limit")
                .short("l")
                .takes_value(false)
                .help("Only show the first match in each function."),
        )
        // .arg(
        //     Arg::with_name("cpp")
        //         .short("X")
        //         .long("cpp")
        //         .takes_value(false)
        //         .help("Enable C++ mode."),
        // )
        .arg(
            Arg::with_name("color")
                .short("C")
                .long("color")
                .takes_value(false)
                .help("Force enable color output."),
        )
        .arg(
            Arg::with_name("force")
                .long("force")
                .short("f")
                .takes_value(false)
                .help("Force a search even if the queries contains syntax errors."),
        )
        .arg(
            Arg::with_name("unique")
                .long("unique")
                .short("u")
                .takes_value(false)
                .help("Enforce uniqueness of variable matches.")
                .long_help(help::UNIQUE),
        )
        .arg(
            Arg::with_name("exclude")
                .long("exclude")
                .takes_value(true)
                .multiple(true)
                .help("Exclude files that match the given regex."),
        )
        .arg(
            Arg::with_name("include")
                .long("include")
                .takes_value(true)
                .multiple(true)
                .help("Only search files that match the given regex."),
        )
        .arg(
            Arg::with_name("line-numbers")
                .long("line-numbers")
                .short("n")
                .takes_value(false)
                .help("Enable line numbers"),
        )
        .get_matches();

    let helper = |option_name| -> Vec<String> {
        if let Some(v) = matches.values_of(option_name) {
            v.map(|v| v.to_string()).collect()
        } else {
            vec![]
        }
    };

    let level = match matches.occurrences_of("v") {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };

    let _ = SimpleLogger::init(level, Config::default());


    let directory_code = Path::new(matches.value_of("PATH").unwrap_or("."));
    let directory_rule = Path::new(matches.value_of("RULES").unwrap_or("."));
    let directory_output = matches.value_of("output").map(|s| s.to_string());

    let code_path = if directory_code.is_absolute() {
        directory_code.to_path_buf()
    } else {
        std::env::current_dir().unwrap().join(directory_code)
    };

    let rules_path = if directory_rule.is_absolute() {
        directory_rule.to_path_buf()
    } else {
        std::env::current_dir().unwrap().join(directory_rule)
    };

    let limit = matches.occurrences_of("limit") > 0;

    let unique = matches.occurrences_of("unique") > 0;

    let cpp = matches.occurrences_of("cpp") > 0;

    let force_color = matches.occurrences_of("color") > 0;

    let extensions = {
        let e = helper("extensions");
        if e.is_empty() {
            if !cpp {
                vec!["c".to_string(), "h".into()]
            } else {
                vec![
                    "cc".to_string(),
                    "cpp".into(),
                    "h".into(),
                    "cxx".into(),
                    "hpp".into(),
                ]
            }
        } else {
            e
        }
    };

    let exclude = helper("exclude");
    let include = helper("include");

    let force_query = matches.occurrences_of("force") > 0;

    let enable_line_numbers = matches.occurrences_of("line-numbers") > 0;

    Args {
        code_path,
        rule_path: rules_path,
        output_path: directory_output,
        extensions,
        limit,
        cpp,
        unique,
        force_color,
        force_query,
        include,
        exclude,
        enable_line_numbers,
    }
}

mod help {
    pub const ABOUT: &str = "\
 weggli is a semantic search tool for C and C++ codebases.
 It is designed to quickly find interesting code pattern in large codebases.
 
 Use -h for short descriptions and --help for more details.
 
 Homepage: https://github.com/LordCasser/weggli-enhance";

    pub const TEMPLATE: &str = "\
 {about}
 
 USAGE: {usage}
 
 ARGS:
 {positionals}
 
 OPTIONS:
 {unified}";

    pub const RULES: &str = "\
 A YAML file or directory containing YAML rule files that define search patterns.
 Each rule file specifies one or more patterns with optional regex constraints.
 
 Patterns use weggli's query language, which closely resembles C and C++ with
 a small number of extra features.
 
 For example, the pattern '{_ $buf[_]; memcpy($buf,_,_);}' will
 find all calls to memcpy that directly write into a stack buffer.
 
 Besides normal C and C++ constructs, weggli's query language
 supports the following features:
 
 _        Wildcard. Will match on any AST node. 

 __       Variadic wildcard (argument list only). Matches zero or more
          arguments at this position. When present, argument count
          checking switches from exact to minimum mode.
          Example: `$f(__, $x, __)` matches `$f` where `$x` is a
          direct argument at any position. Supports multiple `__` in
          a single argument list for multi-parameter scenarios.
 
 $var     Variables. Can be used to write queries that are independent
          of identifiers. Variables match on identifiers, types,
          field names or namespaces. The --unique option
          optionally enforces that $x != $y != $z within a single match.
          Regex constraints can be specified per-variable in the YAML rules
          to enforce that the variable must match (or not match) a
          regular expression.
 
 _(..)    Subexpressions. The _(..) wildcard matches on arbitrary
          sub expressions. This can be helpful if you are looking for some
          operation involving a variable, but don't know more about it.
          For example, _(test) will match on expressions like test+10,
          buf[test->size] or f(g(&test));
 
 not:     Negative sub queries. Only show results that do not match the
          following sub query. For example, '{not: $fv==NULL; not: $fv!=NULL *$v;}'
          would find pointer dereferences that are not preceded by a NULL check.

 strict:  Enable stricter matching. This turns off statement unwrapping and greedy
          function name matching. For example 'strict: func();' will not match
          on 'if (func() == 1)..' or 'a->func()' anymore. 
 
 weggli automatically unwraps expression statements in the query source 
 to search for the inner expression instead. This means that the query `{func($x);}` 
 will match on `func(a);`, but also on `if (func(a)) {..}` or  `return func(a)`. 
 Matching on `func(a)` will also match on `func(a,b,c)` or `func(z,a)`. 
 Similarly, `void func($t $param)` will also match function definitions 
 with multiple parameters. 
 
 Multiple patterns within a rule allow searching across functions or type
 definitions. Use the YAML rules format (see examples) for multi-pattern rules.
 ";

    pub const PATH: &str = "\
 Input directory or file to search. By default, weggli will search inside 
 .c and .h files for the default C mode or .cc, .cpp, .cxx, .h and .hpp files when
 executing in C++ mode (using the --cpp option).
 Alternative file endings can be specified using the --extensions=h,c (-e) option.
 
 When combining weggli with other tools or preprocessing steps, 
 files can also be specified via STDIN by setting the directory to '-' 
 and piping a list of filenames.
 ";
    pub const UNIQUE: &str = "\
 Enforce uniqueness of variable matches.
 By default, two variables such as $a and $b can match on identical values.
 For example, the query '$x=malloc($a); memcpy($x, _, $b);' would
 match on both
 
 void *buf = malloc(size);
 memcpy(buf, src, size);
 
 and
 
 void *buf = malloc(some_constant);
 memcpy(buf, src, size);
 
 Using the unique flag would filter out the first match as $a==$b.
 ";
}
