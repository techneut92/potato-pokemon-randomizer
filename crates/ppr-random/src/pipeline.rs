use crate::{CancelToken, Progress, ProgressSink, Seed, StepRng};

/// Why a run stopped.
#[derive(Debug, thiserror::Error)]
pub enum RandomizeError {
    /// The user cancelled the run. Nothing was written.
    #[error("randomization was cancelled")]
    Cancelled,
    /// A step could not satisfy the settings, e.g. too few species left after the restrictions.
    #[error("{step}: {message}")]
    Impossible {
        /// Label of the failing step.
        step: &'static str,
        /// What went wrong, for the user.
        message: String,
    },
}

/// What a step gets to work with.
pub struct StepContext<'a, C> {
    /// The data the run transforms, e.g. game data plus settings.
    pub data: &'a mut C,
    /// This step's own random stream (ADR 0005).
    pub rng: StepRng,
    /// Long steps check this inside their loops.
    pub cancel: &'a CancelToken,
}

/// One randomization step (Strategy pattern).
pub trait Step<C> {
    /// Stable ID. It selects the RNG stream, so it must never change once released.
    fn id(&self) -> u64;
    /// Label shown in the progress dialog.
    fn label(&self) -> &'static str;
    /// False when the settings or the ROM make this step a no-op.
    fn applies(&self, data: &C) -> bool;
    /// Transforms the data.
    fn run(&self, ctx: StepContext<'_, C>) -> Result<(), RandomizeError>;
}

/// Runs steps in a fixed order (Pipeline pattern).
pub struct Pipeline<C> {
    steps: Vec<Box<dyn Step<C>>>,
}

impl<C> Default for Pipeline<C> {
    fn default() -> Self {
        Self { steps: Vec::new() }
    }
}

impl<C> Pipeline<C> {
    /// Appends a step.
    #[must_use]
    pub fn with(mut self, step: impl Step<C> + 'static) -> Self {
        self.steps.push(Box::new(step));
        self
    }

    /// Labels of all steps, in order, as reported in [`Progress::step`].
    pub fn labels(&self) -> Vec<&'static str> {
        self.steps.iter().map(|s| s.label()).collect()
    }

    /// Runs every applicable step. Stops before the next step once `cancel` is set.
    pub fn run(
        &self,
        data: &mut C,
        seed: Seed,
        cancel: &CancelToken,
        progress: &mut dyn ProgressSink,
    ) -> Result<(), RandomizeError> {
        let active: Vec<_> = self.steps.iter().filter(|s| s.applies(data)).collect();
        let total = active.len().max(1);
        for (i, step) in active.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(RandomizeError::Cancelled);
            }
            progress.report(Progress {
                step: step.label().into(),
                percent: percent(i, total),
            });
            step.run(StepContext {
                data: &mut *data,
                rng: seed.rng_for(step.id()),
                cancel,
            })?;
        }
        if cancel.is_cancelled() {
            return Err(RandomizeError::Cancelled);
        }
        progress.report(Progress {
            step: DONE.into(),
            percent: 100,
        });
        Ok(())
    }
}

/// [`Progress::step`] once every step has run.
pub const DONE: &str = "Done";

fn percent(done: usize, total: usize) -> u8 {
    u8::try_from(done * 100 / total).unwrap_or(100)
}

#[cfg(test)]
mod tests {
    use rand_core::Rng;

    use super::*;

    struct Push(u64, bool);

    impl Step<Vec<u32>> for Push {
        fn id(&self) -> u64 {
            self.0
        }
        fn label(&self) -> &'static str {
            "push"
        }
        fn applies(&self, _: &Vec<u32>) -> bool {
            self.1
        }
        fn run(&self, mut ctx: StepContext<'_, Vec<u32>>) -> Result<(), RandomizeError> {
            ctx.data.push(ctx.rng.next_u32());
            Ok(())
        }
    }

    fn run(p: &Pipeline<Vec<u32>>, seed: u64) -> (Vec<u32>, Vec<u8>) {
        let mut data = Vec::new();
        let mut percents = Vec::new();
        p.run(&mut data, Seed(seed), &CancelToken::default(), &mut |pr: Progress| {
            percents.push(pr.percent);
        })
        .expect("runs");
        (data, percents)
    }

    #[test]
    fn deterministic_and_reports_progress() {
        let p = Pipeline::default()
            .with(Push(1, true))
            .with(Push(2, false))
            .with(Push(3, true));
        let (a, progress) = run(&p, 7);
        assert_eq!(a.len(), 2, "the step that does not apply is skipped");
        assert_eq!(a, run(&p, 7).0);
        assert_eq!(progress, [0, 50, 100]);
    }

    #[test]
    fn adding_a_step_does_not_shift_others() {
        let without = run(&Pipeline::default().with(Push(3, true)), 7).0;
        let with = run(&Pipeline::default().with(Push(1, true)).with(Push(3, true)), 7).0;
        assert_eq!(without[0], with[1]);
    }

    #[test]
    fn cancelled_runs_stop() {
        let cancel = CancelToken::default();
        cancel.cancel();
        let err = Pipeline::default()
            .with(Push(1, true))
            .run(&mut Vec::new(), Seed(1), &cancel, &mut |_: Progress| {});
        assert!(matches!(err, Err(RandomizeError::Cancelled)));
    }
}
