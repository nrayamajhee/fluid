//! Keyed list reconciliation, independent of the DOM.

use std::collections::HashSet;
use std::hash::Hash;

/// What changes between two key lists.
#[derive(Debug, PartialEq, Eq)]
pub struct Diff<K> {
  /// Keys in the old list but not the new one.
  pub removed: Vec<K>,
  /// The new list without duplicate keys (the first one wins).
  pub order: Vec<K>,
  /// Indices into `order` of keys that weren't in the old list.
  pub added: Vec<usize>,
}

pub fn diff<K: Eq + Hash + Clone>(old: &[K], new: impl IntoIterator<Item = K>) -> Diff<K> {
  let old_keys: HashSet<&K> = old.iter().collect();
  let mut seen = HashSet::new();
  let mut order = Vec::new();
  let mut added = Vec::new();
  for key in new {
    if !seen.insert(key.clone()) {
      continue;
    }
    if !old_keys.contains(&key) {
      added.push(order.len());
    }
    order.push(key);
  }
  let removed = old
    .iter()
    .filter(|key| !seen.contains(*key))
    .cloned()
    .collect();
  Diff {
    removed,
    order,
    added,
  }
}

/// The sibling operations [`arrange`] needs.
pub trait Siblings {
  type Node: Clone;
  fn next_sibling(&self, node: &Self::Node) -> Option<Self::Node>;
  fn same(&self, a: &Self::Node, b: &Self::Node) -> bool;
  fn insert_before(&mut self, node: &Self::Node, before: &Self::Node);
}

/// Puts `rows` (each a run of nodes) in order right before `end`, walking
/// back from the end and moving only nodes not already in place.
pub fn arrange<S: Siblings>(dom: &mut S, rows: &[&[S::Node]], end: &S::Node) {
  let mut next = end.clone();
  for row in rows.iter().rev() {
    for node in row.iter().rev() {
      let in_place = dom
        .next_sibling(node)
        .is_some_and(|sibling| dom.same(&sibling, &next));
      if !in_place {
        dom.insert_before(node, &next);
      }
      next = node.clone();
    }
  }
}
