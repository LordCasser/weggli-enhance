use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use rayon::iter::ParallelBridge;
use rayon::prelude::*;
use thread_local::ThreadLocal;
use tree_sitter::Tree;

use crate::types::{Options, ResultsCtx, WorkItem};
use crate::result::QueryResult;

/// Iterate over all paths in `files`, parse files that might contain a match for any of the queries
/// in `work` and send them to the next worker using `sender`.
pub fn parse_files_worker(
    files: Arc<Vec<PathBuf>>,
    sender: Sender<(Arc<String>, Tree, String)>,
    work: &[WorkItem],
) {
    let tl = ThreadLocal::new();

    // Arc::unwrap_or_clone avoids cloning when refcount is 1
    Arc::unwrap_or_clone(files)
        .into_par_iter()
        .for_each_with(sender, move |sender, path| {
            let maybe_parse = |path: &PathBuf| {
                let c = match fs::read(path) {
                    Ok(content) => content,
                    Err(_) => return None,
                };

                let source = String::from_utf8_lossy(&c);

                let potential_match = work.iter().any(
                    |WorkItem {
                         qt: _,
                         identifiers,
                         reason: _,
                         issue: _,
                     }| {
                        identifiers.iter().all(|i| source.find(i).is_some())
                    },
                );

                if !potential_match {
                    None
                } else {
                    let mut parser = tl
                        .get_or(|| RefCell::new(crate::get_parser()))
                        .borrow_mut();
                    parser.parse(source.as_bytes(), None).map(|tree| (tree, source.to_string()))
                }
            };
            if let Some((source_tree, source)) = maybe_parse(&path) {
                sender
                    .send((Arc::new(source), source_tree, path.display().to_string()))
                    .expect("failed to send AST to query worker");
            }
        });
}

/// Fetches parsed ASTs from `receiver`, runs all queries in `work` on them and
/// filters the results based on `--unique` and `--limit` switches.
/// Results are forwarded through the `results_tx` channel.
pub fn execute_queries_worker(
    receiver: Receiver<(Arc<String>, Tree, String)>,
    results_tx: Sender<ResultsCtx>,
    work: &[WorkItem],
    options: Options,
) {
    receiver.into_iter().par_bridge().for_each_with(
        results_tx,
        |results_tx, (source, tree, path)| {
            // For each query
            work.iter().enumerate().for_each(
                |(
                    i,
                    WorkItem {
                        qt,
                        identifiers: _,
                        reason,
                        issue,
                    },
                )| {
                    // Run query
                    let matches = qt.matches(tree.root_node(), &source);

                    if matches.is_empty() {
                        return;
                    }

                    // Enforce --unique: within a single match, all variable values must differ
                    let check_unique = |m: &QueryResult| {
                        if options.unique {
                            let mut seen = HashSet::new();
                            m.vars
                                .keys()
                                .map(|k| m.value(k, &source).unwrap())
                                .all(|x| seen.insert(x))
                        } else {
                            true
                        }
                    };

                    let mut skip_set = HashSet::new();

                    // Enforce --limit: only show the first match in each function
                    let check_limit = |m: &QueryResult| {
                        if options.limit {
                            skip_set.insert(m.start_offset())
                        } else {
                            true
                        }
                    };

                    // Forward match to results collector
                    let process_match = |m: QueryResult| {
                        results_tx
                            .send(ResultsCtx {
                                query_index: i,
                                result: m,
                                path: path.clone(),
                                source: source.clone(),
                                reason: reason.clone(),
                                issue: issue.clone(),
                            })
                            .expect("failed to send result to collector");
                    };

                    matches
                        .into_iter()
                        .filter(check_unique)
                        .filter(check_limit)
                        .for_each(process_match);
                },
            );
        },
    );
}
