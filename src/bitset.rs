pub use fixedbitset::FixedBitSet;
use std::hash::{Hash, Hasher};

pub trait Set<'a> {
    type Iter;

    fn create(size: usize) -> Self;
    fn is_empty(&self) -> bool;
    fn remove(&mut self, index: usize);
    fn insert(&mut self, index: usize);
    fn clear(&mut self);
    fn contains(&self, index: usize) -> bool;
    fn is_subset(&self, other: &Self) -> bool;
    fn union_with(&mut self, other: &Self);
    fn difference_with(&mut self, other: &Self);
    fn ones(&'a self) -> Self::Iter;
}

impl<'a> Set<'a> for FixedBitSet {
    type Iter = fixedbitset::Ones<'a>;

    fn create(size: usize) -> Self {
        //FixedBitSet::with_capacity(size)
        let size = (size + 31) & !31usize;
        FixedBitSet::with_capacity(size)
    }
    fn is_empty(&self) -> bool {
        self.as_slice().iter().all(|&x| x == 0)
        //self.count_ones(..) == 0
    }
    fn remove(&mut self, index: usize) {
        self.set(index, false);
    }
    fn insert(&mut self, index: usize) {
        self.set(index, true);
    }
    fn clear(&mut self) {
        self.clear();
    }
    fn contains(&self, index: usize) -> bool {
        self.contains(index)
    }
    fn is_subset(&self, other: &Self) -> bool {
        self.is_subset(other)
    }
    fn union_with(&mut self, other: &Self) {
        if other.len() > self.len() {
            self.grow(other.len());
        }
        for (x, y) in self.as_mut_slice().iter_mut().zip(other.as_slice().iter()) {
            *x |= *y;
        }
    }
    fn difference_with(&mut self, other: &Self) {
        self.difference_with(other)
    }
    fn ones(&'a self) -> Self::Iter {
        self.ones()
    }
}

// XXX: Using a u64 vs. FixedBitSet is about ~2x faster for the test I was running,
// which had a bitset size of 16.
// A u64 is not suitable for general purposes, but it does show how much
// overhead FixedBitSet has for small cardinalities.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BitSet {
    b: u64,
}

pub struct BitSetIter {
    b: u64,
}

impl<'a> Set<'a> for BitSet {
    type Iter = BitSetIter;

    fn create(size: usize) -> Self {
        assert!(size <= 64);
        Self { b: 0 }
    }
    fn is_empty(&self) -> bool {
        self.b == 0
    }
    fn remove(&mut self, index: usize) {
        self.b &= !(1 << index);
    }
    fn insert(&mut self, index: usize) {
        self.b |= 1 << index;
    }
    fn clear(&mut self) {
        self.b = 0
    }
    fn contains(&self, index: usize) -> bool {
        (self.b & (1 << index)) != 0
    }
    fn is_subset(&self, other: &Self) -> bool {
        self.b | other.b == other.b
    }
    fn union_with(&mut self, other: &Self) {
        self.b |= other.b
    }
    fn difference_with(&mut self, other: &Self) {
        self.b &= !other.b
    }
    fn ones(&self) -> Self::Iter {
        BitSetIter { b: self.b }
    }
}

impl Iterator for BitSetIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.b == 0 {
            return None;
        }
        // from the current block, isolate the
        // LSB and subtract 1, producing k:
        // a block with a number of set bits
        // equal to the index of the LSB
        let k = (self.b & (!self.b + 1)) - 1;
        // update block, removing the LSB
        self.b = self.b & (self.b - 1);
        // return offset + (index of LSB)
        Some(k.count_ones() as usize)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SmallBitSet {
    b: smallvec::SmallVec<[u64; 2]>,
}

impl<'a> Set<'a> for SmallBitSet {
    // TODO: Operate over all bits, not just first 64
    type Iter = BitSetIter;

    fn create(size: usize) -> Self {
        assert!(size <= 64);
        Self {
            b: smallvec::SmallVec::from_slice(&[0]),
        }
    }
    fn is_empty(&self) -> bool {
        self.b[0] == 0
    }
    fn remove(&mut self, index: usize) {
        self.b[0] &= !(1 << index);
    }
    fn insert(&mut self, index: usize) {
        self.b[0] |= 1 << index;
    }
    fn clear(&mut self) {
        self.b[0] = 0
    }
    fn contains(&self, index: usize) -> bool {
        (self.b[0] & (1 << index)) != 0
    }
    fn is_subset(&self, other: &Self) -> bool {
        self.b[0] | other.b[0] == other.b[0]
    }
    fn union_with(&mut self, other: &Self) {
        self.b[0] |= other.b[0]
    }
    fn difference_with(&mut self, other: &Self) {
        self.b[0] &= !other.b[0]
    }
    fn ones(&self) -> Self::Iter {
        BitSetIter {
            b: self.b[0] as u64,
        }
    }
}

type Item = u16;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashBitSet(std::collections::HashSet<Item>);

impl<'a> Set<'a> for HashBitSet {
    // TODO: This implementation is atrocious, but so is the perf so *shrug*
    type Iter = std::collections::hash_set::Iter<'a, Item>;

    fn create(size: usize) -> Self {
        assert!(size <= 64);
        Self(std::collections::HashSet::new())
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    fn remove(&mut self, index: usize) {
        self.0.remove(&(index as Item));
    }
    fn insert(&mut self, index: usize) {
        self.0.insert(index as Item);
    }
    fn clear(&mut self) {
        self.0.clear();
    }
    fn contains(&self, index: usize) -> bool {
        self.0.contains(&(index as Item))
    }
    fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }
    fn union_with(&mut self, other: &Self) {
        other.0.iter().for_each(|&x| {
            self.0.insert(x);
        })
    }
    fn difference_with(&mut self, other: &Self) {
        other.0.iter().for_each(|x| {
            self.0.remove(x);
        })
    }
    fn ones(&'a self) -> Self::Iter {
        self.0.iter()
    }
}

impl Hash for HashBitSet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (0..64).for_each(|i| {
            if self.0.contains(&(i as Item)) {
                i.hash(state)
            }
        })
    }
}
