//! expect: runtime=ok check=ok
//! prints: first(&Letter(65)) => 65
//! prints: first(&Letter(200)) => 0
//! A proof in the signature of a trait's method, and generic code that uses
//! it: the draft's `OneByte`.
use ::locus::locus;

locus! {
    trait OneByte {
        /// Returns an ASCII byte.
        fn byte(&self) -> (b: u8, @(b < 128));

        // A method with a default body, and a proof in it.
        fn low(&self) -> (b: u8, @(b < 128)) {
            let (b, small) = self.byte();
            (b, small)
        }
    }

    struct Letter(u8);

    impl OneByte for Letter {
        fn byte(&self) -> (b: u8, @(b < 128)) {
            if self.0 < 128 { (self.0, _) } else { (0, _) }
        }
    }

    fn ascii(b: u8, small: @(b < 128)) -> u8 { b }

    fn first<P: OneByte>(p: &P) -> u8 {
        let (b, small) = p.low();
        ascii(b, small)
    }
}

fn main() {
    println!("expansion: {}", ::locus::__expansion!());
    println!("first(&Letter(65)) => {:?}", first(&Letter(65)));
    println!("first(&Letter(200)) => {:?}", first(&Letter(200)));
}
