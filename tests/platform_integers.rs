use locus::{
    elab,
    erased::{Outcome, Value},
    kernel::{MachineInt, PointerWidth},
    project,
};
use std::path::PathBuf;
fn check(tag: &str, width: PointerWidth, code: &str) -> Result<project::Checked, String> {
    let dir =
        PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("platform_{tag}_{}", width.bits()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("export.lc");
    std::fs::write(&file, code).unwrap();
    project::check(
        &file,
        &elab::Options {
            pointer_width: width,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())
}
#[test]
#[doc = "spec: 1.96:1"]
#[doc = "spec: 1.26:1"]
fn widths_have_distinct_types_and_checked_arithmetic() {
    for width in [PointerWidth::W32, PointerWidth::W64] {
        let code = "pub fn demo()->usize{let values:Vec<u8>=Vec::from([1,2,3]);let n:usize=values.len();let last:usize=n-1;let value=values[last];(value as usize)+n}";
        let checked = check("indices", width, code).unwrap();
        let f = checked
            .checked
            .functions
            .iter()
            .find(|(n, _)| n.ends_with("_demo"))
            .unwrap()
            .1;
        let a = locus::exec::CheckInterpreter::new(checked.checked.session.program(), 10000)
            .with_lending(checked.checked.session.lending())
            .call(f, vec![])
            .unwrap();
        let b = locus::erased::Interpreter::new(checked.checked.session.erased(), 10000)
            .call(f, vec![])
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a, Outcome::Value(Value::Int(width.usize(), 6)));
        assert!(check("distinct", width, "fn bad(n:u64)->usize{n}").is_err());
        assert!(check("old_index", width, "fn bad(xs:&[u8],i:u64)->u8{xs[i]}").is_err());
        assert!(
            check(
                "no_underflow",
                width,
                "#[no_panic] fn bad(n:usize)->usize{n-1}"
            )
            .is_err()
        );
        check(
            "proven_dec",
            width,
            "#[no_panic] fn dec(n:usize,p:@(n>0))->usize{n-1}",
        )
        .unwrap();
    }
}
#[test]
#[doc = "spec: 1.96:1"]
#[doc = "spec: 2.12:1"]
fn target_sensitive_claims_do_not_become_portable() {
    let source = "fn upper(n:usize)->@(n<=4294967295){_}";
    check("bound32", PointerWidth::W32, source).unwrap();
    assert!(check("bound64", PointerWidth::W64, source).is_err());
    for width in [PointerWidth::W32, PointerWidth::W64] {
        check("bounds",width,"fn bounds(n:usize,s:isize)->(@(n<=usize::MAX),@(s>=isize::MIN),@(s<=isize::MAX)){(_,_,_)}").unwrap();
        check("literals", width, "fn f()->(usize,isize){(7usize,-3isize)}").unwrap();
    }
    assert_ne!(MachineInt::Usize32, MachineInt::U32);
    assert_ne!(MachineInt::Usize64, MachineInt::U64);
    assert_ne!(
        MachineInt::Usize32.kernel_name(),
        MachineInt::Usize64.kernel_name()
    );
}
#[test]
fn pointer_casts_and_wrapping_match_fixed_width_rust_oracles() {
    for ty in [
        MachineInt::Usize32,
        MachineInt::Usize64,
        MachineInt::Isize32,
        MachineInt::Isize64,
    ] {
        for n in [
            -18446744073709551617i128,
            -4294967297,
            -2147483649,
            -1,
            0,
            1,
            2147483648,
            4294967296,
            18446744073709551616,
        ] {
            let expected = match ty {
                MachineInt::Usize32 => n as u32 as i128,
                MachineInt::Usize64 => n as u64 as i128,
                MachineInt::Isize32 => n as i32 as i128,
                MachineInt::Isize64 => n as i64 as i128,
                _ => unreachable!(),
            };
            assert_eq!(ty.wrap(&n.into()), expected.into());
        }
    }
}

#[test]
#[doc = "spec: 1.96:10"]
#[doc = "spec: 3.10:1"]
fn target_identity_is_checked_by_kernel_erasure_and_proof_store() {
    use locus::kernel::{Context, Definitions, Mode, Term, infer_term};
    use locus::store::{Names, text::print_key};
    let mut keys = Vec::new();
    for width in [PointerWidth::W32, PointerWidth::W64] {
        let mut ctx =
            Context::with_definitions(std::rc::Rc::new(Definitions::with_pointer_width(width)));
        let wrong = if width == PointerWidth::W32 {
            MachineInt::Usize64
        } else {
            MachineInt::Usize32
        };
        assert!(
            infer_term(
                &mut ctx,
                &Term::Machine(wrong, 1i64.into()),
                Mode::Executable
            )
            .is_err()
        );
        keys.push(print_key(&Term::Bool(true), &ctx, &Names::new()).unwrap());
        let proof = locus::kernel::Proof::Refl(Term::Machine(width.usize(), 3i64.into()));
        let spelling = locus::store::text::print_proof(&proof, &ctx, &Names::new()).unwrap();
        assert!(spelling.contains(width.usize().kernel_name()));
        let replay = locus::store::text::parse_proof(&spelling, &ctx, &Names::new()).unwrap();
        let goal = Term::eq(
            locus::kernel::Type::machine(width.usize()),
            Term::Machine(width.usize(), 3i64.into()),
            Term::Machine(width.usize(), 3i64.into()),
        );
        locus::kernel::check_proof(&mut ctx, &replay, &goal).unwrap();
        let other_goal = Term::eq(
            locus::kernel::Type::machine(wrong),
            Term::Machine(wrong, 3i64.into()),
            Term::Machine(wrong, 3i64.into()),
        );
        assert!(locus::kernel::check_proof(&mut ctx, &replay, &other_goal).is_err());

        let checked = check("tampered_width", width, "pub fn f(n:usize)->usize{n}").unwrap();
        // Keep the body and signature consistent with each other, but inconsistent with the target.
        let module = checked.checked.session.erased().clone();
        let mut bad = module.clone();
        bad.pointer_width = if width == PointerWidth::W32 {
            PointerWidth::W64
        } else {
            PointerWidth::W32
        };
        assert!(locus::erased::check_module(&bad).is_err());
        assert!(locus::erased::check_module(&module).is_ok());
    }
    assert_ne!(keys[0], keys[1]);
    assert!(
        check(
            "large_array",
            PointerWidth::W32,
            "fn f(xs:&[u8;4294967296]){}"
        )
        .err()
        .unwrap()
        .contains("target usize")
    );
}
#[test]
#[doc = "spec: 1.96:10"]
fn emitted_rust_preserves_pointer_overflow_in_both_modes() {
    let code = "pub fn add(n:usize)->usize{n+1}pub fn sub(n:usize)->usize{n-1}pub fn neg(n:isize)->isize{-n}pub fn div(a:isize,b:isize)->isize{a/b}pub fn rem(a:isize,b:isize)->isize{a%b}pub fn wrap(n:usize)->usize{n.wrapping_add(1)}pub fn cast(n:isize)->usize{n as usize}";
    let checked = check("rust_modes", PointerWidth::HOST, code).unwrap();
    let rust = project::rust(checked).unwrap();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("platform_rust_modes");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("main.rs"),
        format!(
            r#"{rust}
fn main() {{
 std::panic::set_hook(Box::new(|_|{{}}));
 assert!(std::panic::catch_unwind(||add(usize::MAX)).is_err());
 assert!(std::panic::catch_unwind(||sub(0)).is_err());
 assert!(std::panic::catch_unwind(||neg(isize::MIN)).is_err());
 assert!(std::panic::catch_unwind(||div(isize::MIN,-1)).is_err());
 assert!(std::panic::catch_unwind(||rem(isize::MIN,-1)).is_err());
 assert!(std::panic::catch_unwind(||div(3,0)).is_err());
 assert_eq!(add(5),6); assert_eq!(sub(5),4); assert_eq!(neg(5),-5);
 assert_eq!(wrap(usize::MAX),0);assert_eq!(cast(-1),usize::MAX);
}}
"#
        ),
    )
    .unwrap();
    for mode in ["yes", "no"] {
        let out = std::process::Command::new("rustc")
            .args([
                "--edition=2024",
                "-Dwarnings",
                "-C",
                &format!("overflow-checks={mode}"),
            ])
            .arg(dir.join("main.rs"))
            .arg("-o")
            .arg(dir.join("run"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            std::process::Command::new(dir.join("run"))
                .status()
                .unwrap()
                .success()
        );
    }
    let other = if PointerWidth::HOST == PointerWidth::W64 {
        PointerWidth::W32
    } else {
        PointerWidth::W64
    };
    let checked = check(
        "guard",
        other,
        "fn proof(n:usize)->@(n<=usize::MAX){_}pub fn data()->u8{3}",
    )
    .unwrap();
    std::fs::write(dir.join("wrong.rs"), project::rust(checked).unwrap()).unwrap();
    let out = std::process::Command::new("rustc")
        .args(["--edition=2024", "--crate-type=lib"])
        .arg(dir.join("wrong.rs"))
        .arg("--out-dir")
        .arg(&dir)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("Locus checked this module"));
}
#[test]
#[doc = "spec: 1.96:9"]
fn rustc_layout_query_and_cargo_target_configuration_need_no_target_stdlib() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("platform_target_config");
    std::fs::create_dir_all(dir.join(".cargo")).unwrap();
    std::fs::write(dir.join("Cargo.toml"),"[package]\nname=\"platform_fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[lib]\npath=\"lib.rs\"\n[workspace]\n").unwrap();
    std::fs::write(dir.join("lib.rs"), "").unwrap();
    std::fs::write(
        dir.join(".cargo/config.toml"),
        "[build]\ntarget=\"i686-unknown-linux-gnu\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("export.lc"), "pub fn len()->usize{3}").unwrap();
    // A bare filename has an empty Path::parent(), which means the current
    // directory rather than a missing directory. Exercise the real CLI path.
    let relative = std::process::Command::new(env!("CARGO_BIN_EXE_locus"))
        .current_dir(&dir)
        .args([
            "check",
            "export.lc",
            "--target",
            "i686-unknown-linux-gnu",
            "--offline",
            "--no-store",
        ])
        .output()
        .unwrap();
    assert!(
        relative.status.success(),
        "{}",
        String::from_utf8_lossy(&relative.stderr)
    );
    let layout = locus::target::TargetLayout::discover(&dir, None).unwrap();
    assert_eq!(layout.pointer_width, PointerWidth::W32);
    assert!(layout.inputs.contains_key(&dir.join(".cargo/config.toml")));
    let layout =
        locus::target::TargetLayout::discover(&dir, Some("x86_64-unknown-linux-gnu")).unwrap();
    assert_eq!(layout.pointer_width, PointerWidth::W64);
    let mut build = project::Build::new(dir.join("export.lc")).out_dir(dir.join("out"));
    build.use_proofs = false;
    let built = build.generate().unwrap();
    let receipt = std::fs::read_to_string(&built.receipt).unwrap();
    assert!(receipt.contains("i686-unknown-linux-gnu"));
    assert!(build.is_current().unwrap());
    std::fs::write(
        dir.join(".cargo/config.toml"),
        "[build]\ntarget=\"x86_64-unknown-linux-gnu\"\n",
    )
    .unwrap();
    assert!(!build.is_current().unwrap());
    std::fs::write(
        dir.join(".cargo/config.toml"),
        "[build]\ntarget=[\"i686-unknown-linux-gnu\",\"x86_64-unknown-linux-gnu\"]\n",
    )
    .unwrap();
    assert!(
        locus::target::TargetLayout::discover(&dir, None)
            .unwrap_err()
            .contains("select one explicitly")
    );
    assert!(locus::target::TargetLayout::discover(&dir, Some("x86_64-unknown-linux-gnu")).is_ok());
    assert!(
        locus::target::TargetLayout::discover(&dir, Some("not-a-real-target"))
            .unwrap_err()
            .contains("query target layout")
    );
}

#[test]
#[doc = "spec: 2.35:3"]
fn buffer_bounds_are_target_specific_even_without_machine_terms() {
    use locus::kernel::{Context, Definitions, Proof, Term, Type, check_proof};
    for width in [PointerWidth::W32, PointerWidth::W64] {
        let mut ctx =
            Context::with_definitions(std::rc::Rc::new(Definitions::with_pointer_width(width)));
        let value = Term::Free(ctx.declare(Type::Buffer(Box::new(Type::U8))).unwrap());
        let len = locus::kernel::buffer::length(Type::U8, value.clone());
        let evidence = Proof::BufferBound {
            value: value.clone(),
            upper: true,
        };
        check_proof(
            &mut ctx,
            &evidence,
            &Term::int_le(len.clone(), Term::Int(width.usize().max())),
        )
        .unwrap();
        let other = if width == PointerWidth::W32 {
            PointerWidth::W64
        } else {
            PointerWidth::W32
        };
        assert!(
            check_proof(
                &mut ctx,
                &evidence,
                &Term::int_le(len, Term::Int(other.usize().max()))
            )
            .is_err()
        );
    }
}
#[test]
fn arithmetic_at_each_pointer_width_matches_independent_checked_rust_operations() {
    let code = "fn add(a:usize,b:usize)->usize{a+b}fn sub(a:usize,b:usize)->usize{a-b}fn mul(a:usize,b:usize)->usize{a*b}fn div(a:isize,b:isize)->isize{a/b}fn rem(a:isize,b:isize)->isize{a%b}";
    for width in [PointerWidth::W32, PointerWidth::W64] {
        let checked = check("boundary_operations", width, code).unwrap();
        for name in ["add", "sub", "mul", "div", "rem"] {
            let ty = if matches!(name, "div" | "rem") {
                width.isize()
            } else {
                width.usize()
            };
            let mut bounds = vec![
                ty.min().to_i128().unwrap(),
                ty.min().to_i128().unwrap() + 1,
                0,
                1,
                2,
                ty.max().to_i128().unwrap() - 1,
                ty.max().to_i128().unwrap(),
            ];
            if ty.signed() {
                bounds.push(-1);
            }
            let f = checked
                .checked
                .functions
                .iter()
                .find(|(n, _)| n.ends_with(&format!("_{name}")))
                .unwrap()
                .1;
            for &a in &bounds {
                for &b in &bounds {
                    let expected = match (width, name) {
                        (PointerWidth::W32, "add") => {
                            (a as u32).checked_add(b as u32).map(i128::from)
                        }
                        (PointerWidth::W64, "add") => {
                            (a as u64).checked_add(b as u64).map(i128::from)
                        }
                        (PointerWidth::W32, "sub") => {
                            (a as u32).checked_sub(b as u32).map(i128::from)
                        }
                        (PointerWidth::W64, "sub") => {
                            (a as u64).checked_sub(b as u64).map(i128::from)
                        }
                        (PointerWidth::W32, "mul") => {
                            (a as u32).checked_mul(b as u32).map(i128::from)
                        }
                        (PointerWidth::W64, "mul") => {
                            (a as u64).checked_mul(b as u64).map(i128::from)
                        }
                        (PointerWidth::W32, "div") => {
                            (a as i32).checked_div(b as i32).map(i128::from)
                        }
                        (PointerWidth::W64, "div") => {
                            (a as i64).checked_div(b as i64).map(i128::from)
                        }
                        (PointerWidth::W32, "rem") => {
                            (a as i32).checked_rem(b as i32).map(i128::from)
                        }
                        (PointerWidth::W64, "rem") => {
                            (a as i64).checked_rem(b as i64).map(i128::from)
                        }
                        _ => unreachable!(),
                    };
                    let args = vec![Value::Int(ty, a), Value::Int(ty, b)];
                    let x = locus::exec::CheckInterpreter::new(
                        checked.checked.session.program(),
                        10000,
                    )
                    .call(f, args.clone())
                    .unwrap();
                    let y =
                        locus::erased::Interpreter::new(checked.checked.session.erased(), 10000)
                            .call(f, args)
                            .unwrap();
                    assert_eq!(x, y, "{width:?} {name}({a},{b})");
                    match expected {
                        Some(n) => assert_eq!(x, Outcome::Value(Value::Int(ty, n))),
                        None => assert!(matches!(x, Outcome::Panic(_))),
                    }
                }
            }
        }
    }
}
