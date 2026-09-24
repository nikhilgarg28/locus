mod local {
    include!(concat!(env!("OUT_DIR"), "/local.rs"));
}
mod auxiliary {
    include!(concat!(env!("OUT_DIR"), "/auxiliary.rs"));
}
fn main() {
    assert_eq!(auxiliary::first::value() + auxiliary::second::value(), 13);
    let original: verified::checked::Token = verified::checked::Token::new(7);
    let returned: verified::checked::Token = local::round_trip(original);
    assert_eq!(returned.value(), 7);
    assert_eq!(local::answer(), 42);
}
