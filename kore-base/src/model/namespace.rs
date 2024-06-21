// Copyright 2024 Kore Ledger, SL
// SPDX-License-Identifier: AGPL-3.0-or-later

//! # Namespace model.
//!

use borsh::{BorshDeserialize, BorshSerialize};

use serde::{Deserialize, Serialize};

use std::cmp::Ordering;
use std::fmt::{Error, Formatter};

/// This is the name space for a `Subject`.
///
#[derive(
    Clone,
    Hash,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    BorshDeserialize,
    BorshSerialize,
)]
pub struct Namespace(Vec<String>);

/// `Namespace` implementation.
impl Namespace {
    /// Create a new `Namespace`.
    ///
    /// # Returns
    ///
    /// A new `Namespace`.
    ///
    pub fn new() -> Self {
        Namespace(Vec::new())
    }

    /// Add a name to the `Namespace`.
    ///
    /// # Arguments
    ///
    /// * `name` - The name to add.
    ///
    pub fn add(&mut self, name: &str) {
        self.0.push(name.to_owned())
    }

    /// Root name of the `Namespace`.
    ///
    /// # Returns
    ///
    /// The root name of the `Namespace`.
    ///
    pub fn root(&self) -> Namespace {
        if self.0.len() == 1 {
            self.clone()
        } else if !self.0.is_empty() {
            Self(self.0.iter().take(1).cloned().collect())
        } else {
            Self(Vec::new())
        }
    }

    /// Returns the parent of the name space.
    ///
    /// # Returns
    ///
    /// Returns the parent of the name space.
    ///
    pub fn parent(&self) -> Self {
        if self.0.len() > 1 {
            let mut tokens = self.0.clone();
            tokens.truncate(tokens.len() - 1);
            Self(tokens)
        } else {
            Self(Vec::new())
        }
    }

    /// Returns the key of the name space.
    ///
    /// # Returns
    ///
    /// Returns the key of the path.
    ///
    pub fn key(&self) -> String {
        self.0.last().cloned().unwrap_or_else(|| "".to_string())
    }

    /// Returns the levels size of the name space.
    ///
    /// # Returns
    ///
    /// Returns the levels size of the name space.
    ///
    pub fn level(&self) -> usize {
        self.0.len()
    }

    /// Returns the name space at a specific level.
    ///
    /// # Arguments
    ///
    /// * `level` - The level to return the name space at.
    ///
    /// # Returns
    ///
    /// Returns the name space at a specific level.
    ///
    pub fn at_level(&self, level: usize) -> Self {
        if level < 1 || level >= self.level() {
            self.clone()
        } else if self.is_top_level() {
            self.root()
        } else if level == self.level() - 1 {
            self.parent()
        } else {
            let mut tokens = self.0.clone();
            tokens.truncate(level);
            Self(tokens)
        }
    }

    /// Returns if the name space is empty.
    ///
    /// # Returns
    ///
    /// Returns `true` if the name space is empty.
    ///
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns if the name space is an ancestor of another name space.
    ///
    /// # Arguments
    ///
    /// * `other` - The other name space to check.
    ///
    /// # Returns
    ///
    /// Returns `true` if the name space is an ancestor of the other name space.
    ///
    pub fn is_ancestor_of(&self, other: &Namespace) -> bool {
        let me = format!("{}.", self);
        other.to_string().as_str().starts_with(me.as_str())
    }

    /// Returns if the name space is a descendant of another name space.
    ///
    /// # Arguments
    ///
    /// * `other` - The other name space to check.
    ///
    /// # Returns
    ///
    /// Returns `true` if the name space is a descendant of the other name space.
    ///
    pub fn is_descendant_of(&self, other: &Namespace) -> bool {
        let me = self.to_string();
        me.as_str().starts_with(format!("{}.", other).as_str())
    }

    /// Returns if the name space is a parent of another name space.
    ///
    /// # Arguments
    ///
    /// * `other` - The other name space to check.
    ///
    /// # Returns
    ///
    /// Returns `true` if the name space is a parent of the other name space.
    ///
    pub fn is_parent_of(&self, other: &Namespace) -> bool {
        *self == other.parent()
    }

    /// Returns if the name space is a child of another name space.
    ///
    /// # Arguments
    ///
    /// * `other` - The other name space to check.
    ///
    /// # Returns
    ///
    /// Returns `true` if the name space is a child of the other name space.
    ///
    pub fn is_child_of(&self, other: &Namespace) -> bool {
        self.parent() == *other
    }

    /// Returns if the name space is top level.
    ///
    /// # Returns
    ///
    /// Returns `true` if the name space is top level.
    ///
    pub fn is_top_level(&self) -> bool {
        self.0.len() == 1
    }
}

impl std::fmt::Display for Namespace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.level().cmp(&1) {
            Ordering::Less => write!(f, ""),
            Ordering::Equal => write!(f, "{}", self.0[0]),
            Ordering::Greater => write!(f, "{}", self.0.join(".")),
        }
    }
}

impl std::fmt::Debug for Namespace {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        match self.level().cmp(&1) {
            Ordering::Less => {
                write!(f, "")
            }
            Ordering::Equal => write!(f, "{}", self.0[0]),
            Ordering::Greater => write!(f, "{}", self.0.join(".")),
        }
    }
}

impl Default for Namespace {
    fn default() -> Self {
        Namespace::new()
    }
}

impl From<&str> for Namespace {
    fn from(str: &str) -> Self {
        let tokens: Vec<String> = str
            .split('.')
            .filter(|x| !x.trim().is_empty())
            .map(|s| s.to_string())
            .collect();

        Namespace(tokens)
    }
}

impl From<String> for Namespace {
    fn from(str: String) -> Self {
        Namespace::from(str.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace() {
        let ns = Namespace::from("a.b.c");
        assert_eq!(ns.level(), 3);
        assert_eq!(ns.key(), "c");
        assert_eq!(ns.root().to_string(), "a");
        assert_eq!(ns.parent().to_string(), "a.b");
        assert_eq!(ns.at_level(1).to_string(), "a");
        assert_eq!(ns.at_level(2).to_string(), "a.b");
        assert_eq!(ns.at_level(3).to_string(), "a.b.c");
        assert_eq!(ns.is_empty(), false);
        assert_eq!(ns.is_ancestor_of(&Namespace::from("a.b.c.d")), true);
        assert_eq!(ns.is_descendant_of(&Namespace::from("a.b")), true);
        assert_eq!(ns.is_parent_of(&Namespace::from("a.b.c.d")), true);
        assert_eq!(ns.is_child_of(&Namespace::from("a.b")), true);
        assert_eq!(ns.is_top_level(), false);
    }
}
