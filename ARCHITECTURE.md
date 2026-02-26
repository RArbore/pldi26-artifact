# Architecture

This repository is organized into one crate (Rust-speak for package), called `opt-eqsat`. It contains the following Rust files:

- build.rs: boilerplate for running the lalrpop parser generator.
- src/lib.rs: boilerplate for specifying the Rust modules in the crate.
- src/grammar.lalrpop: LR(1) grammar for the simple language that the tool analyzes.
- src/ast.rs: abstract syntax tree of the language parsed by the grammar.
- src/domains.rs: interval domain and a simple union-find.
- src/ssa.rs: SSA e-graphs, translating ASTs into SSA, and a canonical interpreter for SSA graph programs.
- src/analyses.rs: worklist-based e-class analysis algorithm and wrappers for performing standard (pessimistic) e-class analysis or optimistic e-class analysis.
- src/rewrite.rs: arithmetic rewrites on SSA e-graphs, standard equality saturation, and optimistic equality saturation.
- src/generate.rs: generate random programs at the AST level for testing and benchmarking purposes.
- src/bin/experiment.rs: benchmark optimistic e-class analysis on generated programs - note that the original version of the paper makes no claims about the performance of our tool.

To run the full test suite, including testing the soundness of the analysis on generated programs, run `cargo test --release`. To run the "torture" generated test, run `cargo test --release torture -- --ignored --nocapture` - this test is ignored by default because it tests the soundness of the analysis on 100000 programs, which takes several minutes.
