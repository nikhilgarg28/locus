//! expect: runtime=ok check=ok
//! prints: run() => (3, 2, 3, false)
//! Specification 1: a stack over a `Vec<u32>` with a hand-written model,
//! `Seq<Nat>`, and contracts on `push`, `pop` and `top`. Round 3: Locus's own
//! `Vec`, and logic functions over logical types only.
use ::locus::locus;

locus! {
    use ::locus::Vec;
    /// At most `cap` numbers, the last pushed on top.
    pub struct Stack {
        items: Vec<u32>,
        cap: usize,
        bounded: @(items.len() <= cap),
    }

    impl Model for Stack {
        type Logic = Seq<Nat>;
        logic fn model(stack: Snap<Stack>) -> Seq<Nat> { stack.items.elements() }
    }

    impl Stack {
        /// How many more fit. Over the snapshot, which has the capacity:
        /// called by its path, on `snap(s)`.
        logic fn room(s: Snap<Stack>) -> Int { s.cap - s.items.elements().len() }
    }

    /// What `push` does to the model.
    logic fn pushed(before: Seq<Nat>, x: Nat) -> Seq<Nat> { before.push(x) }

    logic fn pushed_is_longer(xs: Seq<Nat>, x: Nat) -> @(pushed(xs, x).len() == xs.len() + 1) { _ }

    logic fn pushed_then_last(xs: Seq<Nat>, x: Nat) -> @(pushed(xs, x).last() == x) { _ }

    impl Stack {
        pub fn new(cap: usize) -> (s: Stack, @(s.len() == 0 && Stack::room(snap(s)) == cap as Int)) {
            (Stack { items: Vec::new(), cap, bounded: _ }, _)
        }

        pub fn len(&self) -> (n: usize, @(n == self.len())) {
            (self.items.len().0, _)
        }

        pub fn is_empty(&self) -> (r: bool, @(r <==> self.len() == 0)) {
            (self.items.is_empty(), _)
        }

        pub fn push(&mut self, x: u32, fits: @(Stack::room(snap(self)) > 0))
            -> @(final!(self) === pushed(*self, x) && Stack::room(snap(final!(self))) == Stack::room(snap(self)) - 1)
        {
            let before = logic! { *self };
            let longer = logic! { pushed_is_longer(before, x) };
            self.items.push(x);
            _
        }

        pub fn pop(&mut self, nonempty: @(self.len() > 0))
            -> (x: u32, @(final!(self).push(x) === *self && final!(self).len() + 1 == self.len()))
        {
            let top = self.items.pop().0.unwrap();
            (top, _)
        }

        pub fn top(&self, nonempty: @(self.len() > 0)) -> (x: u32, @(x == self.last())) {
            (self.items[self.items.len().0 - 1], _)
        }

        /// Pushing and then looking gives back what was pushed.
        pub fn push_then_top(&mut self, x: u32, fits: @(Stack::room(snap(self)) > 0)) -> (y: u32, @(y == x)) {
            let pushed_it = self.push(x, fits);
            let last = logic! { pushed_then_last(*self, x) };
            let (y, is_last) = self.top(_);
            (y, _)
        }
    }

    fn run() -> (usize, u32, u32, bool) {
        let (mut s, fresh) = Stack::new(4);
        let _ = s.push(1, _);
        let _ = s.push(2, _);
        let (three, _) = s.push_then_top(3, _);
        let (n, _) = s.len();
        let (popped, _) = s.pop(_);
        let (two, _) = s.top(_);
        let (empty, _) = s.is_empty();
        (n, two, popped, empty)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
