// SPDX-License-Identifier: GPL-2.0-or-later
//! The schedule: what happens at set times of day, when the PC sleeps and wakes, and when it
//! shuts down.
//!
//! A rule fires at its minute on its weekdays while the app runs. Nothing is caught up: a rule
//! whose time passed while the app was closed, the PC slept or the clock jumped does not fire
//! late (the look would change at a moment nobody chose).

use chrono::{Datelike, Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};

/// Most rules a schedule keeps.
pub const MAX_RULES: usize = 32;
const EVERY_DAY: u8 = 0b111_1111;
/// A gap between two looks at the clock longer than this is a jump (sleep, a clock change),
/// not time passing: nothing in it fires.
const JUMP: Duration = Duration::minutes(2);

/// What to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Nothing,
    Off,
    /// On waking: the look from before sleep.
    Back,
    Profile {
        name: String,
    },
}

/// At `at` (minutes after midnight) on the `days` set (bit 0 Monday .. bit 6 Sunday).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub at: u16,
    #[serde(default = "every_day")]
    pub days: u8,
    pub action: Action,
}

fn every_day() -> u8 {
    EVERY_DAY
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default = "sleep_default")]
    pub sleep: Action,
    #[serde(default = "wake_default")]
    pub wake: Action,
    /// Shutting down, restarting or signing out. Off, or nothing: devices go back to the
    /// modes they had before freeRGB, as when it quits.
    #[serde(default = "shutdown_default")]
    pub shutdown: Action,
}

fn sleep_default() -> Action {
    Action::Off
}

fn wake_default() -> Action {
    Action::Back
}

fn shutdown_default() -> Action {
    Action::Off
}

impl Default for Schedule {
    fn default() -> Self {
        Schedule {
            rules: vec![],
            sleep: sleep_default(),
            wake: wake_default(),
            shutdown: shutdown_default(),
        }
    }
}

impl Schedule {
    /// Why this schedule cannot be kept; `profiles` are the names that exist.
    pub fn check(&self, profiles: &[&str]) -> Result<(), String> {
        let known = |a: &Action| match a {
            Action::Profile { name } if !profiles.contains(&name.as_str()) => {
                Err(format!("No profile called “{name}”"))
            }
            _ => Ok(()),
        };
        if self.rules.len() > MAX_RULES {
            return Err(format!("Keep it to {MAX_RULES} times"));
        }
        for r in &self.rules {
            if r.at >= 24 * 60 {
                return Err("A time must be within the day".into());
            }
            if r.days & EVERY_DAY == 0 {
                return Err("A time needs at least one day".into());
            }
            if !matches!(r.action, Action::Off | Action::Profile { .. }) {
                return Err("A time turns the lights off or applies a profile".into());
            }
            known(&r.action)?;
        }
        if matches!(self.sleep, Action::Back) {
            return Err("Going to sleep cannot go back".into());
        }
        if matches!(self.wake, Action::Off) {
            return Err("Waking cannot turn the lights off".into());
        }
        if !matches!(self.shutdown, Action::Off | Action::Nothing) {
            return Err("Shutting down turns the lights off or leaves them".into());
        }
        known(&self.sleep)?;
        known(&self.wake)
    }

    /// Follow a profile's new name.
    pub fn renamed(&mut self, from: &str, to: &str) {
        self.each_action(|a| {
            if let Action::Profile { name } = a {
                if name == from {
                    *name = to.to_string();
                }
            }
        });
    }

    /// Forget a deleted profile: its times go, sleep and wake go back to their defaults.
    pub fn deleted(&mut self, gone: &str) {
        let uses = |a: &Action| matches!(a, Action::Profile { name } if name == gone);
        self.rules.retain(|r| !uses(&r.action));
        if uses(&self.sleep) {
            self.sleep = sleep_default();
        }
        if uses(&self.wake) {
            self.wake = wake_default();
        }
    }

    fn each_action(&mut self, mut f: impl FnMut(&mut Action)) {
        for r in &mut self.rules {
            f(&mut r.action);
        }
        f(&mut self.sleep);
        f(&mut self.wake);
    }

    /// The rule to run for the clock moving from `prev` to `now` (local time): the last one
    /// whose moment is after `prev` and no later than `now`. None across a jump.
    pub fn due(&self, prev: NaiveDateTime, now: NaiveDateTime) -> Option<&Rule> {
        if now <= prev || now - prev > JUMP {
            return None;
        }
        self.rules
            .iter()
            .filter_map(|r| {
                // The moment can be today's or, just after midnight, yesterday's.
                [now.date(), now.date() - Duration::days(1)]
                    .into_iter()
                    .filter(|d| r.days & (1 << d.weekday().num_days_from_monday()) != 0)
                    .filter_map(|d| d.and_hms_opt(u32::from(r.at / 60), u32::from(r.at % 60), 0))
                    .find(|t| prev < *t && *t <= now)
                    .map(|t| (t, r))
            })
            .max_by_key(|(t, _)| *t)
            .map(|(_, r)| r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// 2026-11-16 is a Monday.
    fn at(day: u32, h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, day)
            .unwrap()
            .and_hms_opt(h, m, s)
            .unwrap()
    }

    fn profile(name: &str) -> Action {
        Action::Profile { name: name.into() }
    }

    fn rule(at: u16, days: u8, action: Action) -> Rule {
        Rule { at, days, action }
    }

    #[test]
    fn a_rule_fires_once_when_its_minute_comes() {
        let s = Schedule {
            rules: vec![rule(23 * 60, EVERY_DAY, Action::Off)],
            ..Default::default()
        };
        assert_eq!(
            s.due(at(16, 22, 59, 45), at(16, 23, 0, 0)),
            Some(&s.rules[0])
        );
        assert_eq!(
            s.due(at(16, 23, 0, 0), at(16, 23, 0, 15)),
            None,
            "only once"
        );
        assert_eq!(
            s.due(at(16, 22, 59, 0), at(16, 22, 59, 30)),
            None,
            "not early"
        );
    }

    #[test]
    fn a_rule_keeps_to_its_days() {
        let weekdays = 0b001_1111;
        let s = Schedule {
            rules: vec![rule(8 * 60, weekdays, profile("Work"))],
            ..Default::default()
        };
        // Monday 16th fires; Sunday 22nd does not.
        assert!(s.due(at(16, 7, 59, 50), at(16, 8, 0, 5)).is_some());
        assert!(s.due(at(22, 7, 59, 50), at(22, 8, 0, 5)).is_none());
    }

    #[test]
    fn the_later_of_two_rules_in_one_step_wins_and_midnight_counts() {
        let s = Schedule {
            rules: vec![
                rule(0, EVERY_DAY, profile("Night")),
                rule(23 * 60 + 59, EVERY_DAY, Action::Off),
            ],
            ..Default::default()
        };
        // 23:58:50 -> 00:00:10 the next day passes both.
        let r = s.due(at(16, 23, 58, 50), at(17, 0, 0, 10)).unwrap();
        assert_eq!(r.action, profile("Night"));
        // A rule for Monday at 23:59 seen after midnight still fires (yesterday's moment).
        let mon = Schedule {
            rules: vec![rule(23 * 60 + 59, 0b1, Action::Off)],
            ..Default::default()
        };
        assert!(
            mon.due(at(16, 23, 59, 0), at(17, 0, 0, 5)).is_none(),
            "23:59:00 is not after itself"
        );
        assert!(mon.due(at(16, 23, 58, 59), at(17, 0, 0, 5)).is_some());
    }

    #[test]
    fn nothing_is_caught_up_across_a_jump() {
        let s = Schedule {
            rules: vec![rule(23 * 60, EVERY_DAY, Action::Off)],
            ..Default::default()
        };
        assert_eq!(
            s.due(at(16, 22, 0, 0), at(16, 23, 30, 0)),
            None,
            "slept through it"
        );
        assert_eq!(
            s.due(at(16, 23, 0, 30), at(16, 22, 59, 30)),
            None,
            "clock set back"
        );
    }

    #[test]
    fn a_schedule_is_checked_before_it_is_kept() {
        let ok = Schedule {
            rules: vec![rule(60, EVERY_DAY, profile("Calm"))],
            sleep: Action::Off,
            wake: profile("Calm"),
            shutdown: Action::Nothing,
        };
        assert_eq!(ok.check(&["Calm"]), Ok(()));
        assert!(ok.check(&[]).is_err(), "profile must exist");

        let bad = |f: fn(&mut Schedule)| {
            let mut s = ok.clone();
            f(&mut s);
            s.check(&["Calm"]).is_err()
        };
        assert!(bad(|s| s.rules[0].at = 24 * 60));
        assert!(bad(|s| s.rules[0].days = 0));
        assert!(bad(|s| s.rules[0].action = Action::Back));
        assert!(bad(|s| s.rules[0].action = Action::Nothing));
        assert!(bad(|s| s.sleep = Action::Back));
        assert!(bad(|s| s.wake = Action::Off));
        assert!(bad(|s| s.shutdown = Action::Back));
        assert!(bad(|s| s.shutdown = profile("Calm")));
        assert!(bad(
            |s| s.rules = vec![rule(0, 1, Action::Off); MAX_RULES + 1]
        ));
    }

    #[test]
    fn profiles_renamed_or_deleted_carry_into_the_schedule() {
        let mut s = Schedule {
            rules: vec![
                rule(60, EVERY_DAY, profile("A")),
                rule(120, EVERY_DAY, Action::Off),
            ],
            sleep: profile("A"),
            wake: profile("A"),
            shutdown: Action::Off,
        };
        s.renamed("A", "B");
        assert_eq!(
            (s.rules[0].action.clone(), s.wake.clone()),
            (profile("B"), profile("B"))
        );
        s.deleted("B");
        assert_eq!(s.rules, [rule(120, EVERY_DAY, Action::Off)]);
        assert_eq!((s.sleep, s.wake), (Action::Off, Action::Back));
    }

    #[test]
    fn the_file_format_reads_well_and_defaults_fill_in() {
        let s: Schedule =
            serde_json::from_str(r#"{"rules":[{"at":1380,"action":{"kind":"off"}}]}"#).unwrap();
        assert_eq!(s.rules[0].days, EVERY_DAY);
        assert_eq!(
            (s.sleep.clone(), s.wake.clone(), s.shutdown.clone()),
            (Action::Off, Action::Back, Action::Off)
        );
        let json = serde_json::to_string(&profile("Work")).unwrap();
        assert_eq!(json, r#"{"kind":"profile","name":"Work"}"#);
    }
}
