use core::cmp::Ordering;
use core::fmt::Debug;
use core::mem::swap;

use crate::ast::{BinaryOp, UnaryOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    MinusInfinity,
    Integer(i64),
    PlusInfinity,
}

impl Bound {
    fn add(&self, other: &Self) -> Option<Self> {
        use Bound::*;
        match (self, other) {
            (MinusInfinity, PlusInfinity) | (PlusInfinity, MinusInfinity) => None,
            (MinusInfinity, _) | (_, MinusInfinity) => Some(MinusInfinity),
            (PlusInfinity, _) | (_, PlusInfinity) => Some(PlusInfinity),
            (Integer(a), Integer(b)) => {
                if let Some(result) = a.checked_add(*b) {
                    Some(Integer(result))
                } else if *a > 0 {
                    Some(Integer(i64::MAX))
                } else {
                    Some(Integer(i64::MIN))
                }
            }
        }
    }

    fn sub(&self, other: &Self) -> Option<Self> {
        use Bound::*;
        match (self, other) {
            (MinusInfinity, MinusInfinity) | (PlusInfinity, PlusInfinity) => None,
            (MinusInfinity, _) | (_, PlusInfinity) => Some(MinusInfinity),
            (PlusInfinity, _) | (_, MinusInfinity) => Some(PlusInfinity),
            (Integer(a), Integer(b)) => {
                if let Some(result) = a.checked_sub(*b) {
                    Some(Integer(result))
                } else if *a > *b {
                    Some(Integer(i64::MAX))
                } else {
                    Some(Integer(i64::MIN))
                }
            }
        }
    }

    fn mul(&self, other: &Self) -> Self {
        use Bound::*;
        match (self, other) {
            (MinusInfinity, MinusInfinity) | (PlusInfinity, PlusInfinity) => PlusInfinity,
            (MinusInfinity, PlusInfinity) | (PlusInfinity, MinusInfinity) => MinusInfinity,
            (MinusInfinity, Integer(x)) | (Integer(x), MinusInfinity) => {
                if *x > 0 {
                    MinusInfinity
                } else if *x < 0 {
                    PlusInfinity
                } else {
                    Integer(0)
                }
            }
            (PlusInfinity, Integer(x)) | (Integer(x), PlusInfinity) => {
                if *x > 0 {
                    PlusInfinity
                } else if *x < 0 {
                    MinusInfinity
                } else {
                    Integer(0)
                }
            }
            (Integer(a), Integer(b)) => {
                if let Some(result) = a.checked_mul(*b) {
                    Integer(result)
                } else if (*a < 0) == (*b < 0) {
                    Integer(i64::MAX)
                } else {
                    Integer(i64::MIN)
                }
            }
        }
    }

    fn neg(&self) -> Self {
        use Bound::*;
        match self {
            MinusInfinity => PlusInfinity,
            Integer(x) => Integer(-x),
            PlusInfinity => MinusInfinity,
        }
    }

    fn is_integer(&self) -> bool {
        if let Bound::Integer(_) = self {
            true
        } else {
            false
        }
    }

    fn is_minus_infinity(&self) -> bool {
        if let Bound::MinusInfinity = self {
            true
        } else {
            false
        }
    }

    fn is_plus_infinity(&self) -> bool {
        if let Bound::PlusInfinity = self {
            true
        } else {
            false
        }
    }
}

impl PartialOrd for Bound {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        use Bound::*;
        use Ordering::*;
        match (self, other) {
            (MinusInfinity, MinusInfinity) => Some(Equal),
            (PlusInfinity, PlusInfinity) => Some(Equal),
            (MinusInfinity, _) => Some(Less),
            (_, MinusInfinity) => Some(Greater),
            (PlusInfinity, _) => Some(Greater),
            (_, PlusInfinity) => Some(Less),
            (Integer(a), Integer(b)) => Some(a.cmp(b)),
        }
    }
}

impl Ord for Bound {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    low: Bound,
    high: Bound,
}

impl Interval {
    pub fn top() -> Self {
        Self {
            low: Bound::MinusInfinity,
            high: Bound::PlusInfinity,
        }
    }

    pub fn bottom() -> Self {
        Self {
            low: Bound::PlusInfinity,
            high: Bound::MinusInfinity,
        }
    }

    pub fn from_constant(cons: i64) -> Self {
        Self {
            low: Bound::Integer(cons),
            high: Bound::Integer(cons),
        }
    }

    pub fn from_low_high(low: i64, high: i64) -> Self {
        Self {
            low: Bound::Integer(low),
            high: Bound::Integer(high),
        }
    }

    pub fn from_low(low: i64) -> Self {
        Self {
            low: Bound::Integer(low),
            high: Bound::PlusInfinity,
        }
    }

    pub fn from_high(high: i64) -> Self {
        Self {
            low: Bound::MinusInfinity,
            high: Bound::Integer(high),
        }
    }

    pub fn from_option_low_high(low: Option<i64>, high: Option<i64>) -> Self {
        match (low, high) {
            (None, None) => Self::top(),
            (None, Some(high)) => Self::from_high(high),
            (Some(low), None) => Self::from_low(low),
            (Some(low), Some(high)) => Self::from_low_high(low, high),
        }
    }

    pub fn join(&self, other: &Self) -> Self {
        let interval = Self {
            low: self.low.min(other.low),
            high: self.high.max(other.high),
        };
        if interval.is_bottom() {
            Interval::bottom()
        } else {
            interval
        }
    }

    pub fn meet(&self, other: &Self) -> Self {
        let interval = Self {
            low: self.low.max(other.low),
            high: self.high.min(other.high),
        };
        if interval.is_bottom() {
            Interval::bottom()
        } else {
            interval
        }
    }

    pub fn leq(&self, other: &Self) -> bool {
        (self.low >= other.low && self.high <= other.high) || self.high < self.low
    }

    pub fn widen(&self, new: &Self) -> Self {
        let interval = Self {
            low: if self.low > new.low {
                Bound::MinusInfinity
            } else {
                self.low
            },
            high: if self.high < new.high {
                Bound::PlusInfinity
            } else {
                self.high
            },
        };
        if interval.is_bottom() {
            Interval::bottom()
        } else {
            interval
        }
    }

    pub fn try_constant(&self) -> Option<i64> {
        if let Bound::Integer(low) = self.low
            && let Bound::Integer(high) = self.high
            && low == high
        {
            Some(low)
        } else {
            None
        }
    }

    pub fn is_zero(&self) -> bool {
        self.is_cons(0)
    }

    pub fn is_cons(&self, cons: i64) -> bool {
        self.leq(&Interval::from_constant(cons))
    }

    pub fn is_bottom(&self) -> bool {
        self.leq(&Interval::bottom())
    }

    pub fn is_top(&self) -> bool {
        *self == Interval::top()
    }

    pub fn forward_unary(&self, op: UnaryOp) -> Self {
        use Bound::*;
        use UnaryOp::*;
        if self.is_top() || self.is_bottom() {
            return *self;
        }
        let interval = match op {
            Neg => Interval {
                low: self.high.neg(),
                high: self.low.neg(),
            },
            Not if self.low == Integer(0) && self.high == Integer(0) => Interval::from_constant(1),
            Not if self.low > Integer(0) || self.high < Integer(0) => Interval::from_constant(0),
            Not => Interval::from_low_high(0, 1),
        };
        if interval.is_bottom() {
            Interval::bottom()
        } else {
            interval
        }
    }

    pub fn forward_binary(&self, other: &Interval, op: BinaryOp) -> Self {
        use BinaryOp::*;
        if self.is_bottom() || other.is_bottom() {
            return Interval::bottom();
        } else if self.is_top() || other.is_top() {
            return Interval::top();
        }
        let interval = match op {
            Add => Interval {
                low: self.low.add(&other.low).unwrap_or(Bound::MinusInfinity),
                high: self.high.add(&other.high).unwrap_or(Bound::PlusInfinity),
            },
            Sub => Interval {
                low: self.low.sub(&other.high).unwrap_or(Bound::MinusInfinity),
                high: self.high.sub(&other.low).unwrap_or(Bound::PlusInfinity),
            },
            Mul => {
                let low_low = self.low.mul(&other.low);
                let low_high = self.low.mul(&other.high);
                let high_low = self.high.mul(&other.low);
                let high_high = self.high.mul(&other.high);
                Interval {
                    low: low_low.min(low_high).min(high_low.min(high_high)),
                    high: low_low.max(low_high).max(high_low.max(high_high)),
                }
            }
            EE => {
                if let Some(self_cons) = self.try_constant()
                    && let Some(other_cons) = other.try_constant()
                    && self_cons == other_cons
                {
                    Interval::from_constant(1)
                } else if self.high < other.low || other.high < self.low {
                    Interval::from_constant(0)
                } else {
                    Interval::from_low_high(0, 1)
                }
            }
            NE => {
                if let Some(self_cons) = self.try_constant()
                    && let Some(other_cons) = other.try_constant()
                    && self_cons == other_cons
                {
                    Interval::from_constant(0)
                } else if self.high < other.low || other.high < self.low {
                    Interval::from_constant(1)
                } else {
                    Interval::from_low_high(0, 1)
                }
            }
            LT if self.high < other.low => Interval::from_constant(1),
            LT if self.low >= other.high
                && (!(self.low.is_minus_infinity() && other.high.is_minus_infinity())
                    && !(self.low.is_plus_infinity() && other.high.is_plus_infinity())) =>
            {
                Interval::from_constant(0)
            }
            LT => Interval::from_low_high(0, 1),
            LE if self.high <= other.low
                && self.high.is_integer()
                && (!(self.high.is_minus_infinity() && other.low.is_minus_infinity())
                    && !(self.high.is_plus_infinity() && other.low.is_plus_infinity())) =>
            {
                Interval::from_constant(1)
            }
            LE if self.low > other.high => Interval::from_constant(0),
            LE => Interval::from_low_high(0, 1),
            GT if self.high <= other.low
                && self.high.is_integer()
                && (!(self.high.is_minus_infinity() && other.low.is_minus_infinity())
                    && !(self.high.is_plus_infinity() && other.low.is_plus_infinity())) =>
            {
                Interval::from_constant(0)
            }
            GT if self.low > other.high => Interval::from_constant(1),
            GT => Interval::from_low_high(0, 1),
            GE if self.high < other.low => Interval::from_constant(0),
            GE if self.low >= other.high
                && other.high.is_integer()
                && (!(self.low.is_minus_infinity() && other.high.is_minus_infinity())
                    && !(self.low.is_plus_infinity() && other.high.is_plus_infinity())) =>
            {
                Interval::from_constant(1)
            }
            GE => Interval::from_low_high(0, 1),
        };
        if interval.is_bottom() {
            Interval::bottom()
        } else {
            interval
        }
    }
}

pub type ClassId = usize;

#[derive(Clone, Debug)]
pub struct UnionFind {
    parents: Vec<ClassId>,
    num_classes: usize,
}

impl UnionFind {
    pub fn new() -> Self {
        Self {
            parents: vec![],
            num_classes: 0,
        }
    }

    pub fn from_num_ids(num_ids: usize) -> Self {
        Self {
            parents: (0..num_ids).collect(),
            num_classes: num_ids,
        }
    }

    pub fn num_ids(&self) -> usize {
        self.parents.len()
    }

    pub fn num_classes(&self) -> usize {
        self.num_classes
    }

    pub fn mk(&mut self) -> ClassId {
        let id = self.parents.len();
        self.parents.push(id);
        self.num_classes += 1;
        id
    }

    pub fn find(&mut self, id: ClassId) -> ClassId {
        let parent = self.parents[id];
        if parent == id {
            parent
        } else {
            let root = self.find(parent);
            self.parents[id] = root;
            root
        }
    }

    pub fn query(&mut self, a: ClassId, b: ClassId) -> bool {
        self.find(a) == self.find(b)
    }

    pub fn union(&mut self, a: ClassId, b: ClassId) {
        let mut a_root = self.find(a);
        let mut b_root = self.find(b);

        if a_root == b_root {
            return;
        } else if a_root > b_root {
            swap(&mut a_root, &mut b_root);
        }

        self.parents[b_root] = a_root;
        self.num_classes -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand::prelude::*;
    use rand::rngs::Xoshiro128PlusPlus;

    fn sample_interval<R: Rng + ?Sized>(rng: &mut R) -> Interval {
        use Bound::*;
        let low_case = rng.random::<u32>() % 100;
        let mut low = if low_case < 5 {
            MinusInfinity
        } else if low_case < 10 {
            PlusInfinity
        } else {
            Integer(rng.random::<i64>())
        };
        let high_case = rng.random::<u32>() % 100;
        let mut high = if high_case < 5 {
            MinusInfinity
        } else if high_case < 10 {
            PlusInfinity
        } else {
            Integer(rng.random::<i64>())
        };
        if low.is_minus_infinity() && high.is_minus_infinity() {
            high = PlusInfinity;
        }
        if low.is_plus_infinity() && high.is_plus_infinity() {
            low = MinusInfinity;
        }
        Interval { low, high }
    }

    fn sample_in_interval<R: Rng + ?Sized>(interval: Interval, rng: &mut R) -> Option<i64> {
        use Bound::*;
        match (interval.low, interval.high) {
            (MinusInfinity, PlusInfinity) => Some(rng.random()),
            (Integer(low), PlusInfinity) => Some(rng.random_range(low..i64::MAX)),
            (MinusInfinity, Integer(high)) => Some(rng.random_range(i64::MIN..high)),
            (Integer(low), Integer(high)) if low < high => Some(rng.random_range(low..high)),
            _ => None,
        }
    }

    fn sample_larger_interval<R: Rng + ?Sized>(interval: Interval, rng: &mut R) -> Interval {
        use Bound::*;
        let low = match interval.low {
            MinusInfinity => MinusInfinity,
            Integer(low) => Integer(rng.random_range(i64::MIN..low)),
            PlusInfinity => Integer(rng.random()),
        };
        let high = match interval.high {
            MinusInfinity => Integer(rng.random()),
            Integer(high) => Integer(rng.random_range(high..i64::MAX)),
            PlusInfinity => PlusInfinity,
        };
        Interval { low, high }
    }

    #[test]
    fn interval_lattice_laws() {
        let mut rng = Xoshiro128PlusPlus::seed_from_u64(0);
        let intervals: Vec<_> = (0..1000).map(|_| sample_interval(&mut rng)).collect();

        for a in &intervals {
            assert!(a.leq(&Interval::top()));
            assert!(Interval::bottom().leq(a));

            if let Some(cons) = a.try_constant() {
                assert_eq!(Interval::from_constant(cons), *a);
            }
        }

        for a in &intervals {
            for b in &intervals {
                let join = a.join(b);
                let meet = a.meet(b);
                assert!(a.leq(&join));
                assert!(b.leq(&join));
                assert!(meet.leq(a));
                assert!(meet.leq(b));
                assert!(meet.leq(&join));
                let widen = a.widen(b);
                assert!(b.leq(&widen));
                let meet_join = join.meet(b);
                assert!(meet_join.leq(b));
                assert!(b.leq(&meet_join));
            }
        }

        assert_eq!(
            Interval::bottom().meet(&Interval::top()),
            Interval::bottom()
        );
        assert_eq!(Interval::bottom().join(&Interval::top()), Interval::top());
    }

    #[test]
    fn interval_sound_abstraction() {
        let mut rng = Xoshiro128PlusPlus::seed_from_u64(0);
        let concrete_abstract: Vec<(i64, Interval)> = (0..1000)
            .map(|_| {
                let interval = sample_interval(&mut rng);
                let cons = sample_in_interval(interval, &mut rng);
                cons.map(|cons| (cons, interval))
            })
            .flatten()
            .collect();

        for (conc1, abs1) in &concrete_abstract {
            assert!(Interval::from_constant(*conc1).leq(abs1));
            assert!(*conc1 == 0 || !abs1.is_zero());
            assert!(
                Interval::from_constant(conc1.saturating_neg())
                    .leq(&abs1.forward_unary(UnaryOp::Neg))
            );
            assert!(
                Interval::from_constant((*conc1 == 0) as i64)
                    .leq(&abs1.forward_unary(UnaryOp::Not))
            );
            for (conc2, abs2) in &concrete_abstract {
                assert!(
                    Interval::from_constant(conc1.saturating_add(*conc2))
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::Add))
                );
                assert!(
                    Interval::from_constant(conc1.saturating_sub(*conc2))
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::Sub))
                );
                assert!(
                    Interval::from_constant(conc1.saturating_mul(*conc2))
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::Mul))
                );
                assert!(
                    Interval::from_constant((conc1 == conc2) as i64)
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::EE))
                );
                assert!(
                    Interval::from_constant((conc1 != conc2) as i64)
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::NE))
                );
                assert!(
                    Interval::from_constant((conc1 < conc2) as i64)
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::LT))
                );
                assert!(
                    Interval::from_constant((conc1 <= conc2) as i64)
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::LE))
                );
                assert!(
                    Interval::from_constant((conc1 > conc2) as i64)
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::GT))
                );
                assert!(
                    Interval::from_constant((conc1 >= conc2) as i64)
                        .leq(&abs1.forward_binary(&abs2, BinaryOp::GE))
                );
            }
        }
    }

    #[test]
    fn interval_transformers_monotone() {
        let mut rng = Xoshiro128PlusPlus::seed_from_u64(0);
        let intervals: Vec<_> = (0..100)
            .map(|_| {
                let interval = sample_interval(&mut rng);
                let larger_interval = sample_larger_interval(interval, &mut rng);
                (interval, larger_interval)
            })
            .collect();

        let unary_ops = [UnaryOp::Neg, UnaryOp::Not];
        let binary_ops = [
            BinaryOp::Add,
            BinaryOp::Sub,
            BinaryOp::Mul,
            BinaryOp::EE,
            BinaryOp::NE,
            BinaryOp::LT,
            BinaryOp::LE,
            BinaryOp::GT,
            BinaryOp::GE,
        ];

        for (a, big_a) in &intervals {
            assert!(a.leq(big_a));
            for op in unary_ops {
                let op_a = a.forward_unary(op);
                let op_big_a = big_a.forward_unary(op);
                assert!(op_a.leq(&op_big_a));
            }
            for (b, big_b) in &intervals {
                assert!(b.leq(big_b));
                for op in binary_ops {
                    let op_a_b = a.forward_binary(b, op);
                    let op_a_big_b = a.forward_binary(big_b, op);
                    let op_big_a_b = big_a.forward_binary(b, op);
                    let op_big_a_big_b = big_a.forward_binary(big_b, op);
                    assert!(op_a_b.leq(&op_a_big_b));
                    assert!(op_a_b.leq(&op_big_a_b));
                    assert!(op_a_big_b.leq(&op_big_a_big_b));
                    assert!(op_big_a_b.leq(&op_big_a_big_b));
                    assert!(op_a_b.leq(&op_big_a_big_b));
                }
            }
        }
    }

    #[test]
    fn simple_uf() {
        let mut uf = UnionFind::new();
        let x = uf.mk();
        let y = uf.mk();
        let z = uf.mk();
        assert_ne!(x, y);
        assert_ne!(y, z);
        assert_ne!(z, x);
        assert_eq!(uf.find(x), x);
        assert_eq!(uf.find(y), y);
        assert_eq!(uf.find(z), z);
        uf.union(x, x);
        uf.union(y, y);
        uf.union(z, z);
        uf.union(x, y);
        assert_eq!(uf.find(x), uf.find(y));
        assert_ne!(uf.find(x), uf.find(z));
        uf.union(y, x);
        uf.union(x, z);
        assert_eq!(uf.find(x), uf.find(z));
        assert_eq!(uf.find(y), uf.find(z));
        assert_eq!(uf.find(y), uf.find(x));
        uf.union(x, y);
        uf.union(z, y);
    }

    #[test]
    fn complex_uf() {
        let mut uf = UnionFind::new();
        let mut ids = vec![];
        for _ in 0..1000 {
            ids.push(uf.mk());
        }
        for i in 0..999 {
            assert_ne!(uf.find(ids[i]), uf.find(ids[i + 1]));
        }
        for i in 0..500 {
            uf.union(ids[2 * i], ids[2 * i + 1]);
        }
        for i in 0..500 {
            assert_eq!(uf.find(ids[2 * i]), uf.find(ids[2 * i + 1]));
            if i < 499 {
                assert_ne!(uf.find(ids[2 * i]), uf.find(ids[2 * i + 2]));
            }
        }
        for i in 0..499 {
            uf.union(ids[2 * i], ids[2 * i + 2]);
        }
        for i in 0..999 {
            assert_eq!(uf.find(ids[i]), uf.find(ids[999]));
        }
    }
}
