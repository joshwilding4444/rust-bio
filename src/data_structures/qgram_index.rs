// Copyright 2014 Johannes Köster.
// Licensed under the MIT license (http://opensource.org/licenses/MIT)
// This file may not be copied, modified, or distributed
// except according to those terms.


use std::num::{Int, UnsignedInt, NumCast, cast, Float};
use std::collections;
use std::slice;
use std;

use alphabets::{Alphabet, RankTransform};


struct QGrams<'a, Q: UnsignedInt + NumCast> {
    text: slice::Iter<'a, u8>,
    qgram: Q,
    bits: usize,
    mask: Q,
    ranks: RankTransform,
}


impl<'a, Q: UnsignedInt + NumCast> QGrams<'a, Q> {
    pub fn new(q: usize, text: &'a [u8], alphabet: &Alphabet) -> Self {
        let ranks = RankTransform::new(alphabet);
        let mut qgrams = QGrams {
            text: text.iter(),
            qgram: cast(0).unwrap(),
            ranks: ranks,
            bits: (alphabet.len() as f32).log2().ceil() as usize,
            mask: cast((1 << q) - 1).unwrap(),
        };
        for _ in 0..q-1 {
            qgrams.next();
        }

        qgrams
    }

    fn qgram_push(&mut self, a: u8) {
        self.qgram = self.qgram << self.bits;
        self.qgram = (self.qgram | cast(a).unwrap()) & self.mask;
    }
}


impl<'a, Q: UnsignedInt + NumCast> Iterator for QGrams<'a, Q> {
    type Item = Q;

    fn next(&mut self) -> Option<Q> {
        match self.text.next() {
            Some(a) => {
                let b = self.ranks.get(*a);
                self.qgram_push(b);
                Some(self.qgram)
            },
            None    => None
        }
    }
}


pub struct QGramIndex<'a> {
    q: usize,
    alphabet: &'a Alphabet,
    address: Vec<usize>,
    pos: Vec<usize>,
}


impl<'a> QGramIndex<'a> {
    pub fn new(q: usize, text: &[u8], alphabet: &'a Alphabet) -> Self {
        QGramIndex::with_max_count(q, text, alphabet, std::usize::MAX)
    }

    pub fn with_max_count(q: usize, text: &[u8], alphabet: &'a Alphabet, max_count: usize) -> Self {
        let qgram_count = alphabet.len().pow(q as u32);
        let mut address = vec![0; qgram_count + 1];
        let mut pos = vec![0; text.len()];

        for qgram in QGrams::<u32>::new(q, text, alphabet) {
            address[qgram as usize] += 1;
        }

        for g in 1..address.len() {
            if address[g] > max_count {
                // mask qgram
                address[g] = 0;
            }
        }

        for i in 1..address.len() {
            address[i] += address[i - 1];
        }

        {
            let mut offset = vec![0; qgram_count];
            for (i, qgram) in QGrams::<u32>::new(q, text, alphabet).enumerate() {
                let a = address[qgram as usize];
                if address[qgram as usize + 1] - a != 0 {
                    // if not masked, insert positions
                    pos[a + offset[qgram as usize]] = i;
                    offset[qgram as usize] += 1;
                }
            }
        }

        QGramIndex { q: q, alphabet: alphabet, address: address, pos: pos }
    }

    pub fn matches(&self, qgram: u32) -> &[usize] {
        &self.pos[self.address[qgram as usize]..self.address[qgram as usize + 1]]
    }

    pub fn diagonals(&self, pattern: &[u8]) -> Vec<Diagonal> {
        let mut diagonals = collections::HashMap::new();
        for (i, qgram) in QGrams::<u32>::new(self.q, pattern, self.alphabet).enumerate() {
            for p in self.matches(qgram) {
                let diagonal = p - i;
                if diagonals.contains_key(&diagonal) {
                    diagonals.insert(diagonal, 1);
                }
                else {
                    *diagonals.get_mut(&diagonal).unwrap() += 1;
                }
            }
        }
        diagonals.into_iter().map(|(diagonal, count)| Diagonal { pos: diagonal, count: count }).collect()
    }

    pub fn exact_matches(&self, pattern: &[u8]) -> Vec<ExactMatch> {
        let mut diagonals: collections::HashMap<usize, ExactMatch> = collections::HashMap::new();
        let mut intervals = Vec::new();
        for (i, qgram) in QGrams::<u32>::new(self.q, pattern, self.alphabet).enumerate() {
            for &p in self.matches(qgram) {
                let diagonal = p - i;
                if !diagonals.contains_key(&diagonal) {
                    // nothing yet, start new match
                    diagonals.insert(diagonal, ExactMatch {
                            pattern_start: i,
                            pattern_stop: i + self.q,
                            text_start: p,
                            text_stop: p + self.q
                    });
                }
                else {
                    let interval = diagonals.get_mut(&diagonal).unwrap();
                    if interval.pattern_stop == i {
                        // extend exact match
                        interval.pattern_stop = i + self.q;
                        interval.text_stop = p + self.q;
                    }
                    else {
                        // report previous match
                        intervals.push(interval.clone());
                        // mismatch or indel, start new match
                        interval.pattern_start = i;
                        interval.pattern_stop = i + self.q;
                        interval.text_start = p;
                        interval.text_stop = p + self.q;
                    }

                }
            }
        }
        // report remaining intervals
        for (_, interval) in diagonals.into_iter() {
            intervals.push(interval);
        }
        intervals
    }
}


pub struct Diagonal {
    pub pos: usize,
    pub count: usize,
}


#[derive(Clone)]
pub struct ExactMatch {
    pub pattern_start: usize,
    pub pattern_stop: usize,
    pub text_start: usize,
    pub text_stop: usize,
}
