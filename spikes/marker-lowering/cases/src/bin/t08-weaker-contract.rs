//! expect: runtime=ok check=ok
//! prints: first(&Wide(150)) => 150
//! The draft rejects this: `Wide` promises `b < 200` where the trait says
//! `b < 128`. That is Locus's check, which this spike does not have. What is
//! shown here is the other half of the draft's sentence: rustc sees
//! `(u8, Erased)` on both sides and accepts.
use ::locus::locus;

locus! {
    trait OneByte {
        fn byte(&self) -> (b: u8, @(b < 128));
    }

    struct Wide(u8);

    impl OneByte for Wide {
        fn byte(&self) -> (b: u8, @(b < 200)) {
            if self.0 < 200 { (self.0, _) } else { (0, _) }
        }
    }

    fn ascii(b: u8, small: @(b < 128)) -> u8 { b }

    fn first<P: OneByte>(p: &P) -> u8 {
        let (b, small) = p.byte();
        ascii(b, small)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("first(&Wide(150)) => {:?}", first(&Wide(150)));
}
