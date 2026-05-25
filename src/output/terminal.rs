use colored::Colorize;

use crate::types::ResultsCtx;

/// Print query results to the terminal with colored output.
pub fn print_terminal(results: Vec<ResultsCtx>, enable_line_numbers: bool) {
    let mut counter = 0;
    let mut prints = Vec::new();

    for r in results {
        let line = r.source[..r.result.start_offset()].matches('\n').count() + 1;

        let fmt_reason = format!(" {} ", r.reason).bold().on_blue();
        let fmt_issue = format!(" {} ", r.issue).bold().on_purple();

        prints.push(format!(
            "{} : {}\n{}:{}\n{}",
            fmt_reason,
            fmt_issue,
            r.path.bold(),
            line,
            r.result.display(&r.source, 5, 5, enable_line_numbers)
        ));
        counter += 1;
    }

    println!("{} {}", counter, "matches".bold().red());
    for p in prints {
        println!("{p}");
    }
}
