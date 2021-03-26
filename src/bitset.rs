use std::fmt;

// This borrows some implementation from the fixedbitset crate, v0.4.0,
// which is licensed under the MIT license.
// https://github.com/petgraph/fixedbitset

pub type BitSet1D = BitSet<()>;
pub type BitSet3D = BitSet<(usize, usize)>;

pub type BitSetRef1D<'a> = BitSetRef<'a, ()>;
#[allow(dead_code)]
pub type BitSetRef3D<'a> = BitSetRef<'a, (usize, usize)>;

pub type BitSetRefMut1D<'a> = BitSetRefMut<'a, ()>;
#[allow(dead_code)]
pub type BitSetRefMut3D<'a> = BitSetRefMut<'a, (usize, usize)>;

type Block = u32;
const BLOCK_BITS: usize = 32;

fn div_rem(x: usize, d: usize) -> (usize, usize) {
    (x / d, x % d)
}

pub trait Index: Copy {
    type Range: std::slice::SliceIndex<[Block], Output = [Block]>;

    fn total_size(&self) -> usize;
    fn transpose(&self, x: usize) -> Self;
    fn slice(&self, sub: Self) -> Self::Range;
}

impl Index for () {
    type Range = std::ops::RangeFull;

    fn total_size(&self) -> usize {
        1
    }
    fn transpose(&self, _x: usize) -> Self {}
    fn slice(&self, _sub: Self) -> Self::Range {
        ..
    }
}

impl Index for usize {
    type Range = std::ops::Range<usize>;

    fn total_size(&self) -> usize {
        *self
    }
    fn transpose(&self, x: usize) -> Self {
        x
    }
    fn slice(&self, sub: Self) -> Self::Range {
        *self * sub..*self * (sub + 1)
    }
}

impl Index for (usize, usize) {
    type Range = std::ops::Range<usize>;

    fn total_size(&self) -> usize {
        self.0 * self.1
    }
    fn transpose(&self, x: usize) -> Self {
        (self.1, x)
    }
    fn slice(&self, sub: Self) -> Self::Range {
        let start = self.1 * (self.0 * sub.0 + sub.1);
        start..start + self.1
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct BitSet<Idx: Index> {
    blocks: Box<[Block]>,
    size: Idx,
}

impl<Idx: Index> BitSet<Idx> {
    pub fn new(outer_size: Idx, inner_size: usize) -> Self {
        let (mut block_size, rem) = div_rem(inner_size, BLOCK_BITS);
        block_size += (rem > 0) as usize;

        let blocks_count = outer_size.total_size();
        Self {
            blocks: vec![0; blocks_count * block_size].into_boxed_slice(),
            size: outer_size.transpose(block_size),
        }
    }

    pub fn slice(&self, index: Idx) -> BitSetRef<'_, ()> {
        let range = self.size.slice(index);
        let blocks = unsafe { self.blocks.get_unchecked(range) };
        BitSetRef { blocks, size: () }
    }

    pub fn slice_mut(&mut self, index: Idx) -> BitSetRefMut<'_, ()> {
        let range = self.size.slice(index);
        let blocks = unsafe { self.blocks.get_unchecked_mut(range) };
        BitSetRefMut { blocks, size: () }
    }

    pub fn borrow(&self) -> BitSetRef<'_, Idx> {
        BitSetRef {
            blocks: &self.blocks,
            size: self.size,
        }
    }

    pub fn borrow_mut(&mut self) -> BitSetRefMut<'_, Idx> {
        BitSetRefMut {
            blocks: &mut self.blocks,
            size: self.size,
        }
    }
}

impl BitSet<()> {
    pub fn resize(&self, new_size: usize) -> Self {
        assert!(self.borrow().ones().all(|x| x < new_size));

        let mut new_bitset = Self::new((), new_size);
        self.blocks
            .iter()
            .zip(new_bitset.blocks.iter_mut())
            .for_each(|(old_b, new_b)| *new_b = *old_b);
        new_bitset
    }
}

impl fmt::Display for BitSet<()> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", &self.borrow().ones().collect::<Vec<_>>())
    }
}

impl fmt::Debug for BitSet<()> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let implied_size = self.blocks.len();
        f.debug_struct("BitSet1D")
            .field("size", &implied_size)
            .field("sets", &self.borrow().ones().collect::<Vec<_>>())
            .finish()
    }
}

impl BitSet<(usize, usize)> {
    pub fn slice2d(&self, index: usize) -> BitSetRef<'_, usize> {
        let range = self.size.total_size() * index..self.size.total_size() * (index + 1);
        let blocks = unsafe { self.blocks.get_unchecked(range) };
        BitSetRef {
            blocks,
            size: self.size.1,
        }
    }
    pub fn slice2d_mut(&mut self, index: usize) -> BitSetRefMut<'_, usize> {
        let range = self.size.total_size() * index..self.size.total_size() * (index + 1);
        let blocks = unsafe { self.blocks.get_unchecked_mut(range) };
        BitSetRefMut {
            blocks,
            size: self.size.1,
        }
    }

    /// Special function for Matcher::next_phrase that is hard to implement there due to aliasing.
    /// `self` represents a 3D bitset: [x][y][z].
    /// Removes all [index][y+1][z] where [index][y][z] exists
    pub fn compact_distance_set(&mut self, index: usize) {
        let blocks = &mut self.blocks[self.size.0 * index..self.size.0 * (index + 1)];
        let stride = blocks.len() / self.size.1;
        debug_assert_eq!(stride * self.size.1, blocks.len());
        for i in 0..stride {
            let mut mask: Block = 0;
            for j in 0..self.size.1 {
                let b = &mut blocks[j * stride + i];
                *b &= !mask;
                mask |= *b;
            }
        }
    }
}

impl fmt::Debug for BitSet<(usize, usize)> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let implied_size = (
            self.blocks.len() / self.size.total_size(),
            self.size.0,
            self.size.1,
        );
        let mut values = vec![];
        for x in 0..implied_size.0 {
            let mut row = vec![];
            for y in 0..implied_size.1 {
                row.push(self.slice((x, y)).ones().collect::<Vec<_>>());
            }
            values.push(row);
        }
        f.debug_struct("BitSet3D")
            .field("size", &implied_size)
            .field("sets", &values)
            .finish()
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub struct BitSetRef<'a, Idx: Index> {
    blocks: &'a [Block],
    size: Idx,
}

impl<'a, Idx: Index> BitSetRef<'a, Idx> {
    pub fn is_empty(&self) -> bool {
        self.blocks.iter().all(|&x| x == 0)
    }
    pub fn contains(&self, index: usize) -> bool {
        let (block, bit) = div_rem(index, BLOCK_BITS);
        let b = unsafe { self.blocks.get_unchecked(block) };
        b & (1 << bit) != 0
    }
    pub fn is_subset(&self, other: &Self) -> bool {
        debug_assert_eq!(self.blocks.len(), other.blocks.len());
        self.blocks
            .iter()
            .zip(other.blocks)
            .all(|(&x, &y)| (x | y) == y)
    }
    pub fn ones(&'a self) -> Ones<'a> {
        Ones {
            block: self.blocks[0],
            offset: 0,
            remaining_blocks: &self.blocks[1..],
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct BitSetRefMut<'a, Idx: Index> {
    blocks: &'a mut [Block],
    size: Idx,
}

impl<'a, Idx: Index> BitSetRefMut<'a, Idx> {
    pub fn reborrow(self) -> BitSetRef<'a, Idx> {
        BitSetRef {
            blocks: self.blocks,
            size: self.size,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.iter().all(|&x| x == 0)
    }
    pub fn contains(&self, index: usize) -> bool {
        let (block, bit) = div_rem(index, BLOCK_BITS);
        let b = unsafe { self.blocks.get_unchecked(block) };
        b & (1 << bit) != 0
    }
    pub fn ones(&'a self) -> Ones<'a> {
        Ones {
            block: self.blocks[0],
            offset: 0,
            remaining_blocks: &self.blocks[1..],
        }
    }
    pub fn remove(&mut self, index: usize) {
        let (block, bit) = div_rem(index, BLOCK_BITS);
        let b = unsafe { self.blocks.get_unchecked_mut(block) };
        *b &= !(1 << bit);
    }
    pub fn insert(&mut self, index: usize) {
        let (block, bit) = div_rem(index, BLOCK_BITS);
        let b = unsafe { self.blocks.get_unchecked_mut(block) };
        *b |= 1 << bit;
    }
    pub fn clear(&mut self) {
        self.blocks.iter_mut().for_each(|x| *x = 0);
    }
    pub fn union_with(&mut self, other: BitSetRef<'_, Idx>) {
        debug_assert_eq!(self.blocks.len(), other.blocks.len());
        for i in 0..self.blocks.len() {
            unsafe { *self.blocks.get_unchecked_mut(i) |= *other.blocks.get_unchecked(i) };
        }
    }
    pub fn difference_with(&mut self, other: BitSetRef<'_, Idx>) {
        debug_assert_eq!(self.blocks.len(), other.blocks.len());
        for i in 0..self.blocks.len() {
            unsafe { *self.blocks.get_unchecked_mut(i) &= !*other.blocks.get_unchecked(i) };
        }
    }
    pub fn intersect_with(&mut self, other: BitSetRef<'_, Idx>) {
        debug_assert_eq!(self.blocks.len(), other.blocks.len());
        for i in 0..self.blocks.len() {
            unsafe { *self.blocks.get_unchecked_mut(i) &= *other.blocks.get_unchecked(i) };
        }
    }
    pub fn copy_from(&mut self, other: BitSetRef<'_, Idx>) {
        debug_assert_eq!(self.blocks.len(), other.blocks.len());
        for i in 0..self.blocks.len() {
            unsafe { *self.blocks.get_unchecked_mut(i) = *other.blocks.get_unchecked(i) };
        }
    }
}

pub struct Ones<'a> {
    block: Block,
    offset: usize,
    remaining_blocks: &'a [Block],
}

impl<'a> Iterator for Ones<'a> {
    type Item = usize; // the bit position of the '1'

    fn next(&mut self) -> Option<Self::Item> {
        while self.block == 0 {
            if self.remaining_blocks.is_empty() {
                return None;
            }
            self.block = self.remaining_blocks[0];
            self.remaining_blocks = &self.remaining_blocks[1..];
            self.offset += BLOCK_BITS;
        }
        #[allow(clippy::unnecessary_cast)]
        let t = self.block & (0 as Block).wrapping_sub(self.block);
        let r = self.block.trailing_zeros() as usize;
        self.block ^= t;
        Some(self.offset + r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitset_1d() {
        let mut bitset = BitSet1D::new((), 100);
        bitset.borrow_mut().insert(2);
        bitset.slice_mut(()).insert(53);
        assert_eq!(bitset.borrow().ones().collect::<Vec<_>>(), vec![2, 53]);
    }

    #[test]
    fn bitset_3d() {
        let mut bitset = BitSet3D::new((2, 3), 100);
        for i in 0..2 {
            for j in 0..3 {
                bitset.slice_mut((i, j)).insert(i * j);
                bitset.slice_mut((i, j)).insert(i + j);
                bitset.slice_mut((i, j)).insert(40 + i);
                bitset.slice_mut((i, j)).insert(80 + j);
            }
        }

        assert_eq!(
            bitset.slice((0, 0)).ones().collect::<Vec<_>>(),
            vec![0, 40, 80]
        );
        assert_eq!(
            bitset.slice((0, 1)).ones().collect::<Vec<_>>(),
            vec![0, 1, 40, 81]
        );
        assert_eq!(
            bitset.slice((0, 2)).ones().collect::<Vec<_>>(),
            vec![0, 2, 40, 82]
        );
        assert_eq!(
            bitset.slice((1, 0)).ones().collect::<Vec<_>>(),
            vec![0, 1, 41, 80]
        );
        assert_eq!(
            bitset.slice((1, 1)).ones().collect::<Vec<_>>(),
            vec![1, 2, 41, 81]
        );
        assert_eq!(
            bitset.slice((1, 2)).ones().collect::<Vec<_>>(),
            vec![2, 3, 41, 82]
        );
    }

    #[test]
    fn bitset_1d_union() {
        let mut bitset_3 = BitSet1D::new((), 1000);
        let mut bitset_5 = BitSet1D::new((), 1000);

        {
            let mut slice_3 = bitset_3.slice_mut(());
            let mut slice_5 = bitset_5.slice_mut(());

            for i in 0..1000 {
                if i % 3 == 0 {
                    slice_3.insert(i);
                }
                if i % 5 == 0 {
                    slice_5.insert(i);
                }
            }
        }

        let mut bitset_3or5 = BitSet1D::new((), 1000);
        let mut slice_3or5 = bitset_3or5.borrow_mut();

        slice_3or5.union_with(bitset_3.borrow());
        slice_3or5.union_with(bitset_5.borrow());

        for i in slice_3or5.ones() {
            assert!(i % 3 == 0 || i % 5 == 0);
        }

        for i in 0..1000 {
            assert_eq!(bitset_3.borrow().contains(i), i % 3 == 0);
            assert_eq!(bitset_5.borrow().contains(i), i % 5 == 0);
            assert_eq!(slice_3or5.contains(i), i % 3 == 0 || i % 5 == 0);
        }
    }
}
