// Compiled as a standalone host tool. This wrapper observes Cargo's actual
// rustc invocations and forwards them unchanged. It never enables unstable
// language features. Environment values stay in private temporary files;
// published interface receipts contain configuration hashes instead.
use std::{
    env, fs,
    io::Write,
    path::PathBuf,
    process::{Command, ExitCode},
};
fn strings(out: &mut impl Write, values: &[String]) -> std::io::Result<()> {
    out.write_all(&(values.len() as u32).to_le_bytes())?;
    for value in values {
        out.write_all(&(value.len() as u32).to_le_bytes())?;
        out.write_all(value.as_bytes())?;
    }
    Ok(())
}
fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args_os()
        .skip(1)
        .map(|s| {
            s.into_string()
                .map_err(|_| "Rust import capture requires UTF-8 tool arguments")
        })
        .collect::<Result<_, _>>()?;
    let compiler = args
        .first()
        .ok_or("Cargo did not supply its rustc executable")?;
    // Cargo probes rustc before compiling. Retain actual crate invocations.
    let capture = if args.iter().any(|a| a == "--crate-name") {
        let root =
            PathBuf::from(env::var_os("LOCUS_RUSTC_CAPTURE").ok_or("missing capture directory")?);
        let path = root.join(std::process::id().to_string());
        fs::create_dir(&path)?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut out = options.open(path.join("invocation.bin"))?;
        strings(&mut out, &args)?;
        strings(
            &mut out,
            &[env::current_dir()?
                .into_os_string()
                .into_string()
                .map_err(|_| "non-UTF-8 working directory")?],
        )?;
        let mut selected: Vec<_> = env::vars_os()
            .map(|(k, v)| {
                Ok((
                    k.into_string()
                        .map_err(|_| "non-UTF-8 environment name in compiler capture")?,
                    v.into_string()
                        .map_err(|_| "non-UTF-8 environment value in compiler capture")?,
                ))
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        selected.sort();
        strings(
            &mut out,
            &selected
                .into_iter()
                .flat_map(|(k, v)| [k, v])
                .collect::<Vec<_>>(),
        )?;
        Some(path)
    } else {
        None
    };
    let status = Command::new(compiler).args(&args[1..]).status()?;
    if status.success() {
        if let Some(path) = capture {
            fs::write(path.join("complete"), b"1")?;
        }
    }
    Ok(status.code().unwrap_or(1))
}
fn main() -> ExitCode {
    match run() {
        Ok(0) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("Locus compiler capture failed: {e}");
            ExitCode::FAILURE
        }
    }
}
