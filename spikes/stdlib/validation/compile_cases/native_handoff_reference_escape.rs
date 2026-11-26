use locus_stdlib_spike::wrappers::WVec; fn main(){let mut w=WVec::from_std(vec![1_u32]);let r=w.with_native(|v| &mut v[0]);*r=2;}
