use rand::distr::StandardUniform;
use rand::prelude::*;
use rustc_hash::FxHashSet;

use crate::ast::{BinaryOp, ExprAST, FuncAST, StmtAST, UnaryOp};

pub fn generate<R: Rng + ?Sized>(mut juice: usize, rng: &mut R) -> FuncAST {
    let mut in_scope = FxHashSet::default();
    let mut body = vec![];

    while juice > 0 {
        body.push(generate_stmt(&mut juice, rng, &mut in_scope));
    }
    juice += 10;
    body.push(StmtAST::Return(generate_expr(
        &mut juice,
        rng,
        &mut in_scope,
    )));

    FuncAST {
        name: "generated".to_string(),
        params: vec![],
        body: StmtAST::Block(body),
    }
}

fn generate_stmt<R: Rng + ?Sized>(
    juice: &mut usize,
    rng: &mut R,
    in_scope: &mut FxHashSet<String>,
) -> StmtAST {
    match rng.random::<u32>() % 100 {
        0..10 => {
            let mut max_num = rng.random::<u32>() % 20;
            let mut block = vec![];
            while max_num > 0 && *juice > 0 {
                max_num -= 1;
                block.push(generate_stmt(juice, rng, in_scope));
            }
            *juice = juice.saturating_sub(1);
            StmtAST::Block(block)
        }
        10..50 => {
            let new = rng.random::<bool>();
            if new || in_scope.is_empty() {
                let var = format!("v{}", in_scope.len());
                let expr = generate_expr(juice, rng, in_scope);
                in_scope.insert(var.clone());
                *juice = juice.saturating_sub(1);
                StmtAST::Assign(var, expr)
            } else {
                let var = in_scope.iter().choose(rng).unwrap().clone();
                let expr = generate_expr(juice, rng, in_scope);
                *juice = juice.saturating_sub(1);
                StmtAST::Assign(var, expr)
            }
        }
        50..70 => {
            let cond = generate_expr(juice, rng, in_scope);
            let mut then_scope = in_scope.clone();
            let then_stmt = if *juice > 0 {
                generate_stmt(juice, rng, &mut then_scope)
            } else {
                StmtAST::Block(vec![])
            };
            let mut else_scope = in_scope.clone();
            let else_stmt = if *juice > 0 {
                generate_stmt(juice, rng, &mut else_scope)
            } else {
                StmtAST::Block(vec![])
            };
            *in_scope = then_scope.intersection(&else_scope).cloned().collect();
            *juice = juice.saturating_sub(1);
            StmtAST::IfElse(cond, Box::new(then_stmt), Box::new(else_stmt))
        }
        70..98 => {
            let cond = generate_expr(juice, rng, in_scope);
            let mut inside_scope = in_scope.clone();
            let body = if *juice > 0 {
                generate_stmt(juice, rng, &mut inside_scope)
            } else {
                StmtAST::Block(vec![])
            };
            *juice = juice.saturating_sub(1);
            StmtAST::While(cond, Box::new(body))
        }
        _ => StmtAST::Return(generate_expr(juice, rng, in_scope)),
    }
}

fn generate_expr<R: Rng + ?Sized>(
    juice: &mut usize,
    rng: &mut R,
    in_scope: &FxHashSet<String>,
) -> ExprAST {
    match rng.random::<u32>() % 4 {
        0 => {
            let num = rng.random::<i64>() % 100;
            *juice = juice.saturating_sub(1);
            ExprAST::Number(num)
        }
        1 => {
            if !in_scope.is_empty() && *juice > 0 {
                let var = in_scope.iter().choose(rng).unwrap().clone();
                *juice = juice.saturating_sub(1);
                ExprAST::Variable(var)
            } else {
                ExprAST::Number(0)
            }
        }
        2 => {
            if *juice > 0 {
                let op = rng.random::<UnaryOp>();
                let input = generate_expr(juice, rng, in_scope);
                *juice = juice.saturating_sub(1);
                ExprAST::Unary(op, Box::new(input))
            } else {
                ExprAST::Number(0)
            }
        }
        _ => {
            if *juice > 0 {
                let op = rng.random::<BinaryOp>();
                let lhs = generate_expr(juice, rng, in_scope);
                if *juice > 0 {
                    let rhs = generate_expr(juice, rng, in_scope);
                    *juice = juice.saturating_sub(1);
                    ExprAST::Binary(op, Box::new(lhs), Box::new(rhs))
                } else {
                    lhs
                }
            } else {
                ExprAST::Number(0)
            }
        }
    }
}

impl Distribution<BinaryOp> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> BinaryOp {
        match rng.random::<u32>() % 9 {
            0 => BinaryOp::Add,
            1 => BinaryOp::Sub,
            2 => BinaryOp::Mul,
            3 => BinaryOp::EE,
            4 => BinaryOp::NE,
            5 => BinaryOp::LT,
            6 => BinaryOp::LE,
            7 => BinaryOp::GT,
            _ => BinaryOp::GE,
        }
    }
}

impl Distribution<UnaryOp> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> UnaryOp {
        match rng.random::<bool>() {
            true => UnaryOp::Neg,
            false => UnaryOp::Not,
        }
    }
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use rand::rngs::Xoshiro128PlusPlus;
    use rayon::prelude::*;

    use crate::analyses::{Analyses, standard_eclass_analysis};
    use crate::domains::Interval;
    use crate::rewrites::optimistic_equality_saturation;
    use crate::ssa::{BlockId, SSAGraph, dce, interpret, naive_ssa_translation};

    use super::*;

    fn check(ssa: &SSAGraph, analyses: &Analyses, block: BlockId, output: i64, iter: u64) {
        assert!(!analyses.unreachable_blocks[&block]);
        let root = ssa.roots[&block];
        let interval = analyses.intervals[&root];
        assert!(Interval::from_constant(output).leq(&interval), "Check failed at iter {}", iter);
    }

    #[test]
    fn generated() {
        (0..1000).into_par_iter().for_each(|i| {
            let mut rng = Xoshiro128PlusPlus::seed_from_u64(i);
            let program = generate(10, &mut rng);
            let (mut ssa, mut cfg) = naive_ssa_translation(&program);
            dce(&mut ssa, &cfg);
            let result = interpret(&ssa, &cfg, &[], 100);
            let analyses1 = optimistic_equality_saturation(&mut ssa, &mut cfg, 2, 2);
            let analyses2 = standard_eclass_analysis(&ssa, &cfg).0;
            if let Some((block, output)) = result {
                check(&ssa, &analyses1, block, output, i);
                check(&ssa, &analyses2, block, output, i);
            }
        });
    }

    #[test]
    #[ignore]
    fn torture_generated() {
        let cnt = AtomicUsize::new(0);
        (0..100000).into_par_iter().for_each(|i| {
            let cnt = cnt.fetch_add(1, Ordering::Relaxed) + 1;
            if cnt % 100 == 0 {
                eprintln!("{} / 100000", cnt);
            }
            let mut rng = Xoshiro128PlusPlus::seed_from_u64(i);
            let program = generate(5000, &mut rng);
            let (mut ssa, mut cfg) = naive_ssa_translation(&program);
            dce(&mut ssa, &cfg);
            let result = interpret(&ssa, &cfg, &[], 5000);
            let analyses1 = optimistic_equality_saturation(&mut ssa, &mut cfg, 2, 2);
            let analyses2 = standard_eclass_analysis(&ssa, &cfg).0;
            if let Some((block, output)) = result {
                check(&ssa, &analyses1, block, output, i);
                check(&ssa, &analyses2, block, output, i);
            }
        });
    }
}
