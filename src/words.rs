use std::convert::TryInto;
use std::fmt;
use std::io::{self, BufRead};

// 28 values: A-Z, Punctuation, Space
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Char(u8);

impl Char {
    pub const PUNCTUATION: Self = Char(26);
    pub const WHITESPACE: Self = Char(27);

    pub fn into_char(self) -> char {
        match self {
            Char::PUNCTUATION => '\'',
            Char::WHITESPACE => '_',
            _ => std::char::from_u32('A' as u32 + self.0 as u32).unwrap(),
        }
    }
}

impl From<char> for Char {
    fn from(c: char) -> Self {
        match c {
            'A'..='Z' => Char((c as u32 - 'A' as u32).try_into().unwrap()),
            'a'..='z' => Char((c as u32 - 'a' as u32).try_into().unwrap()),
            ' ' | '_' => Char::WHITESPACE,
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

    pub fn union(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn is_intersecting(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }
}

impl From<Char> for CharBitset {
    fn from(c: Char) -> Self {
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
        let ones = self.0.count_ones();
        if ones == 0 {
            return write!(f, "0");
        }
        if ones > 1 {
            write!(f, "[")?;
        }
        for i in 0..=Char::WHITESPACE.0 {
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Word {
    pub text: String,
    pub chars: Vec<Char>,
}

impl Word {
    pub fn new(text: &str) -> Self {
        Word {
            text: text.into(),
            chars: text.chars().map(|c| c.into()).collect(),
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

pub fn load_wordlist<P>(filename: P) -> Result<Vec<Word>, ()>
where
    P: AsRef<std::path::Path>,
{
    // TODO: Correctly propagate errors
    let file = std::fs::File::open(filename).unwrap();
    let lines = io::BufReader::new(file).lines();
    Ok(lines
        .filter_map(|line| line.ok())
        .filter_map(|line| {
            if line.len() > 1 || line == "I" || line == "a" {
                Some(Word::new(&line))
            } else {
                None
            }
        })
        .collect())
}
