use core::cell::RefCell;
use std::collections::VecDeque;

use rustc_hash::FxHashMap;

use crate::ast::{BinaryOp, UnaryOp};
use crate::domains::{ClassId, Interval, UnionFind};
use crate::ssa::{BlockId, CFG, SSAGraph, SSAValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum VisitItem {
    Node(SSAValue, ClassId),
    Block(BlockId),
    Edge(BlockId, BlockId),
}

fn dependents(ssa: &SSAGraph, cfg: &CFG) -> FxHashMap<VisitItem, Vec<VisitItem>> {
    use VisitItem::*;

    let mut eclass_map: FxHashMap<ClassId, Vec<SSAValue>> = FxHashMap::default();
    for (value, id) in &ssa.values {
        eclass_map.entry(*id).or_default().push(*value);
    }
    let mut deps: FxHashMap<VisitItem, Vec<VisitItem>> = FxHashMap::default();

    for (value, id) in &ssa.values {
        let node = Node(*value, *id);
        deps.entry(node).or_default();
        value.clone().map_non_back_edge_uses(
            |child_id| {
                for child_value in &eclass_map[child_id] {
                    let child_node = Node(*child_value, *child_id);
                    deps.entry(child_node).or_default().push(node);
                }
            },
            cfg,
        );
        match value {
            SSAValue::Constant(_) | SSAValue::Param(_) => {
                deps.entry(Block(0)).or_default().push(node)
            }
            SSAValue::Phi(block, _, _) => {
                for (pred, _, back_edge) in &cfg[block] {
                    if !back_edge {
                        let edge = Edge(*block, *pred);
                        deps.entry(edge).or_default().push(node);
                    }
                }
            }
            _ => {}
        }
    }

    for (block, preds) in cfg {
        deps.entry(Block(*block)).or_default();
        for (pred, cond, back_edge) in preds {
            deps.entry(Block(*pred))
                .or_default()
                .push(Edge(*block, *pred));
            for value in &eclass_map[cond] {
                deps.entry(Node(*value, *cond))
                    .or_default()
                    .push(Edge(*block, *pred));
            }
            let edge_deps = deps.entry(Edge(*block, *pred)).or_default();
            if !back_edge {
                edge_deps.push(Block(*block));
            }
        }
    }

    deps
}

fn widening_points(ssa: &SSAGraph, cfg: &CFG) -> Vec<(ClassId, BlockId, BlockId)> {
    let mut widening_points = vec![];
    for (value, id) in &ssa.values {
        if let Some((block, _, _)) = value.try_phi()
            && let Some((pred, _, _)) = cfg[&block].iter().find(|(_, _, back_edge)| *back_edge)
        {
            widening_points.push((*id, block, *pred));
        }
    }
    widening_points
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GVNValue {
    Constant(i64),
    Param(usize),
    ForwardPhi(BlockId, ClassId, ClassId),
    LeftBackwardPhi(BlockId, ClassId, ClassId),
    RightBackwardPhi(BlockId, ClassId, ClassId),
    Unary(UnaryOp, ClassId),
    Binary(BinaryOp, ClassId, ClassId),
}

#[derive(Debug, Clone)]
pub struct AnalysisStatistics {
    pub num_nodes: usize,
    pub num_blocks: usize,
    pub num_edges: usize,
    pub num_component_heads: usize,
    pub num_loop_visits: Vec<usize>,
}

impl AnalysisStatistics {
    pub fn new(
        num_nodes: usize,
        num_blocks: usize,
        num_edges: usize,
        num_component_heads: usize,
    ) -> Self {
        Self {
            num_nodes,
            num_blocks,
            num_edges,
            num_component_heads,
            num_loop_visits: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Analyses {
    pub unreachable_blocks: FxHashMap<BlockId, bool>,
    pub unreachable_edges: FxHashMap<(BlockId, BlockId), bool>,
    pub intervals: FxHashMap<ClassId, Interval>,
    pub value_number_uf: RefCell<UnionFind>,

    values: FxHashMap<GVNValue, ClassId>,
}

impl Analyses {
    pub fn top(num_class_ids: usize) -> Self {
        Self {
            unreachable_blocks: FxHashMap::default(),
            unreachable_edges: FxHashMap::default(),
            intervals: FxHashMap::default(),
            value_number_uf: RefCell::new(UnionFind::from_num_ids(num_class_ids)),

            values: FxHashMap::default(),
        }
    }

    fn is_block_unreachable(&self, block: BlockId) -> bool {
        self.unreachable_blocks
            .get(&block)
            .copied()
            .unwrap_or(false)
    }

    fn is_edge_unreachable(
        &self,
        block: BlockId,
        pred: BlockId,
        back_edge: bool,
        old: Option<&Self>,
    ) -> bool {
        if back_edge {
            old.map(|old| old.unreachable_edges[&(block, pred)])
                .unwrap_or(true)
        } else {
            self.unreachable_edges
                .get(&(block, pred))
                .copied()
                .unwrap_or(false)
        }
    }

    fn get_interval(&self, id: ClassId, back_edge: bool, old: Option<&Self>) -> Interval {
        if back_edge {
            old.map(|old| old.intervals[&id])
                .unwrap_or(Interval::bottom())
        } else {
            self.intervals.get(&id).copied().unwrap_or(Interval::top())
        }
    }

    fn get_value_number(&self, id: ClassId, back_edge: bool, old: Option<&Self>) -> ClassId {
        if back_edge {
            old.map(|old| old.value_number_uf.borrow_mut().find(id))
                .unwrap_or(0)
        } else {
            self.value_number_uf.borrow_mut().find(id)
        }
    }

    fn block_unreachable_transfer(&self, block: BlockId, cfg: &CFG, old: Option<&Self>) -> bool {
        if block == 0 {
            return false;
        }
        let mut all_unreachable = true;
        for (pred, _, back_edge) in &cfg[&block] {
            if !self.is_edge_unreachable(block, *pred, *back_edge, old) {
                all_unreachable = false;
                break;
            }
        }
        all_unreachable
    }

    fn edge_unreachable_transfer(
        &self,
        block: BlockId,
        pred: BlockId,
        cfg: &CFG,
        old: Option<&Self>,
    ) -> bool {
        if self.is_block_unreachable(pred) {
            return true;
        }
        let (_, cond, back_edge) = cfg[&block]
            .iter()
            .find(|(other, _, _)| *other == pred)
            .unwrap();
        let cond = self.get_interval(*cond, *back_edge, old);

        cond.is_zero()
    }

    fn interval_transfer(
        &self,
        value: SSAValue,
        id: ClassId,
        cfg: &CFG,
        old: Option<&Self>,
    ) -> Interval {
        use SSAValue::*;
        match value {
            Constant(cons) => Interval::from_constant(cons),
            Param(_) => Interval::top(),
            Phi(block, lhs, rhs) => {
                let preds = &cfg[&block];
                assert_eq!(preds.len(), 2);
                assert!(!preds[0].2 || !preds[1].2);
                let lhs = self.get_interval(lhs, preds[0].2, old);
                let rhs = self.get_interval(rhs, preds[1].2, old);
                let lhs_unreachable = self.is_edge_unreachable(block, preds[0].0, preds[0].2, old);
                let rhs_unreachable = self.is_edge_unreachable(block, preds[1].0, preds[1].2, old);
                let interval = match (lhs_unreachable, rhs_unreachable) {
                    (true, true) => Interval::top(),
                    (false, true) => lhs,
                    (true, false) => rhs,
                    (false, false) => lhs.join(&rhs),
                };
                if (preds[0].2 || preds[1].2)
                    && let Some(old) = old
                {
                    let old = old.intervals[&id];
                    old.widen(&interval)
                } else {
                    interval
                }
            }
            Unary(op, input) => self.get_interval(input, false, old).forward_unary(op),
            Binary(op, lhs, rhs) => {
                let lhs = self.get_interval(lhs, false, old);
                let rhs = self.get_interval(rhs, false, old);
                lhs.forward_binary(&rhs, op)
            }
        }
    }

    fn gvn_transfer(
        &mut self,
        value: SSAValue,
        mut id: ClassId,
        cfg: &CFG,
        old: Option<&Self>,
    ) -> ClassId {
        let value = match value {
            SSAValue::Constant(cons) => GVNValue::Constant(cons),
            SSAValue::Param(param) => GVNValue::Param(param),
            SSAValue::Phi(block, lhs, rhs) => {
                let preds = &cfg[&block];
                let lhs = self.get_value_number(lhs, preds[0].2, old);
                let rhs = self.get_value_number(rhs, preds[1].2, old);
                if preds[0].2 {
                    GVNValue::LeftBackwardPhi(block, lhs, rhs)
                } else if preds[1].2 {
                    GVNValue::RightBackwardPhi(block, lhs, rhs)
                } else {
                    GVNValue::ForwardPhi(block, lhs, rhs)
                }
            }
            SSAValue::Unary(op, input) => {
                let input = self.get_value_number(input, false, old);
                GVNValue::Unary(op, input)
            }
            SSAValue::Binary(op, lhs, rhs) => {
                let lhs = self.get_value_number(lhs, false, old);
                let rhs = self.get_value_number(rhs, false, old);
                GVNValue::Binary(op, lhs, rhs)
            }
        };

        if let Some(old_id) = self.values.get(&value) {
            self.value_number_uf.borrow_mut().union(id, *old_id);
            id = self.value_number_uf.borrow_mut().find(id);
        }
        self.values.insert(value, id);
        id
    }

    fn inner_fixpoint(
        &mut self,
        cfg: &CFG,
        dependents: &FxHashMap<VisitItem, Vec<VisitItem>>,
        old: Option<&Self>,
        statistics: &mut AnalysisStatistics,
    ) {
        use VisitItem::*;
        let mut inner_iter = 0;
        let mut worklist = VecDeque::new();
        worklist.push_back(Block(0));
        while let Some(visit) = worklist.pop_front() {
            inner_iter += 1;
            match visit {
                Node(value, id) => {
                    let old_interval = self.intervals.get(&id).copied();
                    let node_interval = self.interval_transfer(value, id, cfg, old);
                    let interval = node_interval.meet(&old_interval.unwrap_or(Interval::top()));
                    self.intervals.insert(id, interval);

                    let old_number = self.value_number_uf.borrow_mut().find(id);
                    let number = self.gvn_transfer(value, id, cfg, old);

                    if old_interval != Some(interval) || old_number != number {
                        worklist.extend(&dependents[&visit]);
                    }
                }
                Block(block) => {
                    let unreachable = self.block_unreachable_transfer(block, cfg, old);
                    let old_unreachable = self.unreachable_blocks.insert(block, unreachable);

                    if old_unreachable != Some(unreachable) {
                        worklist.extend(&dependents[&visit]);
                    }
                }
                Edge(block, pred) => {
                    let unreachable = self.edge_unreachable_transfer(block, pred, cfg, old);
                    let old_unreachable = self.unreachable_edges.insert((block, pred), unreachable);

                    if old_unreachable != Some(unreachable) {
                        worklist.extend(&dependents[&visit]);
                    }
                }
            }
        }

        statistics.num_loop_visits.push(inner_iter);
    }

    fn changed(&self, other: &Self, widening_points: &Vec<(ClassId, BlockId, BlockId)>) -> bool {
        widening_points.iter().any(|(phi, block, pred)| {
            self.intervals[phi] != other.intervals[phi]
                || self.unreachable_edges[&(*block, *pred)]
                    != other.unreachable_edges[&(*block, *pred)]
        })
    }
}

pub fn outer_fixpoint(ssa: &SSAGraph, cfg: &CFG) -> (Analyses, AnalysisStatistics) {
    let num_edges = cfg.iter().map(|(_, preds)| preds.len()).sum();
    let num_component_heads = cfg
        .iter()
        .filter(|(_, preds)| preds.iter().any(|(_, _, back_edge)| *back_edge))
        .count();
    let dependents = dependents(ssa, cfg);
    let widening_points = widening_points(ssa, cfg);
    let mut statistics =
        AnalysisStatistics::new(ssa.values.len(), cfg.len(), num_edges, num_component_heads);

    let mut analyses = Analyses::top(ssa.uf.num_ids());
    analyses.inner_fixpoint(cfg, &dependents, None, &mut statistics);

    loop {
        let mut new_analyses = Analyses::top(ssa.uf.num_ids());
        new_analyses.inner_fixpoint(cfg, &dependents, Some(&analyses), &mut statistics);
        if analyses.changed(&new_analyses, &widening_points) {
            analyses = new_analyses;
        } else {
            break;
        }
    }
    return (analyses, statistics);
}

pub fn standard_eclass_analysis(ssa: &SSAGraph, cfg: &CFG) -> (Analyses, AnalysisStatistics) {
    let num_edges = cfg.iter().map(|(_, preds)| preds.len()).sum();
    let num_component_heads = cfg
        .iter()
        .filter(|(_, preds)| preds.iter().any(|(_, _, back_edge)| *back_edge))
        .count();
    let dependents = dependents(ssa, cfg);
    let mut statistics =
        AnalysisStatistics::new(ssa.values.len(), cfg.len(), num_edges, num_component_heads);

    let mut top = Analyses::top(ssa.uf.num_ids());
    let mut analyses = top.clone();
    for (_, id) in &ssa.values {
        top.intervals.insert(*id, Interval::top());
    }
    for (block, preds) in cfg {
        top.unreachable_blocks.insert(*block, false);
        for (pred, _, _) in preds {
            top.unreachable_edges.insert((*block, *pred), false);
        }
    }

    analyses.inner_fixpoint(cfg, &dependents, Some(&top), &mut statistics);
    (analyses, statistics)
}

#[cfg(test)]
mod tests {
    use crate::grammar::ProgramParser;
    use crate::ssa::{dce, naive_ssa_translation};

    use super::*;

    fn acyclic_test_interval(program: &str, low: Option<i64>, high: Option<i64>) {
        let parsed = ProgramParser::new().parse(program).unwrap();
        assert_eq!(parsed.len(), 1);
        let (mut ssa, cfg) = naive_ssa_translation(&parsed[0]);
        dce(&mut ssa, &cfg);
        assert_eq!(ssa.roots.len(), 1);
        let root = *ssa.roots.iter().next().unwrap().1;
        let dependents = dependents(&ssa, &cfg);
        let mut analysis = Analyses::top(ssa.uf.num_ids());
        let mut statistics = AnalysisStatistics::new(0, 0, 0, 0);
        analysis.inner_fixpoint(&cfg, &dependents, None, &mut statistics);
        assert_eq!(
            analysis.intervals[&root],
            Interval::from_option_low_high(low, high)
        );
    }

    fn cyclic_test_interval(program: &str, low: Option<i64>, high: Option<i64>) {
        let parsed = ProgramParser::new().parse(program).unwrap();
        assert_eq!(parsed.len(), 1);
        let (mut ssa, cfg) = naive_ssa_translation(&parsed[0]);
        dce(&mut ssa, &cfg);
        assert_eq!(ssa.roots.len(), 1);
        let root = *ssa.roots.iter().next().unwrap().1;
        let analysis = outer_fixpoint(&ssa, &cfg).0;
        assert_eq!(
            analysis.intervals[&root],
            Interval::from_option_low_high(low, high)
        );
    }

    fn cyclic_test_gvn(program: &str) {
        let parsed = ProgramParser::new().parse(program).unwrap();
        assert_eq!(parsed.len(), 1);
        let (mut ssa, cfg) = naive_ssa_translation(&parsed[0]);
        dce(&mut ssa, &cfg);
        assert_eq!(ssa.roots.len(), 2);
        let mut roots = ssa.roots.iter();
        let root1 = *roots.next().unwrap().1;
        let root2 = *roots.next().unwrap().1;
        let analysis = outer_fixpoint(&ssa, &cfg).0;
        assert!(analysis.value_number_uf.borrow_mut().query(root1, root2));
    }

    fn cyclic_test_interval_multiroot(program: &str, low: Option<i64>, high: Option<i64>) {
        let parsed = ProgramParser::new().parse(program).unwrap();
        assert_eq!(parsed.len(), 1);
        let (mut ssa, cfg) = naive_ssa_translation(&parsed[0]);
        dce(&mut ssa, &cfg);
        let analysis = outer_fixpoint(&ssa, &cfg).0;
        for (block, root) in &ssa.roots {
            if !analysis.unreachable_blocks[block] {
                assert_eq!(
                    analysis.intervals[root],
                    Interval::from_option_low_high(low, high)
                );
            }
        }
    }

    #[test]
    fn acyclic_analysis1() {
        let program = r#"
fn test() { if 1 { x = 5; } else { x = 9; } return x; }
"#;
        acyclic_test_interval(program, Some(5), Some(5));
    }

    #[test]
    fn acyclic_analysis2() {
        let program = r#"
fn test() { x = 5; if x < 3 { y = 7; } else { y = 9; } return y; }
"#;
        acyclic_test_interval(program, Some(9), Some(9));
    }

    #[test]
    fn acyclic_analysis3() {
        let program = r#"
fn test() { x = 5; y = x + 7; if y > x { y = 3; } return y; }
"#;
        acyclic_test_interval(program, Some(3), Some(3));
    }

    #[test]
    fn acyclic_analysis4() {
        let program = r#"
fn test(x) { if x { y = 3; } else { y = 5; } return y; }
"#;
        acyclic_test_interval(program, Some(3), Some(5));
    }

    #[test]
    fn acyclic_analysis5() {
        let program = r#"
fn test(x) { y = 7; while x - y { y = y + 1; } return y; }
"#;
        acyclic_test_interval(program, Some(7), Some(7));
    }

    #[test]
    fn cyclic_analysis1() {
        let program = r#"
fn test() { x = -10; while x { x = x + 1; } return x; }
"#;
        cyclic_test_interval(program, Some(-10), None);
    }

    #[test]
    fn cyclic_analysis2() {
        let program = r#"
fn test(y) { x = -10; while y { y = y + 1; if y == 7 { x = 10; } } return x; }
"#;
        cyclic_test_interval(program, Some(-10), None);
    }

    #[test]
    fn cyclic_analysis3() {
        let program = r#"
fn test(y) { x = 10; while y { y = y + 1; if y == 7 { x = -10; } } return x; }
"#;
        cyclic_test_interval(program, None, Some(10));
    }

    #[test]
    fn cyclic_analysis4() {
        let program = r#"
fn test(x) { a = x; b = x; while x {} if x { return a; } else { return b; } }
"#;
        cyclic_test_gvn(program);
    }

    #[test]
    fn cyclic_analysis5() {
        let program = r#"
fn test(x) { a = x; b = x; while x { at = a; a = b + 1; b = at + 1; } if x { return a; } else { return b; } }
"#;
        cyclic_test_gvn(program);
    }

    #[test]
    fn cyclic_analysis6() {
        let program = r#"
fn test(x) { a = x; b = x; while x { a = a + 3; b = b + 3; a = a + 2; b = b + 2; } if x { return a; } else { return b; } }
"#;
        cyclic_test_gvn(program);
    }

    #[test]
    fn cyclic_analysis7() {
        let program = r#"
fn test(c) { x = 0; while c { while c { x = x + 1; } } return x; }
"#;
        cyclic_test_interval(program, Some(0), None);
    }

    #[test]
    fn cyclic_analysis8() {
        let program = r#"
fn test(c) { x = 0; while c { x = x + 1; } while c { x = x + 1; } return x; }
"#;
        cyclic_test_interval(program, Some(0), None);
    }

    #[test]
    fn cyclic_analysis9() {
        let program = r#"
fn test(c) { x = 0; while c { while c { while c { while c { while c { while c { x = x + 1; } } } } } } return x; }
"#;
        cyclic_test_interval(program, Some(0), None);
    }

    #[test]
    fn cyclic_analysis10() {
        let program = r#"
fn test(c) { x = 0; while c { while c { while c { while c { while c { while c { x = x + 1; } } } } } } while c { while c { while c { while c { while c { while c { x = x + 1; } } } } } } return x; }
"#;
        cyclic_test_interval(program, Some(0), None);
    }

    #[test]
    fn cyclic_analysis11() {
        let program = r#"
fn test(c) { x = 0; while c { } return x; }
"#;
        cyclic_test_interval(program, Some(0), Some(0));
    }

    #[test]
    fn cyclic_analysis12() {
        let program = r#"
fn test(c) { x = 0; while c { if x { return c; } else { return x; } return x; } return x; return x; return c; }
"#;
        cyclic_test_interval_multiroot(program, Some(0), Some(0));
    }
}
