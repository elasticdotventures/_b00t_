//! Minimal OODA (Observe-Orient-Decide-Act) decision framework.
//! Provides a lightweight loop struct for agent decision cycles.

/// Result of a single OODA cycle phase.
#[derive(Debug, Clone, PartialEq)]
pub enum PhaseResult {
    Continue,
    Repeat,
    Terminate(String),
}

/// One OODA cycle iteration.
#[derive(Debug, Clone)]
pub struct OodaIteration {
    pub phase: String,
    pub observation: String,
    pub orientation: String,
    pub decision: String,
    pub action: String,
    pub success: bool,
}

/// Minimal OODA loop executor.
pub struct OodaLoop {
    pub max_iterations: u32,
    iterations: Vec<OodaIteration>,
}

impl OodaLoop {
    pub fn new(max_iterations: u32) -> Self {
        Self {
            max_iterations,
            iterations: Vec::new(),
        }
    }

    /// Run the OODA cycle up to `max_iterations` times.
    /// Each call to `cycle` receives the current zero-based iteration index.
    /// Returns the last iteration produced, or a default iteration if zero runs.
    pub fn run<F>(&mut self, mut cycle: F) -> OodaIteration
    where
        F: FnMut(u32) -> OodaIteration,
    {
        let mut last = OodaIteration {
            phase: String::new(),
            observation: String::new(),
            orientation: String::new(),
            decision: String::new(),
            action: String::new(),
            success: true,
        };
        for i in 0..self.max_iterations {
            let iteration = cycle(i);
            self.iterations.push(iteration.clone());
            last = iteration;
        }
        last
    }

    pub fn iterations(&self) -> &[OodaIteration] {
        &self.iterations
    }

    /// Fraction of iterations where `success == true` (0.0 ..= 1.0).
    /// Returns 0.0 when there are no iterations.
    pub fn success_rate(&self) -> f64 {
        if self.iterations.is_empty() {
            return 0.0;
        }
        let ok = self.iterations.iter().filter(|i| i.success).count();
        ok as f64 / self.iterations.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_iter(i: u32, success: bool) -> OodaIteration {
        OodaIteration {
            phase: format!("phase-{}", i),
            observation: format!("obs-{}", i),
            orientation: format!("ori-{}", i),
            decision: format!("dec-{}", i),
            action: format!("act-{}", i),
            success,
        }
    }

    #[test]
    fn basic_cycle_execution() {
        let mut loop_ = OodaLoop::new(3);
        let last = loop_.run(|i| make_iter(i, true));
        assert_eq!(loop_.iterations().len(), 3);
        assert!(last.success);
        assert_eq!(last.phase, "phase-2");
    }

    #[test]
    fn max_iteration_enforcement() {
        let mut loop_ = OodaLoop::new(2);
        loop_.run(|i| make_iter(i, true));
        assert_eq!(loop_.iterations().len(), 2);
    }

    #[test]
    fn success_rate_calculation() {
        let mut loop_ = OodaLoop::new(5);
        loop_.run(|i| make_iter(i, i % 2 == 0)); // 3 successes, 2 failures
        assert!((loop_.success_rate() - 0.6).abs() < 1e-9);
    }

    #[test]
    fn zero_iterations() {
        let mut loop_ = OodaLoop::new(0);
        let last = loop_.run(|_| make_iter(0, true));
        assert!(loop_.iterations().is_empty());
        assert_eq!(loop_.success_rate(), 0.0);
        // last should be the default (empty fields, success=true)
        assert!(last.success);
        assert!(last.phase.is_empty());
    }

    #[test]
    fn phase_result_variants() {
        assert_eq!(PhaseResult::Continue, PhaseResult::Continue);
        assert_eq!(PhaseResult::Repeat, PhaseResult::Repeat);
        match PhaseResult::Terminate("done".into()) {
            PhaseResult::Terminate(msg) => assert_eq!(msg, "done"),
            _ => panic!("expected Terminate"),
        }
    }
}
