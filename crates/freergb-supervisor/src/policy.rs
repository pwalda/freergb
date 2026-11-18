// SPDX-License-Identifier: GPL-2.0-or-later
//! Restart policy for an owned server: exponential backoff, reset after a healthy run, and a
//! crash-loop limit. Pure (no clock, no sleeping) so it is unit-tested directly.

use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RestartPolicy {
    pub base_delay: Duration,
    pub max_delay: Duration,
    /// An owned server that stayed connected this long counts as healthy: the backoff
    /// and the crash count start over.
    pub healthy_after: Duration,
    /// This many failed runs in a row (none of them healthy) end in `Failed`.
    pub max_consecutive_failures: u32,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            healthy_after: Duration::from_secs(60),
            max_consecutive_failures: 5,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Try again after this delay; `attempt` counts from 1.
    Retry {
        attempt: u32,
        after: Duration,
    },
    GiveUp {
        failures: u32,
    },
}

#[derive(Debug)]
pub struct RestartState {
    policy: RestartPolicy,
    failures: u32,
}

impl RestartState {
    pub fn new(policy: RestartPolicy) -> Self {
        Self {
            policy,
            failures: 0,
        }
    }

    /// An owned run ended. `connected_for` is how long it was connected (None if it never
    /// became ready).
    pub fn run_ended(&mut self, connected_for: Option<Duration>) -> Verdict {
        if connected_for.is_some_and(|d| d >= self.policy.healthy_after) {
            self.failures = 0;
        }
        self.failures += 1;
        if self.failures >= self.policy.max_consecutive_failures {
            return Verdict::GiveUp {
                failures: self.failures,
            };
        }
        let exp = self.failures - 1;
        let factor = 1u32.checked_shl(exp).unwrap_or(u32::MAX);
        let after = self
            .policy
            .base_delay
            .saturating_mul(factor)
            .min(self.policy.max_delay);
        Verdict::Retry {
            attempt: self.failures,
            after,
        }
    }

    /// Forget past failures (user pressed retry, or an external server took over).
    pub fn reset(&mut self) {
        self.failures = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st() -> RestartState {
        RestartState::new(RestartPolicy {
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(8),
            healthy_after: Duration::from_secs(60),
            max_consecutive_failures: 5,
        })
    }

    #[test]
    fn backs_off_exponentially_then_caps() {
        let mut s = st();
        let mut delays = vec![];
        for _ in 0..4 {
            match s.run_ended(None) {
                Verdict::Retry { after, .. } => delays.push(after.as_secs()),
                v => panic!("{v:?}"),
            }
        }
        assert_eq!(delays, vec![1, 2, 4, 8]);
    }

    #[test]
    fn gives_up_after_consecutive_failures() {
        let mut s = st();
        for _ in 0..4 {
            assert!(matches!(
                s.run_ended(Some(Duration::from_secs(5))),
                Verdict::Retry { .. }
            ));
        }
        assert_eq!(s.run_ended(None), Verdict::GiveUp { failures: 5 });
    }

    #[test]
    fn a_healthy_run_resets_everything() {
        let mut s = st();
        for _ in 0..3 {
            s.run_ended(None);
        }
        // long healthy run, then a crash: back to the first delay, not the fourth
        assert_eq!(
            s.run_ended(Some(Duration::from_secs(3600))),
            Verdict::Retry {
                attempt: 1,
                after: Duration::from_secs(1)
            }
        );
    }

    #[test]
    fn huge_failure_counts_do_not_overflow() {
        let mut s = RestartState::new(RestartPolicy {
            max_consecutive_failures: u32::MAX,
            ..RestartPolicy::default()
        });
        s.failures = 200;
        assert!(
            matches!(s.run_ended(None), Verdict::Retry { after, .. } if after == Duration::from_secs(30))
        );
    }
}
