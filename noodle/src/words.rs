use anyhow::Result;
use fst::Map;
use memmap::Mmap;
use std::convert::TryInto;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

#[cfg(feature = "serialize")]
use serde::Serialize;

pub type Tranche = u8;
pub type Wordlist = Map<Mmap>;

// 28 values: A-Z, Punctuation, Space
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Char(u8);

impl Char {
    pub const PUNCTUATION: Self = Char(26);
    pub const WORD_END: Self = Char(27);
    pub const _MAX: usize = 28;

    pub fn into_u8(self) -> u8 {
        assert!((self.0 as usize) < Self::_MAX);
        match self {
            Char::PUNCTUATION => b'\'',
            Char::WORD_END => b'_',
            _ => b'a' + self.0 as u8,
        }
    }

    pub fn as_index(self) -> usize {
        assert!((self.0 as usize) < Self::_MAX);
        self.0 as usize
    }

    pub fn from_index(i: usize) -> Char {
        assert!((i as usize) < Self::_MAX);
        Char(i as u8)
    }
}

impl From<char> for Char {
    fn from(c: char) -> Self {
        match c {
            'A'..='Z' => Char((c as u32 - 'A' as u32).try_into().unwrap()),
            'a'..='z' => Char((c as u32 - 'a' as u32).try_into().unwrap()),
            ' ' | '_' => Char::WORD_END,
            _ => Char::PUNCTUATION,
        }
    }
}

impl Into<char> for &Char {
    fn into(self) -> char {
        self.into_u8().into()
    }
}

impl From<u8> for Char {
    fn from(c: u8) -> Self {
        match c {
            b'A'..=b'Z' => Char((c - b'A').try_into().unwrap()),
            b'a'..=b'z' => Char((c - b'a').try_into().unwrap()),
            b' ' | b'_' => Char::WORD_END,
            _ => Char::PUNCTUATION,
        }
    }
}

impl Into<u8> for &Char {
    fn into(self) -> u8 {
        self.into_u8()
    }
}

impl fmt::Debug for Char {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c: char = self.into();
        write!(f, "{}", c)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CharBitset(u32);

// TODO: CharBitset is incredibly lightweight compared to BitSet1D,
// 4 stack bytes vs. 16 stack bytes & 8 heap bytes.
// This does mean that there's a some redundant code -- maybe there could
// be a Set trait that these different reprs could implement for consistency?
impl CharBitset {
    pub const EMPTY: Self = Self(0);
    pub const LETTERS: Self = Self((1 << 26) - 1);
    pub const ALL: Self = Self((1 << 28) - 1);

    pub fn from_range(low: char, high: char) -> Self {
        let mut x = 0;
        for c in low..=high {
            x |= CharBitset::from(c).0;
        }
        CharBitset(x)
    }

    pub fn invert(&mut self) {
        self.0 ^= Self::LETTERS.0;
    }

    pub fn union_with(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn difference_with(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    pub fn is_intersecting(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub fn contains(&self, chr: Char) -> bool {
        self.is_intersecting(chr.into())
    }
}

impl From<Char> for CharBitset {
    fn from(c: Char) -> Self {
        Self(1 << c.0)
    }
}

impl From<&Char> for CharBitset {
    fn from(c: &Char) -> Self {
        Self(1 << c.0)
    }
}

impl From<char> for CharBitset {
    fn from(c: char) -> Self {
        let c: Char = c.into();
        c.into()
    }
}

impl fmt::Debug for CharBitset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::LETTERS {
            return write!(f, ".");
        } else if *self == Self::ALL {
            return write!(f, "[a-z_']");
        }
        let ones = self.0.count_ones();
        if ones == 0 {
            return write!(f, "0");
        }
        if ones > 1 {
            write!(f, "[")?;
        }
        for i in 0..=Char::WORD_END.0 {
            if (1 << i) & self.0 != 0 {
                write!(f, "{:?}", Char(i))?;
            }
        }
        if ones > 1 {
            write!(f, "]")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serialize", derive(Serialize))]
pub struct Word {
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub tranche: Tranche,
    #[cfg_attr(feature = "serialize", serde(skip))]
    pub chars: Box<[Char]>,
    pub text: Box<str>,
    pub score: u32,
}

impl Word {
    pub fn new(text: &str, tranche: Tranche, score: u32) -> Self {
        Word {
            text: text.into(),
            chars: text
                .chars()
                // Unicode NFKD normalization
                .nfkd()
                .filter(|c: &char| !is_combining_mark(*c))
                .flat_map(|c: char| {
                    let cs: Box<dyn Iterator<Item = char>> = match c {
                        'æ' | 'Æ' => Box::new(std::iter::once('a').chain(std::iter::once('e'))),
                        'œ' | 'Œ' => Box::new(std::iter::once('o').chain(std::iter::once('e'))),
                        _ => Box::new(std::iter::once(c)),
                    };
                    cs
                })
                // Case folding
                .flat_map(|c| c.to_uppercase())
                .flat_map(|c| c.to_lowercase())
                // Convert UTF-8 characters into Char enums
                .map(|c| (c).into())
                // Add a WORD_END character to the end
                .chain(std::iter::once(Char::WORD_END))
                .collect(),
            tranche,
            score,
        }
    }
}

impl fmt::Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({})",
            self.text,
            self.chars
                .iter()
                .map(|c| c.into_u8() as char)
                .collect::<String>()
        )
    }
}

pub fn wordlist_to_fst<P>(wordlist: &[Word], filename: P) -> Result<()>
where
    P: AsRef<std::path::Path>,
{
    let mut values: Vec<(Box<[u8]>, u64)> = wordlist
        .iter()
        .map(|w| {
            (
                w.chars
                    .iter()
                    .map(|c| c.into())
                    .collect::<Vec<u8>>()
                    .into_boxed_slice(),
                w.score as u64,
            )
        })
        .collect();
    values.sort();
    values.dedup_by(|a, b| a.0 == b.0);

    let writer = io::BufWriter::new(File::create(filename)?);
    let mut builder = fst::MapBuilder::new(writer)?;
    builder.extend_iter(values.into_iter())?;
    builder.finish()?;
    Ok(())
}

pub fn load_wordlist_fst<P>(filename: P) -> Result<Wordlist>
where
    P: AsRef<std::path::Path> + Clone,
{
    let mmap = unsafe { Mmap::map(&File::open(filename)?)? };
    Ok(Map::new(mmap)?)
}

pub fn load_wordlist<P>(filename: P) -> Result<Vec<Word>>
where
    P: AsRef<std::path::Path> + Clone,
{
    // TODO: Correctly propagate errors
    // TODO: Include tranche values in the wordlist file, rather than making them up
    let file = File::open(filename.clone()).unwrap();
    let bufread = io::BufReader::new(file);
    let unzip: Box<dyn BufRead> =
        if filename.as_ref().extension() == Some(std::ffi::OsStr::new("zst")) {
            Box::new(io::BufReader::new(
                zstd::stream::read::Decoder::new(bufread).unwrap(),
            ))
        } else {
            Box::new(bufread)
        };

    const INITIAL_TRANCHE_SIZE: usize = 10000;
    let mut tranche_size: usize = INITIAL_TRANCHE_SIZE;
    let mut tranche_count: usize = 0;
    let mut tranche: Tranche = 0;
    let mut word_count: usize = 0;
    let mut skipped_count: usize = 0;

    let mut wordlist: Vec<_> = unzip
        .lines()
        .filter_map(|line| line.ok())
        .filter_map(|line| {
            // Parse either a plain wordlist, or a 2-column (count, word) variant
            let mut word = line.as_ref();
            let mut score = line.len() as u32;
            if let Some((count_col, word_col)) = line.split_once("\t") {
                if let Ok(count) = count_col.parse::<u32>() {
                    score = count;
                    word = word_col;
                }
            }

            // Bump words which aren't strictly ASCII lowercase into the next tranche
            let t = tranche + (!word.chars().all(|c| c.is_ascii_lowercase())) as Tranche;
            let word = Word::new(word, t, score);

            // Remove any 1-letter words (except "I" and "a") (all words have Char::WORD_END)
            if word.chars.len() <= 2 && word.text.as_ref() != "I" && word.text.as_ref() != "a" {
                skipped_count += 1;
                return None;
            }

            // Remove any words that contain a digit
            if word.text.contains(|c| ('0'..='9').contains(&c)) {
                skipped_count += 1;
                return None;
            }

            word_count += 1;
            tranche_count += 1;
            if tranche_count > tranche_size {
                // Make each tranche 150% the size of the previous one
                tranche_size += tranche_size / 2;
                tranche_count = 0;
                tranche += 1;
            }

            Some(word)
        })
        .collect();
    wordlist.sort();
    println!(
        "Loaded {} words with {} tranches (skipped {})",
        wordlist.len(),
        tranche + 1,
        skipped_count
    );
    Ok(wordlist)
}

pub trait WordListRef<'word> {
    fn size(&self) -> usize;
    fn borrow(&self, index: usize) -> &'word Word;
}

impl<'word, 'a> WordListRef<'word> for &'a [&'word Word] {
    fn size(&self) -> usize {
        self.len()
    }

    fn borrow(&self, index: usize) -> &'word Word {
        self[index]
    }
}

impl<'word> WordListRef<'word> for &'word [Word] {
    fn size(&self) -> usize {
        self.len()
    }

    fn borrow(&self, index: usize) -> &'word Word {
        &self[index]
    }
}
