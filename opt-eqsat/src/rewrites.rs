use core::mem::take;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::analyses::{Analyses, outer_fixpoint};
use crate::ast::BinaryOp;
use crate::domains::{ClassId, Interval, UnionFind};
use crate::ssa::{CFG, SSAGraph, SSAValue};

type EClassMap = FxHashMap<ClassId, FxHashSet<SSAValue>>;

pub fn saturate(ssa: &mut SSAGraph, cfg: &mut CFG, analyses: &Analyses, max: usize) -> bool {
    let mut changed = false;
    let mut eclass_map: EClassMap = FxHashMap::default();
    for (value, id) in &ssa.values {
        eclass_map.entry(*id).or_default().insert(*value);
    }

    for _ in 0..max {
        let num_nodes = ssa.values.len();
        let num_classes = ssa.uf.num_classes();
        let new_nodes = rewrites(ssa, &eclass_map, analyses);

        for (value, id) in new_nodes {
            if let Some(old) = ssa.values.insert(value, id) {
                ssa.uf.union(old, id);
            }
            eclass_map.entry(id).or_default().insert(value);
        }

        if num_nodes == ssa.values.len() && num_classes == ssa.uf.num_classes() {
            break;
        }

        rebuild(ssa, cfg, &mut eclass_map);
        changed = true;
    }

    changed
}

fn rebuild(ssa: &mut SSAGraph, cfg: &mut CFG, eclass_map: &mut EClassMap) {
    loop {
        let mut canonicalized = vec![];

        for (value, id) in &mut ssa.values {
            let mut new = value.clone();
            new.map_uses(|id| *id = ssa.uf.find(*id));
            let new_id = ssa.uf.find(*id);
            if new != *value {
                canonicalized.push((*value, new, new_id));
            } else {
                *id = new_id;
            }
        }

        if canonicalized.is_empty() {
            break;
        } else {
            for (old, canon, canon_id) in canonicalized {
                ssa.values.remove(&old);
                if let Some(old) = ssa.values.insert(canon, canon_id) {
                    ssa.uf.union(old, canon_id);
                }
            }
        }
    }

    for (block, root) in take(&mut ssa.roots) {
        ssa.roots.insert(block, ssa.uf.find(root));
    }

    for (_, preds) in cfg {
        for (_, cond, _) in preds {
            *cond = ssa.uf.find(*cond);
        }
    }

    let old_eclass_map = take(eclass_map);
    for (id, values) in old_eclass_map {
        let entry = eclass_map.entry(ssa.uf.find(id)).or_default();
        entry.extend(values.into_iter().map(|mut value| {
            value.map_uses(|id| *id = ssa.uf.find(*id));
            value
        }));
    }
}

fn rewrites(
    ssa: &mut SSAGraph,
    eclass_map: &EClassMap,
    analyses: &Analyses,
) -> Vec<(SSAValue, ClassId)> {
    use BinaryOp::*;
    use SSAValue::*;
    let intervals = &analyses.intervals;
    let mut new_nodes = vec![];

    let mut add_value = |value, uf: &mut UnionFind| {
        if let Some(id) = ssa.values.get(&value) {
            *id
        } else {
            let id = uf.mk();
            new_nodes.push((value, id));
            id
        }
    };

    let is_constant = |id, cons| {
        eclass_map[id].contains(&Constant(cons))
            || intervals.get(id).unwrap_or(&Interval::top()).is_cons(cons)
    };

    for (node, id) in &ssa.values {
        match node {
            Binary(Add, lhs, rhs) if lhs == rhs => {
                let two = add_value(Constant(2), &mut ssa.uf);
                let two_times_lhs = add_value(Binary(Mul, two, *lhs), &mut ssa.uf);
                ssa.uf.union(*id, two_times_lhs);
            }
            Binary(Add, zero, rhs) if is_constant(zero, 0) => ssa.uf.union(*id, *rhs),
            Binary(Add, lhs, rhs) => {
                let recomm = add_value(Binary(Add, *rhs, *lhs), &mut ssa.uf);
                ssa.uf.union(*id, recomm);
                for value in &eclass_map[lhs] {
                    match value {
                        Binary(Add, sub_lhs, sub_rhs) => {
                            let sub_rhs_plus_rhs =
                                add_value(Binary(Add, *sub_rhs, *rhs), &mut ssa.uf);
                            let reassoc =
                                add_value(Binary(Add, *sub_lhs, sub_rhs_plus_rhs), &mut ssa.uf);
                            ssa.uf.union(*id, reassoc);
                        }
                        _ => {}
                    }
                }
                for value in &eclass_map[rhs] {
                    match value {
                        Binary(Add, sub_lhs, sub_rhs) => {
                            let lhs_plus_sub_lhs =
                                add_value(Binary(Add, *lhs, *sub_lhs), &mut ssa.uf);
                            let reassoc =
                                add_value(Binary(Add, lhs_plus_sub_lhs, *sub_rhs), &mut ssa.uf);
                            ssa.uf.union(*id, reassoc);
                        }
                        _ => {}
                    }
                }
            }
            Binary(Mul, two, rhs) if is_constant(two, 2) => {
                let rhs_plus_rhs = add_value(Binary(Add, *rhs, *rhs), &mut ssa.uf);
                ssa.uf.union(*id, rhs_plus_rhs);
            }
            Binary(Mul, one, rhs) if is_constant(one, 1) => ssa.uf.union(*id, *rhs),
            Binary(Mul, lhs, rhs) => {
                let recomm = add_value(Binary(Mul, *rhs, *lhs), &mut ssa.uf);
                ssa.uf.union(*id, recomm);
                for value in &eclass_map[lhs] {
                    match value {
                        Binary(Add, sub_lhs, sub_rhs) => {
                            let sub_lhs_times_rhs =
                                add_value(Binary(Mul, *sub_lhs, *rhs), &mut ssa.uf);
                            let sub_rhs_times_rhs =
                                add_value(Binary(Mul, *sub_rhs, *rhs), &mut ssa.uf);
                            let distribute = add_value(
                                Binary(Add, sub_lhs_times_rhs, sub_rhs_times_rhs),
                                &mut ssa.uf,
                            );
                            ssa.uf.union(*id, distribute);
                        }
                        _ => {}
                    }
                }
                for value in &eclass_map[rhs] {
                    match value {
                        Binary(Add, sub_lhs, sub_rhs) => {
                            let lhs_times_sub_lhs =
                                add_value(Binary(Mul, *lhs, *sub_lhs), &mut ssa.uf);
                            let lhs_times_sub_rhs =
                                add_value(Binary(Mul, *lhs, *sub_rhs), &mut ssa.uf);
                            let distribute = add_value(
                                Binary(Add, lhs_times_sub_lhs, lhs_times_sub_rhs),
                                &mut ssa.uf,
                            );
                            ssa.uf.union(*id, distribute);
                        }
                        _ => {}
                    }
                }
            }
            Binary(Sub, lhs, rhs) if lhs == rhs => {
                let zero = add_value(Constant(0), &mut ssa.uf);
                ssa.uf.union(*id, zero);
            }
            Binary(Sub, lhs, rhs) => {
                for rhs_value in &eclass_map[rhs] {
                    for lhs_value in &eclass_map[lhs] {
                        if let Binary(Add, sub_lhs, sub_rhs) = lhs_value
                            && eclass_map[sub_rhs].contains(rhs_value)
                        {
                            ssa.uf.union(*id, *sub_lhs);
                        }
                    }
                }
            }
            Binary(EE, lhs, rhs) if lhs == rhs => {
                let one = add_value(Constant(1), &mut ssa.uf);
                ssa.uf.union(*id, one);
            }
            Binary(NE, lhs, rhs) if lhs == rhs => {
                let zero = add_value(Constant(0), &mut ssa.uf);
                ssa.uf.union(*id, zero);
            }
            _ => {}
        }
    }

    let num_ids = analyses.value_number_uf.borrow().num_ids();
    for id in 0..num_ids {
        let root = analyses.value_number_uf.borrow_mut().find(id);
        ssa.uf.union(id, root);
    }

    new_nodes
}

pub fn optimistic_equality_saturation(
    ssa: &mut SSAGraph,
    cfg: &mut CFG,
    max_outer_iters: usize,
    max_rewrite_iters: usize,
) -> Analyses {
    let mut analyses = outer_fixpoint(ssa, cfg).0;
    for _ in 0..max_outer_iters {
        if !saturate(ssa, cfg, &analyses, max_rewrite_iters) {
            break;
        }
        analyses = outer_fixpoint(ssa, cfg).0;
    }
    analyses
}

#[cfg(test)]
mod tests {
    use crate::domains::Interval;
    use crate::grammar::ProgramParser;
    use crate::ssa::{dce, naive_ssa_translation};

    use super::*;

    fn test_roots_eq(program: &str) {
        let parsed = ProgramParser::new().parse(program).unwrap();
        assert_eq!(parsed.len(), 1);
        let (mut ssa, mut cfg) = naive_ssa_translation(&parsed[0]);
        dce(&mut ssa, &cfg);
        let analyses = outer_fixpoint(&ssa, &cfg).0;
        saturate(&mut ssa, &mut cfg, &analyses, 3);
        assert_eq!(ssa.roots.values().collect::<FxHashSet<_>>().len(), 1);
    }

    fn opt_eqsat(
        program: &str,
        max_outer_iters: usize,
        max_rewrite_iters: usize,
    ) -> (SSAGraph, Analyses, ClassId) {
        let parsed = ProgramParser::new().parse(program).unwrap();
        assert_eq!(parsed.len(), 1);
        let (mut ssa, mut cfg) = naive_ssa_translation(&parsed[0]);
        dce(&mut ssa, &cfg);
        let result =
            optimistic_equality_saturation(&mut ssa, &mut cfg, max_outer_iters, max_rewrite_iters);
        assert_eq!(ssa.roots.values().collect::<FxHashSet<_>>().len(), 1);
        let root = *ssa.roots.iter().next().unwrap().1;
        (ssa, result, root)
    }

    #[test]
    fn rewrite1() {
        let program = r#"
fn test(x) { if x { return x + x; } else { return 2 * x; } }
"#;
        test_roots_eq(program);
    }

    #[test]
    fn rewrite2() {
        let program = r#"
fn test(x) { if x { y = 2; } else { y = 1 + 1; } y = y - 1; z = x + 1; if x { return y * z; } else { return z; } }
"#;
        test_roots_eq(program);
    }

    #[test]
    #[should_panic]
    fn bad_rewrite1() {
        let program = r#"
fn test(x) { if x { return x + x + x; } else { return 2 * x; } }
"#;
        test_roots_eq(program);
    }

    #[test]
    fn opt_eqsat1() {
        let program = r#"
fn test() { x = 1; while 1 { x = x + (1 * 5); } return x; }
"#;
        let (ssa, result, root) = opt_eqsat(program, 2, 2);
        let one = ssa.values[&SSAValue::Constant(1)];
        let five = ssa.values[&SSAValue::Constant(5)];
        assert_eq!(
            ssa.values[&SSAValue::Binary(BinaryOp::Mul, one, five)],
            five
        );
        assert_eq!(result.intervals[&root], Interval::from_low(1),);
    }

    #[test]
    fn opt_eqsat2() {
        let program = r#"
fn test(a, b, c) { if a { return a + (b + c); } else { return (a + b) + c; } }
"#;
        opt_eqsat(program, 2, 2);
    }

    #[test]
    fn opt_eqsat3() {
        let program = r#"
fn test(a, b, c) { if a { return a * (b + c); } else { return a * b + a * c; } }
"#;
        opt_eqsat(program, 2, 2);
    }

    #[test]
    fn opt_eqsat4() {
        let program = r#"
fn test(a, b) { if a { return a; } else { return (a + b) - b; } }
"#;
        opt_eqsat(program, 2, 2);
    }

    #[test]
    fn opt_eqsat5() {
        let program = r#"
fn test(y) { x = -6; while y < 10 { y = y + 1; x = x + 8; x = x - 8; } return x + 2; }
"#;
        let (_, analyses, root) = opt_eqsat(program, 2, 2);
        assert!(analyses.intervals[&root].is_cons(-4));
    }

    #[test]
    fn opt_eqsat6() {
        let program = r#"
fn test(y) { x = -6; z = 42; while y < 10 { y = y + 1; x = x + 8; if x != 2 { z = 24; } x = x - 8; } return z + 7; }
"#;
        let (_, analyses, root) = opt_eqsat(program, 2, 2);
        assert!(analyses.intervals[&root].is_cons(49));
    }

    #[test]
    fn opt_eqsat7() {
        let program = r#"
fn test(y, z) { if y { return ((2 + y) + z) * y; } else { return 2 * y + (y * y + z * y); } }
"#;
        opt_eqsat(program, 2, 2);
    }

    #[test]
    fn opt_eqsat8() {
        let program = r#"
fn test(y) { z = 42; while 1 { if y != y { z = 24; } } return z + 3; }
"#;
        let (_, analyses, root) = opt_eqsat(program, 2, 2);
        assert!(analyses.intervals[&root].is_cons(45));
    }

    #[test]
    fn opt_eqsat9() {
        let program = r#"
fn test(x) { y = x; while 1 { x = x + 1; y = y + 1; } return x - y; }
"#;
        let (ssa, _, root) = opt_eqsat(program, 2, 2);
        let correct = ssa.values[&SSAValue::Constant(0)];
        assert_eq!(root, correct);
    }

    #[test]
    fn opt_eqsat10() {
        let program = r#"
fn test(x) { y = x; while 1 { xt = x; x = x + 1; y = xt + 1; } return x - y; }
"#;
        let (ssa, _, root) = opt_eqsat(program, 2, 2);
        let correct = ssa.values[&SSAValue::Constant(0)];
        assert_eq!(root, correct);
    }

    #[test]
    fn opt_eqsat11() {
        let program = r#"
fn test(x) { y = x; while 1 { xt = x; x = y + 1; y = xt + 1; } return x - y; }
"#;
        let (ssa, _, root) = opt_eqsat(program, 2, 2);
        let correct = ssa.values[&SSAValue::Constant(0)];
        assert_eq!(root, correct);
    }

    #[test]
    fn opt_eqsat12() {
        let program = r#"
fn test(x) { y = x; while 1 { x = x + x; y = 2 * y; } return x - y; }
"#;
        let (ssa, _, root) = opt_eqsat(program, 2, 2);
        let correct = ssa.values[&SSAValue::Constant(0)];
        assert_eq!(root, correct);
    }

    #[test]
    fn opt_eqsat13() {
        let program = r#"
fn test(y) {
    while y {
        y = y + 1;
        if ((2 + y) * y) != (2 * y + y * y) {}
    }
    return y;
}
"#;
        opt_eqsat(program, 2, 2);
    }

    #[test]
    fn opt_eqsat14() {
        let program = r#"
fn test() { v0 = 0; while v0 { if 1 { } else { } } return 0; }
"#;
        let (_, analyses, root) = opt_eqsat(program, 2, 2);
        assert_eq!(analyses.intervals[&root], Interval::from_constant(0));
    }

    #[test]
    fn opt_eqsat15() {
        let program = r#"
fn test() { v0 = 0; while v0 { if 23 { v0 = 44; } } return !-34; }
"#;
        let (_, analyses, root) = opt_eqsat(program, 2, 2);
        assert_eq!(analyses.intervals[&root], Interval::from_constant(0));
    }

    #[test]
    fn opt_eqsat16() {
        let program = r#"
fn test() { v0 = 0; while v0 { while (v0 - v0) { } } return v0 + 0; }
"#;
        let (_, analyses, root) = opt_eqsat(program, 2, 2);
        assert_eq!(analyses.intervals[&root], Interval::from_constant(0));
    }

    #[test]
    #[should_panic]
    fn bad_opt_eqsat1() {
        let program = r#"
fn test(a, b, c) { if a { return a + (b + c); } else { return (a + b) + a; } }
"#;
        opt_eqsat(program, 2, 2);
    }

    #[test]
    fn paper_example1() {
        let program = r#"
fn example1(y) {
    x = -6;
    z = 42;
    while y < 10 {
        y = y + 1;
        x = x + 8;
        if (((x + y) + z) * y) != (2 * y + (y * y + z * y)) {
            z = 24;
        }
        x = x - 8;
    }
    return z + 7;
}
"#;
        let (_, analyses, root) = opt_eqsat(program, 2, 3);
        assert!(analyses.intervals[&root].is_cons(49));
    }

    #[test]
    fn paper_example2() {
        let program = r#"
fn example2(x) {
    y = x;
    while y < 10 {
        xt = x;
        x = y * y + y * 5;
        y = xt * (y + 5 + 0);
    }

    return x - y;
}
"#;
        let (_, analyses, root) = opt_eqsat(program, 2, 2);
        assert!(analyses.intervals[&root].is_cons(0));
    }
}
