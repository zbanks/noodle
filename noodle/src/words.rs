use std::convert::TryInto;
use std::fmt;
use std::io::{self, BufRead};
use unicode_normalization::UnicodeNormalization;

#[cfg(feature = "serialize")]
use serde::Serialize;

pub type Tranche = u8;

// 28 values: A-Z, Punctuation, Space
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Char(u8);

impl Char {
    pub const PUNCTUATION: Self = Char(26);
    pub const WORD_END: Self = Char(27);
    pub const _MAX: usize = 28;

    pub fn into_char(self) -> char {
        assert!((self.0 as usize) < Self::_MAX);
        match self {
            Char::PUNCTUATION => '\'',
            Char::WORD_END => '_',
            _ => std::char::from_u32('a' as u32 + self.0 as u32).unwrap(),
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
        self.into_char()
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
    pub const LETTERS_BUT_I: Self = Self(((1 << 26) - 1) & !(1 << ('I' as u32 - 'A' as u32)));
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
    pub chars: Vec<Char>,
    pub text: String,
    pub score: u32,
}

impl Word {
    pub fn new(text: &str, tranche: Tranche, score: u32) -> Self {
        Word {
            text: text.into(),
            chars: text
                .chars()
                .nfkd()
                .map(|c| c.into())
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
            self.chars.iter().map(|c| c.into_char()).collect::<String>()
        )
    }
}

#[allow(clippy::result_unit_err)]
pub fn load_wordlist<P>(filename: P) -> Result<Vec<Word>, ()>
where
    P: AsRef<std::path::Path>,
{
    // TODO: Correctly propagate errors
    // TODO: Include tranche values in the wordlist file, rather than making them up
    let file = std::fs::File::open(filename).unwrap();
    let lines = io::BufReader::new(file).lines();

    const INITIAL_TRANCHE_SIZE: usize = 10000;
    let mut tranche_size: usize = INITIAL_TRANCHE_SIZE;
    let mut tranche_count: usize = 0;
    let mut tranche: Tranche = 0;
    let mut word_count: usize = 0;

    let mut wordlist: Vec<_> = lines
        .filter_map(|line| line.ok())
        .filter_map(|line| {
            if line.len() > 1 || line == "I" || line == "a" {
                // Bump words which aren't strictly ASCII lowercase into the next tranche
                let t = tranche + (!line.chars().all(|c| c.is_ascii_lowercase())) as Tranche;

                word_count += 1;
                tranche_count += 1;
                if tranche_count > tranche_size {
                    // Make each tranche 150% the size of the previous one
                    tranche_size += tranche_size / 2;
                    tranche_count = 0;
                    tranche += 1;
                }

                let score = word_count as u32;
                Some(Word::new(&line, t, score))
            } else {
                None
            }
        })
        .collect();
    wordlist.sort();
    println!(
        "Loaded {} words with {} tranches",
        wordlist.len(),
        tranche + 1
    );
    Ok(wordlist)
}
