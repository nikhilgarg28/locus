//! A real minimum-degree-two B-tree: splits, rotations, merges, and deletion.
#![forbid(unsafe_code)]
use crate::{
    logic::{Map, Observe, TotalOrd},
    permission::PBox,
    vector::PVec,
};
use std::cmp::Ordering;
struct Node<K, V> {
    keys: PVec<(K, V)>,
    children: PVec<PBox<Node<K, V>>>,
}
impl<K, V> Node<K, V> {
    fn new() -> Self {
        Self {
            keys: PVec::new(),
            children: PVec::new(),
        }
    }
    fn leaf(&self) -> bool {
        self.children.is_empty()
    }
    fn split(&mut self, i: usize) {
        let (middle, right) = {
            let c = &mut self.children[i];
            assert_eq!(c.keys.len(), 3);
            let mut right = Node::new();
            right.keys.push(c.keys.remove(2));
            let middle = c.keys.remove(1);
            if !c.leaf() {
                right.children.push(c.children.remove(2));
                right.children.push(c.children.remove(2));
            }
            (middle, PBox::new(right))
        };
        self.keys.insert(i, middle);
        self.children.insert(i + 1, right);
    }
    fn merge(&mut self, i: usize) {
        let separator = self.keys.remove(i);
        let mut right = self.children.remove(i + 1).into_inner();
        let left = &mut self.children[i];
        left.keys.push(separator);
        while !right.keys.is_empty() {
            left.keys.push(right.keys.remove(0));
        }
        while !right.children.is_empty() {
            left.children.push(right.children.remove(0));
        }
    }
    /// Ensure child has at least two keys before descending. Return its new index.
    fn fill(&mut self, i: usize) -> usize {
        if self.children[i].keys.len() >= 2 {
            return i;
        }
        if i > 0 && self.children[i - 1].keys.len() >= 2 {
            let replacement = self.children[i - 1].keys.pop().unwrap();
            let separator = std::mem::replace(&mut self.keys[i - 1], replacement);
            self.children[i].keys.insert(0, separator);
            if !self.children[i - 1].leaf() {
                let c = self.children[i - 1].children.pop().unwrap();
                self.children[i].children.insert(0, c);
            }
            i
        } else if i < self.keys.len() && self.children[i + 1].keys.len() >= 2 {
            let replacement = self.children[i + 1].keys.remove(0);
            let separator = std::mem::replace(&mut self.keys[i], replacement);
            self.children[i].keys.push(separator);
            if !self.children[i + 1].leaf() {
                let c = self.children[i + 1].children.remove(0);
                self.children[i].children.push(c);
            }
            i
        } else if i < self.keys.len() {
            self.merge(i);
            i
        } else {
            self.merge(i - 1);
            i - 1
        }
    }
    fn extreme(&mut self, last: bool) -> (K, V) {
        if self.leaf() {
            return if last {
                self.keys.pop().unwrap()
            } else {
                self.keys.remove(0)
            };
        }
        let i = if last { self.keys.len() } else { 0 };
        let i = self.fill(i);
        self.children[i].extreme(last)
    }
}
impl<K: TotalOrd, V> Node<K, V> {
    fn locate(&self, k: &K) -> usize {
        let mut i = 0;
        while i < self.keys.len() && self.keys[i].0.cmp_checked(k) == Ordering::Less {
            i += 1;
        }
        i
    }
    fn get(&self, k: &K) -> Option<&V> {
        let i = self.locate(k);
        if i < self.keys.len() && self.keys[i].0.cmp_checked(k) == Ordering::Equal {
            Some(&self.keys[i].1)
        } else if self.leaf() {
            None
        } else {
            Node::get(&self.children[i], k)
        }
    }
    fn insert(&mut self, k: K, v: V) -> Option<V> {
        let mut i = self.locate(&k);
        if i < self.keys.len() && self.keys[i].0.cmp_checked(&k) == Ordering::Equal {
            return Some(std::mem::replace(&mut self.keys[i].1, v));
        }
        if self.leaf() {
            self.keys.insert(i, (k, v));
            return None;
        }
        if self.children[i].keys.len() == 3 {
            self.split(i);
            match k.cmp_checked(&self.keys[i].0) {
                Ordering::Equal => return Some(std::mem::replace(&mut self.keys[i].1, v)),
                Ordering::Greater => i += 1,
                Ordering::Less => {}
            }
        }
        self.children[i].insert(k, v)
    }
    fn remove(&mut self, k: &K) -> Option<(K, V)> {
        let i = self.locate(k);
        if i < self.keys.len() && self.keys[i].0.cmp_checked(k) == Ordering::Equal {
            if self.leaf() {
                return Some(self.keys.remove(i));
            }
            if self.children[i].keys.len() >= 2 {
                let predecessor = self.children[i].extreme(true);
                return Some(std::mem::replace(&mut self.keys[i], predecessor));
            }
            if self.children[i + 1].keys.len() >= 2 {
                let successor = self.children[i + 1].extreme(false);
                return Some(std::mem::replace(&mut self.keys[i], successor));
            }
            self.merge(i);
            self.children[i].remove(k)
        } else if self.leaf() {
            None
        } else {
            let i = self.fill(i);
            self.children[i].remove(k)
        }
    }
    fn visit(&self, f: &mut impl FnMut(&K, &V)) {
        for i in 0..self.keys.len() {
            if !self.leaf() {
                self.children[i].visit(f);
            }
            f(&self.keys[i].0, &self.keys[i].1);
        }
        if !self.leaf() {
            self.children[self.keys.len()].visit(f);
        }
    }
    fn validate(
        &self,
        root: bool,
        lo: Option<&K>,
        hi: Option<&K>,
        depth: usize,
        leaf_depth: &mut Option<usize>,
    ) -> usize {
        assert!(self.keys.len() <= 3 && (root || !self.keys.is_empty()));
        for i in 0..self.keys.len() {
            let k = &self.keys[i].0;
            if let Some(x) = lo {
                assert_eq!(x.cmp_checked(k), Ordering::Less);
            }
            if let Some(x) = hi {
                assert_eq!(k.cmp_checked(x), Ordering::Less);
            }
            if i > 0 {
                assert_eq!(self.keys[i - 1].0.cmp_checked(k), Ordering::Less);
            }
        }
        if self.leaf() {
            if let Some(d) = leaf_depth {
                assert_eq!(*d, depth);
            } else {
                *leaf_depth = Some(depth);
            }
            return self.keys.len();
        }
        assert_eq!(self.children.len(), self.keys.len() + 1);
        let mut total = self.keys.len();
        for i in 0..self.children.len() {
            let low = if i == 0 {
                lo
            } else {
                Some(&self.keys[i - 1].0)
            };
            let high = if i == self.keys.len() {
                hi
            } else {
                Some(&self.keys[i].0)
            };
            total += self.children[i].validate(false, low, high, depth + 1, leaf_depth);
        }
        total
    }
}
pub struct PBTreeMap<K, V> {
    root: PBox<Node<K, V>>,
    len: usize,
}
impl<K: TotalOrd, V> Default for PBTreeMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<K: TotalOrd, V> PBTreeMap<K, V> {
    pub fn new() -> Self {
        Self {
            root: PBox::new(Node::new()),
            len: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn get(&self, k: &K) -> Option<&V> {
        Node::get(&self.root, k)
    }
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        if self.root.keys.len() == 3 {
            let old = std::mem::replace(&mut self.root, PBox::new(Node::new()));
            self.root.children.push(old);
            self.root.split(0);
        }
        let old = self.root.insert(k, v);
        if old.is_none() {
            self.len += 1;
        }
        old
    }
    pub fn remove(&mut self, k: &K) -> Option<V> {
        let result = self.root.remove(k);
        if self.root.keys.is_empty() && !self.root.leaf() {
            self.root = self.root.children.remove(0);
        }
        if result.is_some() {
            self.len -= 1;
        }
        result.map(|(_, v)| v)
    }
    pub fn validate(&self) {
        assert_eq!(self.root.validate(true, None, None, 0, &mut None), self.len);
    }
    pub fn visit(&self, mut f: impl FnMut(&K, &V)) {
        self.root.visit(&mut f);
    }
    pub fn into_entries(mut self) -> PVec<(K, V)> {
        let mut out = PVec::new();
        while !self.root.keys.is_empty() {
            out.push(self.root.extreme(false));
            self.len -= 1;
            if self.root.keys.is_empty() && !self.root.leaf() {
                self.root = self.root.children.remove(0);
            }
        }
        out
    }
}
impl<K: TotalOrd, V: Observe> Observe for PBTreeMap<K, V> {
    type Model = Map<K::Model, V::Model>;
    fn observe(&self) -> Self::Model {
        let mut m = Map::empty();
        self.visit(|k, v| {
            assert!(
                m.insert(k.observe(), v.observe()).is_none(),
                "key abstraction collapsed distinct keys"
            );
        });
        m
    }
}
