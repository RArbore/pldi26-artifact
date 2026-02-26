use core::cmp::max;
use core::iter::zip;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

use rand::prelude::*;
use rand::rngs::Xoshiro128PlusPlus;

use opt_eqsat::analyses::{outer_fixpoint, standard_eclass_analysis};
use opt_eqsat::domains::Interval;
use opt_eqsat::generate::generate;
use opt_eqsat::rewrites::optimistic_equality_saturation;
use opt_eqsat::ssa::{dce, interpret, naive_ssa_translation};

const NUM_PROGRAMS: usize = 100;
const SAMPLE_SIZE: u128 = 25;

fn main() {
    let mut programs = vec![];
    let mut rng = Xoshiro128PlusPlus::seed_from_u64(1);
    while programs.len() < NUM_PROGRAMS {
        let func = generate(programs.len() * 100 + 5000, &mut rng);
        let (mut ssa, cfg) = naive_ssa_translation(&func);
        dce(&mut ssa, &cfg);
        if ssa.values.len() > 10
            && ssa.values.len() < 4000
            && let Some((block, output)) = interpret(&ssa, &cfg, &[], 1000)
        {
            programs.push((ssa, cfg, block, output));
        }
    }

    let mut max_component_heads = 0;
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
    let mut num_items_after_rewriting = vec![];
    let mut total_micros = vec![];
    let mut num_outer_iters = vec![];
    for (idx, (mut ssa, mut cfg, block, output)) in programs.into_iter().enumerate() {
        let root = ssa.roots[&block];

        let (analyses, statistics1) = outer_fixpoint(&ssa, &cfg);
        assert!(!analyses.unreachable_blocks[&block]);
        assert!(Interval::from_constant(output).leq(&analyses.intervals[&root]));

        let time1 = Instant::now();
        for _ in 0..SAMPLE_SIZE {
            outer_fixpoint(&ssa, &cfg);
        }
        let time1 = time1.elapsed();

        optimistic_equality_saturation(&mut ssa, &mut cfg, 3, 1);
        let root = ssa.roots[&block];

        let (analyses, statistics2) = standard_eclass_analysis(&ssa, &cfg);
        assert!(!analyses.unreachable_blocks[&block]);
        assert!(Interval::from_constant(output).leq(&analyses.intervals[&root]));

        let time2 = Instant::now();
        for _ in 0..SAMPLE_SIZE {
            standard_eclass_analysis(&ssa, &cfg);
        }
        let time2 = time2.elapsed();

        let (analyses, statistics3) = outer_fixpoint(&ssa, &cfg);
        assert!(!analyses.unreachable_blocks[&block]);
        assert!(Interval::from_constant(output).leq(&analyses.intervals[&root]));

        let time3 = Instant::now();
        for _ in 0..SAMPLE_SIZE {
            outer_fixpoint(&ssa, &cfg);
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
        let avg_micros1 = time1.as_micros().div_ceil(SAMPLE_SIZE);
        let avg_micros2 = time2.as_micros().div_ceil(SAMPLE_SIZE);
        let avg_micros3 = time3.as_micros().div_ceil(SAMPLE_SIZE);
        let num_visit_items_before = num_nodes + num_blocks + num_edges;
        let num_visit_items_after = num_e_nodes + num_blocks + num_edges;
        let avg_visits_per_item1 = (avg_visits1 as f64) / (num_visit_items_before as f64);
        let avg_visits_per_item2 = (avg_visits2 as f64) / (num_visit_items_after as f64);
        let avg_visits_per_item3 = (avg_visits3 as f64) / (num_visit_items_after as f64);
        let avg_micros_per_outer1 = (avg_micros1 as f64) / (num_outer_iters1 as f64);
        let avg_micros_per_outer2 = (avg_micros2 as f64) / (num_outer_iters2 as f64);
        let avg_micros_per_outer3 = (avg_micros3 as f64) / (num_outer_iters3 as f64);
        let avg_micros_per_outer_per_item1 =
            avg_micros_per_outer1 / (num_visit_items_before as f64);
        let avg_micros_per_outer_per_item2 = avg_micros_per_outer2 / (num_visit_items_after as f64);
        let avg_micros_per_outer_per_item3 = avg_micros_per_outer3 / (num_visit_items_after as f64);

        max_component_heads = max(max_component_heads, num_component_heads);
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
        num_items_after_rewriting.push(num_visit_items_after);
        total_micros.push(avg_micros3);
        num_outer_iters.push(num_outer_iters3);

        println!("Problem #{}:", idx + 1);
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
    println!("Max number of component heads: {}", max_component_heads);
    println!(
        "Max number of outer iterations for standard program analysis:   {} (% where it's 2: {}%)",
        max_outer_iters1,
        num_outer_iters_is_two1 * 100 / NUM_PROGRAMS
    );
    println!(
        "Max number of outer iterations for optimistic e-class analysis: {} (% where it's 2: {}%)",
        max_outer_iters3,
        num_outer_iters_is_two3 * 100 / NUM_PROGRAMS
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
        "Average average visits per item per outer iteration for optimistic e-class analysis: {} (max: {})",
        avg_avg_visits_per_item3, max_visits_per_item3
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

    let mut file = File::create("plot_data.csv").unwrap();
    for (num_items, (micros, outer_iters)) in zip(
        num_items_after_rewriting,
        zip(total_micros, num_outer_iters),
    ) {
        writeln!(file, "{num_items} {micros} {outer_iters}").unwrap();
    }
}
