//! Naive LR(1) generation algorithm.

use kernel_set;
use session::Session;
use grammar::repr::*;
use std::fmt::{Debug, Formatter, Error};
use std::rc::Rc;
use util::{Map, Prefix};

use self::lookahead::Lookahead;

pub mod ascent;

mod backtrace;
mod core;
mod error;
mod first;
mod la0;
mod lookahead;

#[cfg(test)] mod interpret;

pub use self::error::report_error;

#[derive(Debug)]
pub struct State<'grammar> {
    index: StateIndex,
    items: Items<'grammar>,
    tokens: Map<Lookahead, Action<'grammar>>,
    conflicts: Map<Lookahead, Vec<Conflict<'grammar>>>,
    gotos: Map<NonterminalString, StateIndex>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Action<'grammar> {
    Shift(StateIndex),
    Reduce(&'grammar Production),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Conflict<'grammar> {
    // when in this state...
    state: StateIndex,

    // we can reduce...
    production: &'grammar Production,

    // but we can also...
    action: Action<'grammar>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Items<'grammar> {
    vec: Rc<Vec<Item<'grammar>>>
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct StateIndex(usize);

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Item<'grammar> {
    production: &'grammar Production,
    /// the dot comes before `index`, so `index` would be 1 for X = A (*) B C
    index: usize,
    lookahead: Lookahead,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct LR0Item<'grammar> {
    production: &'grammar Production,
    index: usize
}

#[derive(Debug)]
pub struct TableConstructionError<'grammar> {
    // LR(1) state set. Some of these states are in error.
    states: Vec<State<'grammar>>,
}

pub fn build_states<'grammar>(session: &Session,
                              grammar: &'grammar Grammar,
                              start: NonterminalString)
                              -> Result<Vec<State<'grammar>>, TableConstructionError<'grammar>>
{
    match grammar.algorithm {
        Algorithm::LR1 => core::build_lr1_states(session, grammar, start),
        Algorithm::LALR1 => la0::lalr_states(session, grammar, start),
    }
}

impl<'grammar> Item<'grammar> {
    fn to_lr0(&self) -> LR0Item<'grammar> {
        LR0Item { production: self.production, index: self.index }
    }

    fn can_shift(&self) -> bool {
        self.index < self.production.symbols.len()
    }

    fn can_reduce(&self) -> bool {
        self.index == self.production.symbols.len()
    }

    fn shifted_item(&self) -> Option<(Symbol, Item<'grammar>)> {
        if self.can_shift() {
            Some((self.production.symbols[self.index],
                  Item { production: self.production,
                         index: self.index + 1,
                         lookahead: self.lookahead }))
        } else {
            None
        }
    }

    fn shift_symbol(&self) -> Option<(Symbol, &[Symbol])> {
        if self.can_shift() {
            Some((self.production.symbols[self.index], &self.production.symbols[self.index+1..]))
        } else {
            None
        }
    }
}

impl<'grammar> kernel_set::Kernel for Items<'grammar> {
    type Index = StateIndex;

    fn index(c: usize) -> StateIndex {
        StateIndex(c)
    }
}

impl<'grammar> Debug for Item<'grammar> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{} ={} (*){} [{:?}]",
               self.production.nonterminal,
               Prefix(" ", &self.production.symbols[..self.index]),
               Prefix(" ", &self.production.symbols[self.index..]),
               self.lookahead)
    }
}

impl<'grammar> Debug for LR0Item<'grammar> {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "{} ={} (*){}",
               self.production.nonterminal,
               Prefix(" ", &self.production.symbols[..self.index]),
               Prefix(" ", &self.production.symbols[self.index..]))
    }
}

impl Debug for Lookahead {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match *self {
            Lookahead::EOF => write!(fmt, "EOF"),
            Lookahead::Terminal(s) => write!(fmt, "{}", s),
        }
    }
}

impl Debug for StateIndex {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        write!(fmt, "S{}", self.0)
    }
}

impl<'grammar> State<'grammar> {
    fn prefix(&self) -> &'grammar [Symbol] {
        // Each state fn takes as argument the longest prefix of any
        // item. Note that all items must have compatible prefixes.
        let (_, prefix) =
            self.items.vec
                      .iter()
                      .map(|item| &item.production.symbols[..item.index])
                      .map(|symbols| (symbols.len(), symbols))
                      .max() // grr, max_by is unstable :(
                      .unwrap();

        debug_assert!(
            self.items.vec
                      .iter()
                      .all(|item| prefix.ends_with(&item.production.symbols[..item.index])));

        prefix
    }
}

impl<'grammar> Action<'grammar> {
    pub fn shift(&self) -> Option<StateIndex> {
        match *self {
            Action::Shift(next_index) => Some(next_index),
            _ => None
        }
    }
    pub fn reduce(&self) -> Option<&'grammar Production> {
        match *self {
            Action::Reduce(production) => Some(production),
            _ => None,
        }
    }
}

