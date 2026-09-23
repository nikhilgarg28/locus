//! Opt-in wall-clock observations. No checker decision reads these samples.
use std::{cell::RefCell, time::Instant};

#[derive(Clone, Debug)]
pub struct Sample {
    pub stage: &'static str,
    pub nanos: u128,
}
thread_local! { static SAMPLES: RefCell<Option<Vec<Sample>>> = const { RefCell::new(None) }; }

pub struct Timer {
    stage: &'static str,
    started: Option<Instant>,
}
impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            let nanos = started.elapsed().as_nanos();
            SAMPLES.with(|samples| {
                if let Some(samples) = samples.borrow_mut().as_mut() {
                    samples.push(Sample {
                        stage: self.stage,
                        nanos,
                    });
                }
            });
        }
    }
}
pub fn start(stage: &'static str) -> Timer {
    Timer {
        stage,
        started: SAMPLES.with(|samples| samples.borrow().is_some().then(Instant::now)),
    }
}
pub fn checkpoint() -> Option<usize> {
    SAMPLES.with(|samples| samples.borrow().as_ref().map(Vec::len))
}
pub fn since(checkpoint: Option<usize>) -> Vec<Sample> {
    SAMPLES.with(|samples| match (checkpoint, samples.borrow().as_ref()) {
        (Some(at), Some(samples)) => samples[at..].to_vec(),
        _ => Vec::new(),
    })
}
struct Restore(Option<Vec<Sample>>);
impl Drop for Restore {
    fn drop(&mut self) {
        SAMPLES.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}
pub fn capture<R>(body: impl FnOnce() -> R) -> (R, Vec<Sample>) {
    let restore = Restore(SAMPLES.with(|slot| slot.replace(Some(Vec::new()))));
    let value = body();
    let samples = SAMPLES.with(|slot| slot.borrow_mut().take().unwrap_or_default());
    drop(restore);
    (value, samples)
}
