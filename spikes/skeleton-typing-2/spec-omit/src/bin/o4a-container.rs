//! expect: runtime=reject check=reject
//! OMISSION in s4-container.rs: `model(..)` left out at an argument whose parameter's type is a type parameter.
//! Specification 4: a type with an invariant, a generic container of it, a
//! logic function that is generic over the element type, and a
//! specification trait that is implemented for logical types and used
//! through a bound.
use ::locus::locus;

locus! {
    /// A specification trait: a logical value has a weight.
    trait Weighed: Logical {
        logic fn weight(self) -> Nat;
        /// The weight is positive unless the value is `light`.
        logic fn light(self) -> Prop;
        logic fn light_or_positive(x: Self) -> @(x.light() || x.weight() > 0);
    }

    impl Weighed for Nat {
        logic fn weight(self) -> Nat { self }
        logic fn light(self) -> Prop { self == 0 }
        logic fn light_or_positive(x: Nat) -> @(x.light() || x.weight() > 0) { _ }
    }

    /// A logical type of the program, with the trait too.
    #[derive(Logical)]
    struct Parcel { grams: Nat, fragile: Bool }

    impl Weighed for Parcel {
        logic fn weight(self) -> Nat { if self.fragile { self.grams * 2 } else { self.grams } }
        logic fn light(self) -> Prop { self.grams == 0 }
        logic fn light_or_positive(x: Parcel) -> @(x.light() || x.weight() > 0) { _ }
    }

    /// Generic over the element type: the weight of a whole sequence.
    logic fn total<W: Weighed>(xs: Seq<W>) -> Nat {
        if xs.len() == 0 { 0 as Nat } else { xs[0].weight() + total(xs.skip(1 as Nat)) }
    }

    logic fn total_of_pushed<W: Weighed>(xs: Seq<W>, x: W) -> @(total(xs.push(x)) == total(xs) + x.weight()) { _ }

    logic fn heavier<W: Weighed>(a: W, b: W) -> Prop { a.weight() >= b.weight() }

    /// A type with an invariant: an even number.
    pub struct Even {
        n: u32,
        even: @(n % 2 == 0),
    }

    impl Model for Even {
        type Logic = Nat;
        logic fn model(e: &Even) -> Nat { e.n }
    }

    /// A generic container with an invariant of its own: it is never over
    /// its limit. The length needs no model of `T`, only its snapshot.
    pub struct Shelf<T> {
        items: Vec<T>,
        limit: usize,
        fits: @(snap(items).len() <= limit),
    }

    impl<T> Shelf<T> {
        pub fn new(limit: usize) -> (s: Shelf<T>, @(snap(s.items).len() == 0)) {
            (Shelf { items: Vec::new(), limit, fits: _ }, _)
        }

        pub fn put(&mut self, x: T, room: @(snap(self.items).len() < self.limit))
            -> @(snap(final!(self).items).len() == snap(self.items).len() + 1)
        {
            self.items.push(x);
            _
        }
    }

    /// Used through a bound: the elements' models are weighed.
    pub fn put_weighed<T: Model>(shelf: &mut Shelf<T>, x: T, room: @(snap(shelf.items).len() < shelf.limit))
        -> @(total(final!(shelf).items) == total(shelf.items) + x.weight()
            && heavier(final!(shelf).items.last(), model(x)))
        where T::Logic: Weighed
    {
        let before = logic! { model(shelf.items) };
        let added = logic! { total_of_pushed(before, x) };
        let _ = shelf.put(x, room);
        _
    }

    pub fn weigh(shelf: &Shelf<Even>) -> (w: u32, @(w == total(shelf.items))) {
        let mut w = 0;
        let mut so_far: @(w == total(shelf.items.take(0 as Nat))) = _;
        for e in &shelf.items {
            w += e.n;
            so_far = _;
        }
        (w, _)
    }

    fn run() -> (usize, u32) {
        let (mut shelf, _) = Shelf::new(3);
        let _ = put_weighed(&mut shelf, Even { n: 2, even: _ }, _);
        let _ = put_weighed(&mut shelf, Even { n: 4, even: _ }, _);
        let (w, _) = weigh(&shelf);
        (shelf.items.len(), w)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("run() => {:?}", run());
}
