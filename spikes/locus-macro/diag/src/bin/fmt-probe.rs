//! Question 4: misformatted on purpose, the same way inside and outside `locus!`.
use ::locus::locus;

locus! {
    fn   halve( n:u32,even : @(n%2==0) )->u32{ n/2 }
    fn plain_inside( x:u32 )->u32{x+1}
}

fn   plain_outside( x:u32 )->u32{x+1}

fn main() {
    let n = halve(4, ::locus::Erased) + plain_inside(1) + plain_outside(1);
    println!("{n}");
}
