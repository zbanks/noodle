//use bit_set::BitSet;
//use fixedbitset::FixedBitSet;

// TODO: This was sloppily patched in as a compatible alternative to bit_set::BitSet
//impl Set for FixedBitSet {
//    fn create() -> Self {
//        // This would be dynamically sized
//        FixedBitSet::with_capacity(32)
//    }
//
//    fn is_empty(&self) -> bool {
//        self.as_slice() == [0]
//    }
//
//    fn remove(&mut self, index: usize) {
//        self.set(index, false);
//    }
//}
//

//pub type BitSet = FixedBitSet;

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

pub trait Set {
    fn create() -> Self;
    fn is_empty(&self) -> bool;
    fn remove(&mut self, index: usize);
    fn insert(&mut self, index: usize);
    fn clear(&mut self);
    fn contains(&self, index: usize) -> bool;
    fn union_with(&mut self, other: &Self);
    fn difference_with(&mut self, other: &Self);
    fn ones(&self) -> BitSetIter;
}

impl Set for BitSet {
    fn create() -> Self {
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
    fn union_with(&mut self, other: &Self) {
        self.b |= other.b
    }
    fn difference_with(&mut self, other: &Self) {
        self.b &= !other.b
    }
    fn ones(&self) -> BitSetIter {
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
