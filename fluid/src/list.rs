//! Keyed list reconciliation, kept DOM-free so it is tested natively.

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

#[cfg(test)]
mod tests {
  use super::*;

  /// A parent's children as a list of ids.
  struct Model {
    children: Vec<u32>,
    moves: usize,
  }

  impl Siblings for Model {
    type Node = u32;
    fn next_sibling(&self, node: &u32) -> Option<u32> {
      let at = self.children.iter().position(|child| child == node)?;
      self.children.get(at + 1).copied()
    }
    fn same(&self, a: &u32, b: &u32) -> bool {
      a == b
    }
    fn insert_before(&mut self, node: &u32, before: &u32) {
      self.moves += 1;
      self.children.retain(|child| child != node);
      let at = self
        .children
        .iter()
        .position(|child| child == before)
        .unwrap();
      self.children.insert(at, *node);
    }
  }

  const END: u32 = 0;

  fn arranged(start: &[u32], order: &[u32]) -> (Vec<u32>, usize) {
    let mut model = Model {
      children: start.iter().copied().chain([END]).collect(),
      moves: 0,
    };
    let rows: Vec<[u32; 1]> = order.iter().map(|&key| [key]).collect();
    let rows: Vec<&[u32]> = rows.iter().map(|row| &row[..]).collect();
    arrange(&mut model, &rows, &END);
    model.children.pop();
    (model.children, model.moves)
  }

  #[test]
  fn diff_reports_removed_added_and_dedupes() {
    let d = diff(&[1, 2, 3], [3, 4, 1, 4, 5]);
    assert_eq!(d.removed, [2]);
    assert_eq!(d.order, [3, 4, 1, 5]);
    assert_eq!(d.added, [1, 3]);
    assert!(diff::<u8>(&[], []).order.is_empty());
  }

  #[test]
  fn unchanged_lists_move_nothing() {
    assert_eq!(arranged(&[1, 2, 3], &[1, 2, 3]), (vec![1, 2, 3], 0));
  }

  #[test]
  fn appends_and_inserts_move_only_new_nodes() {
    // New nodes start out detached (here: absent), as freshly built rows.
    assert_eq!(arranged(&[1, 2], &[1, 2, 3]), (vec![1, 2, 3], 1));
    assert_eq!(arranged(&[1, 3], &[1, 2, 3]), (vec![1, 2, 3], 1));
    assert_eq!(arranged(&[], &[1, 2]), (vec![1, 2], 2));
  }

  #[test]
  fn swaps_and_reversals_end_in_order() {
    assert_eq!(arranged(&[1, 2, 3, 4], &[1, 3, 2, 4]).0, [1, 3, 2, 4]);
    assert_eq!(arranged(&[1, 2, 3, 4], &[4, 3, 2, 1]).0, [4, 3, 2, 1]);
    // Rotations move every row after the moved one (no LIS optimisation).
    assert_eq!(arranged(&[1, 2, 3], &[3, 1, 2]), (vec![3, 1, 2], 2));
    assert_eq!(arranged(&[1, 2, 3], &[2, 3, 1]), (vec![2, 3, 1], 1));
  }

  #[test]
  fn every_permutation_of_five_is_reached() {
    fn permutations(items: Vec<u32>) -> Vec<Vec<u32>> {
      if items.len() <= 1 {
        return vec![items];
      }
      let mut all = Vec::new();
      for (at, &first) in items.iter().enumerate() {
        let mut rest = items.clone();
        rest.remove(at);
        for mut tail in permutations(rest) {
          tail.insert(0, first);
          all.push(tail);
        }
      }
      all
    }
    for order in permutations(vec![1, 2, 3, 4, 5]) {
      let (children, moves) = arranged(&[1, 2, 3, 4, 5], &order);
      assert_eq!(children, order);
      assert!(moves <= 4, "{order:?} took {moves} moves");
    }
  }

  #[test]
  fn multi_node_rows_stay_together() {
    let mut model = Model {
      children: vec![10, 11, 20, 21, END],
      moves: 0,
    };
    arrange(&mut model, &[&[20, 21], &[10, 11]], &END);
    assert_eq!(model.children, [20, 21, 10, 11, END]);
  }
}
