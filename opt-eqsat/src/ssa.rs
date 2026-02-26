use core::cell::RefCell;
use core::mem::take;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ast::{BinaryOp, ExprAST, FuncAST, StmtAST, UnaryOp};
use crate::domains::{ClassId, UnionFind};

pub type BlockId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SSAValue {
    Constant(i64),
    Param(usize),
    Phi(BlockId, ClassId, ClassId),
    Unary(UnaryOp, ClassId),
    Binary(BinaryOp, ClassId, ClassId),
}

impl SSAValue {
    pub fn is_constant(&self) -> bool {
        if let SSAValue::Constant(_) = self {
            true
        } else {
            false
        }
    }

    pub fn is_param(&self) -> bool {
        if let SSAValue::Param(_) = self {
            true
        } else {
            false
        }
    }

    pub fn try_phi(&self) -> Option<(BlockId, ClassId, ClassId)> {
        if let SSAValue::Phi(block, lhs, rhs) = self {
            Some((*block, *lhs, *rhs))
        } else {
            None
        }
    }

    pub fn map_uses<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut ClassId),
    {
        match self {
            SSAValue::Constant(_) | SSAValue::Param(_) => {}
            SSAValue::Phi(_, lhs, rhs) | SSAValue::Binary(_, lhs, rhs) => {
                f(lhs);
                f(rhs);
            }
            SSAValue::Unary(_, input) => f(input),
        }
    }

    pub fn map_non_back_edge_uses<F>(&mut self, mut f: F, cfg: &CFG)
    where
        F: FnMut(&mut ClassId),
    {
        match self {
            SSAValue::Constant(_) | SSAValue::Param(_) => {}
            SSAValue::Phi(block, lhs, rhs) => {
                if cfg[&block][0].2 {
                    f(rhs);
                } else if cfg[&block][1].2 {
                    f(lhs);
                } else {
                    f(lhs);
                    f(rhs);
                }
            }
            SSAValue::Binary(_, lhs, rhs) => {
                f(lhs);
                f(rhs);
            }
            SSAValue::Unary(_, input) => f(input),
        }
    }
}

pub type CFG = FxHashMap<BlockId, Vec<(BlockId, ClassId, bool)>>;

#[derive(Debug)]
pub struct SSAGraph {
    pub values: FxHashMap<SSAValue, ClassId>,
    pub uf: UnionFind,
    pub roots: FxHashMap<BlockId, ClassId>,
}

impl SSAGraph {
    fn add_value(&mut self, value: SSAValue) -> ClassId {
        if let Some(id) = self.values.get(&value) {
            *id
        } else {
            let id = self.alloc_id();
            self.values.insert(value, id);
            id
        }
    }

    fn alloc_id(&mut self) -> ClassId {
        self.uf.mk()
    }

    fn set_value(&mut self, id: ClassId, value: SSAValue) -> ClassId {
        if let Some(old) = self.values.get(&value) {
            *old
        } else {
            self.values.insert(value, id);
            id
        }
    }
}

#[derive(Debug, Clone)]
struct NaiveSSAContext<'a, 'b> {
    vars: FxHashMap<&'a str, ClassId>,
    num_blocks: &'b RefCell<BlockId>,
    last_block: BlockId,
}

pub fn naive_ssa_translation(func: &FuncAST) -> (SSAGraph, CFG) {
    let mut ssa = SSAGraph {
        values: FxHashMap::default(),
        uf: UnionFind::new(),
        roots: FxHashMap::default(),
    };
    let mut cfg = CFG::default();
    cfg.insert(0, vec![]);
    let num_blocks = RefCell::new(1);
    let mut ctx = NaiveSSAContext {
        vars: FxHashMap::default(),
        num_blocks: &num_blocks,
        last_block: 0,
    };

    for (idx, name) in func.params.iter().enumerate() {
        ctx.vars.insert(name, ssa.add_value(SSAValue::Param(idx)));
    }
    ctx.handle_stmt(&mut ssa, &mut cfg, &func.body);

    loop {
        let mut changed = false;
        for (mut value, id) in take(&mut ssa.values) {
            value.map_uses(|u| {
                *u = ssa.uf.find(*u);
            });
            let id = ssa.uf.find(id);
            if let Some(old) = ssa.values.insert(value, id) {
                ssa.uf.union(old, id);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    for (_, preds) in cfg.iter_mut() {
        for (_, cond, _) in preds {
            *cond = ssa.uf.find(*cond);
        }
    }

    for (_, root) in ssa.roots.iter_mut() {
        *root = ssa.uf.find(*root);
    }

    (ssa, cfg)
}

impl<'a, 'b> NaiveSSAContext<'a, 'b> {
    fn new_block(&self) -> BlockId {
        let mut num = self.num_blocks.borrow_mut();
        let id = *num;
        *num += 1;
        id
    }

    fn handle_stmt(mut self, ssa: &mut SSAGraph, cfg: &mut CFG, stmt: &'a StmtAST) -> Option<Self> {
        use StmtAST::*;
        match stmt {
            Block(stmts) => {
                for stmt in stmts {
                    if let Some(ctx) = self.handle_stmt(ssa, cfg, stmt) {
                        self = ctx;
                    } else {
                        return None;
                    }
                }
                Some(self)
            }
            Assign(name, expr) => {
                let value = self.handle_expr(ssa, expr);
                self.vars.insert(name, value);
                Some(self)
            }
            IfElse(cond, then_stmt, else_stmt) => {
                let true_cond = self.handle_expr(ssa, cond);
                let false_cond = ssa.add_value(SSAValue::Unary(UnaryOp::Not, true_cond));
                let true_value = ssa.add_value(SSAValue::Constant(1));

                let true_block = self.new_block();
                let mut then_ctx = self.clone();
                cfg.insert(true_block, vec![(self.last_block, true_cond, false)]);
                then_ctx.last_block = true_block;
                let then_ctx = then_ctx.handle_stmt(ssa, cfg, then_stmt);

                let false_block = self.new_block();
                let mut else_ctx = self.clone();
                cfg.insert(false_block, vec![(self.last_block, false_cond, false)]);
                else_ctx.last_block = false_block;
                let else_ctx = else_ctx.handle_stmt(ssa, cfg, else_stmt);

                match (then_ctx, else_ctx) {
                    (Some(then_ctx), Some(else_ctx)) => {
                        let merge_block = self.new_block();
                        cfg.insert(
                            merge_block,
                            vec![
                                (then_ctx.last_block, true_value, false),
                                (else_ctx.last_block, true_value, false),
                            ],
                        );

                        for (name, then_value) in &then_ctx.vars {
                            if let Some(else_value) = else_ctx.vars.get(name) {
                                self.vars.insert(
                                    name,
                                    ssa.add_value(SSAValue::Phi(
                                        merge_block,
                                        *then_value,
                                        *else_value,
                                    )),
                                );
                            }
                        }
                        self.last_block = merge_block;
                        Some(self)
                    }
                    (ctx, None) | (None, ctx) => ctx,
                }
            }
            While(cond, stmt) => {
                let mut initial_vars = vec![];
                for (name, value) in self.vars.iter_mut() {
                    let initial = *value;
                    let phi = ssa.alloc_id();
                    *value = phi;
                    initial_vars.push((*name, initial, phi));
                }

                let true_cond = self.handle_expr(ssa, cond);
                let false_cond = ssa.add_value(SSAValue::Unary(UnaryOp::Not, true_cond));
                let true_value = ssa.add_value(SSAValue::Constant(1));

                let entry_block = self.last_block;
                let cond_block = self.new_block();
                cfg.insert(cond_block, vec![(entry_block, true_value, false)]);
                self.last_block = cond_block;
                let body_block = self.new_block();
                let mut body_ctx = self.clone();
                body_ctx.last_block = body_block;
                let body_ctx = body_ctx.handle_stmt(ssa, cfg, stmt);
                let exit_block = self.new_block();
                self.last_block = exit_block;
                cfg.insert(body_block, vec![(cond_block, true_cond, false)]);
                cfg.insert(exit_block, vec![(cond_block, false_cond, false)]);

                if let Some(body_ctx) = body_ctx {
                    for (name, initial, phi) in initial_vars {
                        let after_loop = body_ctx.vars[name];
                        let old_phi =
                            ssa.set_value(phi, SSAValue::Phi(cond_block, initial, after_loop));
                        ssa.uf.union(phi, old_phi);
                    }
                    cfg.get_mut(&cond_block)
                        .unwrap()
                        .push((body_ctx.last_block, true_value, true))
                } else {
                    for (name, initial, phi) in initial_vars {
                        self.vars.insert(name, initial);
                        ssa.uf.union(phi, initial);
                    }
                }

                Some(self)
            }
            Return(expr) => {
                let root = self.handle_expr(ssa, expr);
                assert!(ssa.roots.insert(self.last_block, root).is_none());
                None
            }
        }
    }

    fn handle_expr(&mut self, ssa: &mut SSAGraph, expr: &ExprAST) -> ClassId {
        use ExprAST::*;
        let value = match expr {
            Number(cons) => SSAValue::Constant(*cons),
            Variable(name) => return self.vars[&name as &str],
            Unary(op, input) => SSAValue::Unary(*op, self.handle_expr(ssa, input)),
            Binary(op, lhs, rhs) => {
                SSAValue::Binary(*op, self.handle_expr(ssa, lhs), self.handle_expr(ssa, rhs))
            }
        };
        ssa.add_value(value)
    }
}

pub fn dce(ssa: &mut SSAGraph, cfg: &CFG) {
    let backwards: FxHashMap<_, _> = ssa.values.iter().map(|(value, id)| (*id, *value)).collect();
    let mut alive = FxHashSet::default();
    let mut worklist = Vec::from_iter(ssa.roots.iter().map(|(_, root)| *root));
    worklist.extend(
        cfg.iter()
            .map(|(_, preds)| preds.iter().map(|(_, cond, _)| *cond))
            .flatten(),
    );

    while let Some(id) = worklist.pop() {
        if !alive.contains(&id) {
            alive.insert(id);
            backwards[&id].clone().map_uses(|id| worklist.push(*id));
        }
    }

    ssa.values.retain(|_, id| alive.contains(id));
}

pub fn interpret(
    ssa: &SSAGraph,
    cfg: &CFG,
    args: &[i64],
    mut juice: usize,
) -> Option<(BlockId, i64)> {
    let id_to_value: FxHashMap<_, _> = ssa.values.iter().map(|(id, value)| (*value, *id)).collect();
    let mut cfg_succs: FxHashMap<BlockId, Vec<(BlockId, ClassId, usize)>> = FxHashMap::default();
    let mut phis_at_block: FxHashMap<BlockId, Vec<(ClassId, ClassId, ClassId)>> =
        FxHashMap::default();
    for (block, preds) in cfg {
        cfg_succs.entry(*block).or_default();
        phis_at_block.entry(*block).or_default();
        for (idx, (pred, cond, _)) in preds.iter().enumerate() {
            cfg_succs
                .entry(*pred)
                .or_default()
                .push((*block, *cond, idx));
        }
    }
    for (value, id) in &ssa.values {
        if let SSAValue::Phi(block, lhs, rhs) = value {
            phis_at_block
                .entry(*block)
                .or_default()
                .push((*id, *lhs, *rhs));
        }
    }

    let mut phi_values = FxHashMap::default();
    let mut block = 0;
    'walk: loop {
        if let Some(value) = ssa.roots.get(&block) {
            return Some((block, eval(*value, &id_to_value, &phi_values, args)?));
        }

        if juice == 0 {
            return None;
        }
        juice -= 1;

        for (succ, cond, idx) in &cfg_succs[&block] {
            if eval(*cond, &id_to_value, &phi_values, args)? != 0 {
                block = *succ;
                for (phi, lhs, rhs) in &phis_at_block[&block] {
                    let new_value = if *idx == 0 {
                        eval(*lhs, &id_to_value, &phi_values, args)
                    } else {
                        eval(*rhs, &id_to_value, &phi_values, args)
                    }?;
                    phi_values.insert(*phi, new_value);
                }
                continue 'walk;
            }
        }

        panic!("No ready successor after block {}", block);
    }
}

fn eval(
    id: ClassId,
    id_to_value: &FxHashMap<ClassId, SSAValue>,
    phi_values: &FxHashMap<ClassId, i64>,
    args: &[i64],
) -> Option<i64> {
    let value = match id_to_value[&id] {
        SSAValue::Constant(cons) => cons,
        SSAValue::Param(idx) => args[idx],
        SSAValue::Phi(_, _, _) => phi_values[&id],
        SSAValue::Unary(op, input) => {
            let input = eval(input, id_to_value, phi_values, args)?;
            match op {
                UnaryOp::Neg => -input,
                UnaryOp::Not => (input == 0) as i64,
            }
        }
        SSAValue::Binary(op, lhs, rhs) => {
            let lhs = eval(lhs, id_to_value, phi_values, args)?;
            let rhs = eval(rhs, id_to_value, phi_values, args)?;
            match op {
                BinaryOp::Add => lhs.checked_add(rhs)?,
                BinaryOp::Sub => lhs.checked_sub(rhs)?,
                BinaryOp::Mul => lhs.checked_mul(rhs)?,
                BinaryOp::EE => (lhs == rhs) as i64,
                BinaryOp::NE => (lhs != rhs) as i64,
                BinaryOp::LT => (lhs < rhs) as i64,
                BinaryOp::LE => (lhs <= rhs) as i64,
                BinaryOp::GT => (lhs > rhs) as i64,
                BinaryOp::GE => (lhs >= rhs) as i64,
            }
        }
    };
    Some(value)
}

#[cfg(test)]
mod tests {
    use crate::grammar::ProgramParser;

    use super::*;

    fn interpret_helper(program: &str, args: &[i64]) -> i64 {
        let parsed = ProgramParser::new().parse(program).unwrap();
        assert_eq!(parsed.len(), 1);
        let (mut ssa, cfg) = naive_ssa_translation(&parsed[0]);
        dce(&mut ssa, &cfg);
        interpret(&ssa, &cfg, args, 1000).unwrap().1
    }

    #[test]
    fn interpret1() {
        let program = r#"
fn test(x) { return x + 1; }
"#;
        assert_eq!(interpret_helper(program, &[42]), 43);
    }

    #[test]
    fn interpret2() {
        let program = r#"
fn test(x, y) { while x < y { x = x + 1; } return x; }
"#;
        assert_eq!(interpret_helper(program, &[24, 42]), 42);
    }

    #[test]
    fn interpret3() {
        let program = r#"
fn test(x, y) { if x < y { return x + y; } else { y = 42; } return x - y; }
"#;
        assert_eq!(interpret_helper(program, &[73, 9]), 31);
    }

    #[test]
    fn interpret4() {
        let program = r#"
fn test(x) { while x { while x { return 24; } } return x; }
"#;
        assert_eq!(interpret_helper(program, &[42]), 24);
    }
}
