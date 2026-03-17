use core::cmp::max;
use core::fmt::Display;
use core::iter::zip;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

use colored::Colorize;
use rand::prelude::*;
use rand::rngs::Xoshiro128PlusPlus;
use tabled::{
    Table, Tabled,
    settings::{Alignment, Style, object::Rows},
};

use opt_eqsat::analyses::{dependents, outer_fixpoint, standard_eclass_analysis};
use opt_eqsat::domains::Interval;
use opt_eqsat::generate::generate;
use opt_eqsat::rewrites::optimistic_equality_saturation;
use opt_eqsat::ssa::{SSAValue, dce, interpret, naive_ssa_translation};

const NUM_PROGRAMS: usize = 100;
const SAMPLE_SIZE: u128 = 25;

#[derive(Debug, Clone)]
struct Absolute {
    absolute: f64,
}

impl Display for Absolute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}", self.absolute)
    }
}

impl From<f64> for Absolute {
    fn from(absolute: f64) -> Self {
        Absolute { absolute }
    }
}

#[derive(Debug, Clone)]
struct AbsoluteAndRelative {
    absolute: f64,
    relative: Vec<f64>,
}

impl Display for AbsoluteAndRelative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}", self.absolute)?;
        if !self.relative.is_empty() {
            write!(f, " ({:.2}x", self.relative[0])?;
            for relative in &self.relative[1..] {
                write!(f, ", {:.2}x", relative)?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Distribution {
    min: f64,
    median: f64,
    mean: f64,
    max: f64,
}

#[derive(Debug, Clone, Tabled)]
struct Row {
    stat: &'static str,
    standard_abstract_interpretation: Absolute,
    standard_e_class_analysis: Absolute,
    optimistic_e_class_analysis: AbsoluteAndRelative,
    num_visit_items_before: Absolute,
    num_visit_items_after: AbsoluteAndRelative,
    standard_abstract_interpretation_n: Absolute,
    optimistic_e_class_analysis_n: Absolute,
}

fn main() {
    let mut programs = vec![];
    let mut rng = Xoshiro128PlusPlus::seed_from_u64(10);
    while programs.len() < NUM_PROGRAMS {
        let func = generate(programs.len() * 50 + 1000, &mut rng);
        let (mut ssa, cfg) = naive_ssa_translation(&func);
        dce(&mut ssa, &cfg);
        if ssa.values.len() > 20
            && ssa.values.len() < 8000
            && let Some((block, output)) = interpret(&ssa, &cfg, &[], 1000)
        {
            programs.push((ssa, cfg, block, output));
        }
    }

    let mut max_component_heads = 0;
    let mut max_component_head_phis = 0;
    let mut max_outer_iters1 = 0;
    let mut max_outer_iters3 = 0;
    let mut num_outer_iters_is_two1 = 0;
    let mut num_outer_iters_is_two3 = 0;
    let mut avg_avg_visits_per_item1 = 0.0f64;
    let mut avg_avg_visits_per_item2 = 0.0f64;
    let mut avg_avg_visits_per_item3 = 0.0f64;
    let mut max_visits_per_item1 = 0.0f64;
    let mut max_visits_per_item2 = 0.0f64;
    let mut max_visits_per_item3 = 0.0f64;
    let mut avg_avg_micros_per_item1 = 0.0f64;
    let mut avg_avg_micros_per_item2 = 0.0f64;
    let mut avg_avg_micros_per_item3 = 0.0f64;
    let mut max_micros_per_item1 = 0.0f64;
    let mut max_micros_per_item2 = 0.0f64;
    let mut max_micros_per_item3 = 0.0f64;
    let mut num_e_nodes_per_program = vec![];
    let mut total_micros = vec![];
    let mut num_outer_iters = vec![];
    let mut table_distributions = vec![];
    for (idx, (mut ssa, mut cfg, block, output)) in programs.into_iter().enumerate() {
        let root = ssa.roots[&block];
        let deps = dependents(&ssa, &cfg);

        let (analyses, statistics1) = outer_fixpoint(&ssa, &cfg, &deps);
        assert!(!analyses.unreachable_blocks[&block]);
        assert!(Interval::from_constant(output).leq(&analyses.intervals[&root]));

        let time1 = Instant::now();
        for _ in 0..SAMPLE_SIZE {
            outer_fixpoint(&ssa, &cfg, &deps);
        }
        let time1 = time1.elapsed();

        optimistic_equality_saturation(&mut ssa, &mut cfg, 8, 1, 10000);
        let root = ssa.roots[&block];
        let deps = dependents(&ssa, &cfg);

        let (analyses, statistics2) = standard_eclass_analysis(&ssa, &cfg, &deps);
        assert!(!analyses.unreachable_blocks[&block]);
        assert!(Interval::from_constant(output).leq(&analyses.intervals[&root]));

        let time2 = Instant::now();
        for _ in 0..SAMPLE_SIZE {
            standard_eclass_analysis(&ssa, &cfg, &deps);
        }
        let time2 = time2.elapsed();

        let (analyses, statistics3) = outer_fixpoint(&ssa, &cfg, &deps);
        assert!(!analyses.unreachable_blocks[&block]);
        assert!(Interval::from_constant(output).leq(&analyses.intervals[&root]));

        let time3 = Instant::now();
        for _ in 0..SAMPLE_SIZE {
            outer_fixpoint(&ssa, &cfg, &deps);
        }
        let time3 = time3.elapsed();

        assert_eq!(statistics1.num_blocks, statistics2.num_blocks);
        assert_eq!(statistics2.num_blocks, statistics3.num_blocks);
        assert_eq!(statistics1.num_edges, statistics2.num_edges);
        assert_eq!(statistics2.num_edges, statistics3.num_edges);
        assert_eq!(
            statistics1.num_component_heads,
            statistics2.num_component_heads
        );
        assert_eq!(
            statistics2.num_component_heads,
            statistics3.num_component_heads
        );
        assert_eq!(statistics2.num_nodes, statistics3.num_nodes);
        let num_nodes = statistics1.num_nodes;
        let num_e_nodes = statistics2.num_nodes;
        let num_blocks = statistics1.num_blocks;
        let num_edges = statistics1.num_edges;
        let num_component_heads = statistics1.num_component_heads;
        let num_outer_iters1 = statistics1.num_loop_visits.len();
        let num_outer_iters2 = statistics2.num_loop_visits.len();
        assert_eq!(num_outer_iters2, 1);
        let num_outer_iters3 = statistics3.num_loop_visits.len();
        let avg_visits1 = statistics1.num_loop_visits.iter().sum::<usize>() / num_outer_iters1;
        let avg_visits2 = statistics2.num_loop_visits.iter().sum::<usize>() / num_outer_iters2;
        let avg_visits3 = statistics3.num_loop_visits.iter().sum::<usize>() / num_outer_iters3;
        let avg_micros1 = (time1.as_nanos() as f64 / 1000.0) / SAMPLE_SIZE as f64;
        let avg_micros2 = (time2.as_nanos() as f64 / 1000.0) / SAMPLE_SIZE as f64;
        let avg_micros3 = (time3.as_nanos() as f64 / 1000.0) / SAMPLE_SIZE as f64;
        let num_visit_items_before = num_nodes + num_blocks + num_edges;
        let num_visit_items_after = num_e_nodes + num_blocks + num_edges;
        let avg_visits_per_item1 = (avg_visits1 as f64) / (num_visit_items_before as f64);
        let avg_visits_per_item2 = (avg_visits2 as f64) / (num_visit_items_after as f64);
        let avg_visits_per_item3 = (avg_visits3 as f64) / (num_visit_items_after as f64);
        let avg_micros_per_outer1 = avg_micros1 / (num_outer_iters1 as f64);
        let avg_micros_per_outer2 = avg_micros2 / (num_outer_iters2 as f64);
        let avg_micros_per_outer3 = avg_micros3 / (num_outer_iters3 as f64);
        let avg_micros_per_outer_per_item1 =
            avg_micros_per_outer1 / (num_visit_items_before as f64);
        let avg_micros_per_outer_per_item2 = avg_micros_per_outer2 / (num_visit_items_after as f64);
        let avg_micros_per_outer_per_item3 = avg_micros_per_outer3 / (num_visit_items_after as f64);
        let num_component_head_phis = ssa
            .values
            .iter()
            .filter(|(value, _)| {
                if let SSAValue::Phi(v, _, _) = value
                    && cfg[v].iter().any(|pred| pred.2)
                {
                    true
                } else {
                    false
                }
            })
            .count();

        max_component_heads = max(max_component_heads, num_component_heads);
        max_component_head_phis = max(max_component_head_phis, num_component_head_phis);
        max_outer_iters1 = max(max_outer_iters1, num_outer_iters1);
        max_outer_iters3 = max(max_outer_iters3, num_outer_iters3);
        if num_outer_iters1 == 2 {
            num_outer_iters_is_two1 += 1;
        }
        if num_outer_iters3 == 2 {
            num_outer_iters_is_two3 += 1;
        }
        avg_avg_visits_per_item1 += avg_visits_per_item1;
        avg_avg_visits_per_item2 += avg_visits_per_item2;
        avg_avg_visits_per_item3 += avg_visits_per_item3;
        max_visits_per_item1 = max_visits_per_item1.max(avg_visits_per_item1);
        max_visits_per_item2 = max_visits_per_item2.max(avg_visits_per_item2);
        max_visits_per_item3 = max_visits_per_item3.max(avg_visits_per_item3);
        avg_avg_micros_per_item1 += avg_micros_per_outer_per_item1;
        avg_avg_micros_per_item2 += avg_micros_per_outer_per_item2;
        avg_avg_micros_per_item3 += avg_micros_per_outer_per_item3;
        max_micros_per_item1 = max_micros_per_item1.max(avg_micros_per_outer_per_item1);
        max_micros_per_item2 = max_micros_per_item2.max(avg_micros_per_outer_per_item2);
        max_micros_per_item3 = max_micros_per_item3.max(avg_micros_per_outer_per_item3);
        num_e_nodes_per_program.push(num_e_nodes);
        total_micros.push(avg_micros3);
        num_outer_iters.push(num_outer_iters3);

        table_distributions.push((
            avg_micros1,
            avg_micros2,
            avg_micros3,
            num_visit_items_before,
            num_visit_items_after,
            num_outer_iters1,
            num_outer_iters3,
            avg_micros3 / avg_micros1,
            avg_micros3 / avg_micros2,
            num_visit_items_after as f64 / num_visit_items_before as f64,
        ));

        println!(
            "Problem #{} ({}, {}):",
            idx + 1,
            num_component_heads,
            num_component_head_phis
        );
        println!(
            "There are {} visit items before rewriting",
            num_visit_items_before
        );
        println!(
            "There are {} visit items after rewriting",
            num_visit_items_after
        );
        println!(
            "Standard program analysis ran {} outer iters",
            num_outer_iters1
        );
        println!(
            "Optimistic e-class analysis ran {} outer iters",
            num_outer_iters3
        );
        println!(
            "Standard program analysis visits each item {} times on average, per outer iteration",
            avg_visits_per_item1
        );
        println!(
            "Standard e-class analysis visits each item {} times on average",
            avg_visits_per_item2
        );
        println!(
            "Optimistic e-class analysis visits each item {} times on average, per outer iteration",
            avg_visits_per_item3
        );
        println!(
            "Standard program analysis took {} micro-seconds per outer iteration and {} micro-seconds per visit item, per outer iteration",
            avg_micros_per_outer1, avg_micros_per_outer_per_item1
        );
        println!(
            "Standard e-class analysis took {} micro-seconds per outer iteration and {} micro-seconds per visit item, per outer iteration",
            avg_micros_per_outer2, avg_micros_per_outer_per_item2
        );
        println!(
            "Optimistic e-class analysis took {} micro-seconds per outer iteration and {} micro-seconds per visit item, per outer iteration",
            avg_micros_per_outer3, avg_micros_per_outer_per_item3
        );

        println!("");
    }

    avg_avg_visits_per_item1 /= NUM_PROGRAMS as f64;
    avg_avg_visits_per_item2 /= NUM_PROGRAMS as f64;
    avg_avg_visits_per_item3 /= NUM_PROGRAMS as f64;
    avg_avg_micros_per_item1 /= NUM_PROGRAMS as f64;
    avg_avg_micros_per_item2 /= NUM_PROGRAMS as f64;
    avg_avg_micros_per_item3 /= NUM_PROGRAMS as f64;
    println!("Important info for paper:");
    println!(
        "Max number of component heads (max number of loops): {}",
        format!("{}", max_component_heads).bold()
    );
    println!(
        "Max number of component head phis: {}",
        format!("{}", max_component_head_phis).bold()
    );
    println!(
        "Max number of e-nodes: {}",
        format!("{}", num_e_nodes_per_program.iter().max().unwrap()).bold()
    );
    println!(
        "Max number of outer iterations for standard program analysis:   {} (% where it's 2: {}%)",
        max_outer_iters1,
        num_outer_iters_is_two1 * 100 / NUM_PROGRAMS
    );
    println!(
        "Max number of outer iterations for optimistic e-class analysis: {}",
        format!(
            "{} (% where it's 2: {}%)",
            max_outer_iters3,
            num_outer_iters_is_two3 * 100 / NUM_PROGRAMS
        )
        .bold()
    );
    println!(
        "Average average visits per item per outer iteration for standard program analysis:   {} (max: {})",
        avg_avg_visits_per_item1, max_visits_per_item1
    );
    println!(
        "Average average visits per item per outer iteration for standard e-class analysis:   {} (max: {})",
        avg_avg_visits_per_item2, max_visits_per_item2
    );
    println!(
        "Average average visits per item per outer iteration for optimistic e-class analysis: {}",
        format!(
            "{} (max: {})",
            avg_avg_visits_per_item3, max_visits_per_item3
        )
        .bold()
    );
    println!(
        "Average average micros per visit for standard program analysis:   {} (max: {})",
        avg_avg_micros_per_item1, max_micros_per_item1
    );
    println!(
        "Average average micros per visit for standard e-class analysis:   {} (max: {})",
        avg_avg_micros_per_item2, max_micros_per_item2
    );
    println!(
        "Average average micros per visit for optimistic e-class analysis: {} (max: {})",
        avg_avg_micros_per_item3, max_micros_per_item3
    );

    let standard_abstract_interpretation =
        dist(table_distributions.iter().map(|t| t.0 as f64), false);
    let standard_e_class_analysis = dist(table_distributions.iter().map(|t| t.1 as f64), false);
    let optimistic_e_class_analysis = dist(table_distributions.iter().map(|t| t.2 as f64), false);
    let num_visit_items_before = dist(table_distributions.iter().map(|t| t.3 as f64), false);
    let num_visit_items_after = dist(table_distributions.iter().map(|t| t.4 as f64), false);
    let standard_abstract_interpretation_n =
        dist(table_distributions.iter().map(|t| t.5 as f64), false);
    let optimistic_e_class_analysis_n = dist(table_distributions.iter().map(|t| t.6 as f64), false);
    let optimistic_e_class_analysis_vs_standard_abstract_interpretation =
        dist(table_distributions.iter().map(|t| t.7 as f64), true);
    let optimistic_e_class_analysis_vs_standard_e_class_analysis =
        dist(table_distributions.iter().map(|t| t.8 as f64), true);
    let num_visit_items_after_vs_num_visit_items_before =
        dist(table_distributions.iter().map(|t| t.9 as f64), true);

    let rows = [
        Row {
            stat: "min",
            standard_abstract_interpretation: standard_abstract_interpretation.min.into(),
            standard_e_class_analysis: standard_e_class_analysis.min.into(),
            optimistic_e_class_analysis: AbsoluteAndRelative {
                absolute: optimistic_e_class_analysis.min,
                relative: vec![
                    optimistic_e_class_analysis_vs_standard_abstract_interpretation.min,
                    optimistic_e_class_analysis_vs_standard_e_class_analysis.min,
                ],
            },
            num_visit_items_before: num_visit_items_before.min.into(),
            num_visit_items_after: AbsoluteAndRelative {
                absolute: num_visit_items_after.min,
                relative: vec![num_visit_items_after_vs_num_visit_items_before.min],
            },
            standard_abstract_interpretation_n: standard_abstract_interpretation_n.min.into(),
            optimistic_e_class_analysis_n: optimistic_e_class_analysis_n.min.into(),
        },
        Row {
            stat: "median",
            standard_abstract_interpretation: standard_abstract_interpretation.median.into(),
            standard_e_class_analysis: standard_e_class_analysis.median.into(),
            optimistic_e_class_analysis: AbsoluteAndRelative {
                absolute: optimistic_e_class_analysis.median,
                relative: vec![
                    optimistic_e_class_analysis_vs_standard_abstract_interpretation.median,
                    optimistic_e_class_analysis_vs_standard_e_class_analysis.median,
                ],
            },
            num_visit_items_before: num_visit_items_before.median.into(),
            num_visit_items_after: AbsoluteAndRelative {
                absolute: num_visit_items_after.median,
                relative: vec![num_visit_items_after_vs_num_visit_items_before.median],
            },
            standard_abstract_interpretation_n: standard_abstract_interpretation_n.median.into(),
            optimistic_e_class_analysis_n: optimistic_e_class_analysis_n.median.into(),
        },
        Row {
            stat: "mean",
            standard_abstract_interpretation: standard_abstract_interpretation.mean.into(),
            standard_e_class_analysis: standard_e_class_analysis.mean.into(),
            optimistic_e_class_analysis: AbsoluteAndRelative {
                absolute: optimistic_e_class_analysis.mean,
                relative: vec![
                    optimistic_e_class_analysis_vs_standard_abstract_interpretation.mean,
                    optimistic_e_class_analysis_vs_standard_e_class_analysis.mean,
                ],
            },
            num_visit_items_before: num_visit_items_before.mean.into(),
            num_visit_items_after: AbsoluteAndRelative {
                absolute: num_visit_items_after.mean,
                relative: vec![num_visit_items_after_vs_num_visit_items_before.mean],
            },
            standard_abstract_interpretation_n: standard_abstract_interpretation_n.mean.into(),
            optimistic_e_class_analysis_n: optimistic_e_class_analysis_n.mean.into(),
        },
        Row {
            stat: "max",
            standard_abstract_interpretation: standard_abstract_interpretation.max.into(),
            standard_e_class_analysis: standard_e_class_analysis.max.into(),
            optimistic_e_class_analysis: AbsoluteAndRelative {
                absolute: optimistic_e_class_analysis.max,
                relative: vec![
                    optimistic_e_class_analysis_vs_standard_abstract_interpretation.max,
                    optimistic_e_class_analysis_vs_standard_e_class_analysis.max,
                ],
            },
            num_visit_items_before: num_visit_items_before.max.into(),
            num_visit_items_after: AbsoluteAndRelative {
                absolute: num_visit_items_after.max,
                relative: vec![num_visit_items_after_vs_num_visit_items_before.max],
            },
            standard_abstract_interpretation_n: standard_abstract_interpretation_n.max.into(),
            optimistic_e_class_analysis_n: optimistic_e_class_analysis_n.max.into(),
        },
    ];
    let mut table = Table::new(rows.clone());
    table
        .with(Style::rounded())
        .modify(Rows::first(), Alignment::center());
    println!("\n{}", format!("{}", table).bold());

    let mut file = File::create("plot_data.csv").unwrap();
    for (num_e_nodes, (micros, outer_iters)) in
        zip(num_e_nodes_per_program, zip(total_micros, num_outer_iters))
    {
        writeln!(file, "{num_e_nodes} {micros} {outer_iters}").unwrap();
    }

    let mut file = File::create("latex_table.tex").unwrap();
    writeln!(
        file,
        r#"
\begin{{table}}[t]
    \centering
    \footnotesize
    \begin{{tabular}}{{|c|c|c|c|c|}}
         \hline
        & Min & Median & Mean & Max \\
         \hline
        \makecell{{Standard Abstract \\ Interpretation (\textmu s)}} & {} & {} & {} & {} \\
         \hline
        \makecell{{Standard E-Class \\ Analysis (\textmu s)}} & {} & {} & {} & {} \\
         \hline
        \makecell{{Optimistic E-Class \\ Analysis (\textmu s)}} & {} & {} & {} & {} \\
         \hline
        \makecell{{\# Visit Items \\ Pre-rewriting}} & {} & {} & {} & {} \\
         \hline
        \makecell{{\# Visit Items \\ Post-rewriting}} & {} & {} & {} & {} \\
         \hline
        \makecell{{$n$ for Standard \\ Abstract Interpretation}} & {} & {} & {} & {} \\
         \hline
        \makecell{{$n$ for Optimistic \\ E-Class Analysis}} & {} & {} & {} & {} \\
         \hline
    \end{{tabular}}
    \caption{{Wall clock time (in \textmu s) for standard abstract interpretation, standard e-class analysis, and optimistic e-class analysis run on 100 generated programs, along with the amount of visit items and the number of greatest fixpoint computations ($n$). The relative execution time for optimistic e-class analysis is shown in parentheses compared to standard abstract interpretation and standard e-class analysis (smaller is better). The number of visit items after rewriting is shown relative to before rewriting. Standard abstract interpretation is run before rewriting while both e-class analyses are run after rewriting.}}
    \label{{tab:evaluation}}
\end{{table}}
"#,
        rows[0].standard_abstract_interpretation,
        rows[1].standard_abstract_interpretation,
        rows[2].standard_abstract_interpretation,
        rows[3].standard_abstract_interpretation,
        rows[0].standard_e_class_analysis,
        rows[1].standard_e_class_analysis,
        rows[2].standard_e_class_analysis,
        rows[3].standard_e_class_analysis,
        rows[0].optimistic_e_class_analysis,
        rows[1].optimistic_e_class_analysis,
        rows[2].optimistic_e_class_analysis,
        rows[3].optimistic_e_class_analysis,
        rows[0].num_visit_items_before,
        rows[1].num_visit_items_before,
        rows[2].num_visit_items_before,
        rows[3].num_visit_items_before,
        rows[0].num_visit_items_after,
        rows[1].num_visit_items_after,
        rows[2].num_visit_items_after,
        rows[3].num_visit_items_after,
        rows[0].standard_abstract_interpretation_n,
        rows[1].standard_abstract_interpretation_n,
        rows[2].standard_abstract_interpretation_n,
        rows[3].standard_abstract_interpretation_n,
        rows[0].optimistic_e_class_analysis_n,
        rows[1].optimistic_e_class_analysis_n,
        rows[2].optimistic_e_class_analysis_n,
        rows[3].optimistic_e_class_analysis_n,
    )
    .unwrap();
}

fn dist<I: Iterator<Item = f64> + Clone>(iter: I, geo: bool) -> Distribution {
    let num_items = iter.clone().count() as f64;
    let min = iter.clone().reduce(f64::min).unwrap();
    let max = iter.clone().reduce(f64::max).unwrap();
    let mean = if geo {
        (iter.clone().map(|factor| factor.ln()).sum::<f64>() / num_items).exp()
    } else {
        iter.clone().sum::<f64>() / num_items
    };
    let mut sorted = iter.collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    Distribution {
        min,
        median,
        mean,
        max,
    }
}
