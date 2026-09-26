use super::{Error, Loaded};
use crate::elab::{self, Elaborated, Options};
use std::path::Path;
use std::sync::Arc;

pub struct Checked {
    pub loaded: Loaded,
    pub checked: Elaborated,
}

pub fn check(path: &Path, options: &Options) -> Result<Checked, Error> {
    check_loaded(super::load(path)?, options)
}

pub(super) fn check_loaded(loaded: Loaded, options: &Options) -> Result<Checked, Error> {
    let mut options = options.clone();
    if let Some(target) = &loaded.target {
        options.pointer_width = target.pointer_width;
    }
    options.module_access = Some(Arc::new(loaded.graph.access.clone()));
    let checked = elab::elaborate_with_options(
        loaded.sources.get(loaded.bundle.file),
        &loaded.program,
        &options,
    );
    if !checked.is_success() {
        let diagnostics = checked
            .diagnostics
            .iter()
            .map(|d| loaded.diagnostic(d))
            .collect();
        return Err(Error {
            sources: loaded.sources,
            diagnostics,
        });
    }
    Ok(Checked { loaded, checked })
}
