//! Generic voting: pluggable quorum strategies instead of a hardcoded
//! threshold. Generalizes the logic already written and unit-tested in
//! `b00t-ipc`'s `Proposal::is_passed`/`is_rejected` (there, hardcoded to
//! "2 yes votes") so any subsystem's vote type can plug in as `O`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;

/// How many cast ballots an option needs to win.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Quorum {
    /// Strict majority of all non-abstain ballots cast so far.
    Majority,
    /// A fixed count, independent of how many players are eligible.
    AtLeast(usize),
    /// Every non-abstain ballot cast so far must agree.
    Unanimous,
    /// Liberum veto (Polish–Lithuanian Sejm, 16th–18th c.): any single bare
    /// [`Ballot::Veto`] blocks immediately — unlike `Unanimous`, which just
    /// stays [`Outcome::Pending`] forever on disagreement, a veto here
    /// resolves straight to [`Outcome::Rejected`]. Non-veto `Cast` ballots
    /// still must all agree on one option to `Pass`, exactly as under
    /// `Unanimous`.
    LiberumVeto,
}

/// A single player's vote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ballot<O> {
    /// A vote for a specific option.
    Cast(O),
    /// A block. `alternative: None` rejects outright, regardless of quorum
    /// (mirrors `b00t_c0re_lib`'s `VotingType::VetoCapable`); `Some(o)`
    /// instead casts for the proposed alternative.
    Veto { alternative: Option<O> },
    /// Recorded but does not count toward quorum.
    Abstain,
}

/// A proposal open for voting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal<O> {
    pub id: String,
    pub subject: String,
    pub options: Vec<O>,
    pub quorum: Quorum,
    pub deadline: Option<DateTime<Utc>>,
    /// Informational only — `tally` does not restrict who may vote.
    /// Empty means unrestricted.
    #[serde(default)]
    pub eligible_voters: Vec<String>,
}

/// Result of [`tally`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome<O> {
    Passed(O),
    Rejected,
    Pending,
}

/// Tally `ballots` against `options` under `quorum`.
///
/// - Under [`Quorum::LiberumVeto`], any bare [`Ballot::Veto`] (no
///   alternative) rejects immediately, regardless of other ballots. Under
///   every other quorum, a bare veto doesn't count toward any option —
///   same as [`Ballot::Abstain`] — and does not block.
/// - Otherwise, counts [`Ballot::Cast`] (and vetoes-with-an-alternative) per
///   option, in `options` order, and returns the first option whose count
///   meets the threshold `quorum` implies. Ties are broken by `options`
///   order. Returns [`Outcome::Pending`] if no option has met the threshold
///   yet — more ballots may still arrive.
pub fn tally<O: Eq + Hash + Clone>(
    options: &[O],
    ballots: &[(String, Ballot<O>)],
    quorum: &Quorum,
) -> Outcome<O> {
    if matches!(quorum, Quorum::LiberumVeto)
        && ballots
            .iter()
            .any(|(_, b)| matches!(b, Ballot::Veto { alternative: None }))
    {
        return Outcome::Rejected;
    }

    let mut counts: HashMap<&O, usize> = HashMap::new();
    let mut total_cast = 0usize;
    for (_, ballot) in ballots {
        let cast_for = match ballot {
            Ballot::Cast(o) => Some(o),
            Ballot::Veto {
                alternative: Some(o),
            } => Some(o),
            // Only blocks under `LiberumVeto`, handled above; under every
            // other quorum it's a no-op, same as `Abstain`.
            Ballot::Veto { alternative: None } => None,
            Ballot::Abstain => None,
        };
        if let Some(o) = cast_for {
            *counts.entry(o).or_insert(0) += 1;
            total_cast += 1;
        }
    }

    for option in options {
        let count = counts.get(option).copied().unwrap_or(0);
        let needed = match quorum {
            Quorum::AtLeast(n) => *n,
            Quorum::Majority => total_cast / 2 + 1,
            Quorum::Unanimous | Quorum::LiberumVeto => total_cast.max(1),
        };
        if count > 0 && count >= needed {
            return Outcome::Passed(option.clone());
        }
    }

    Outcome::Pending
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ballots<O: Clone>(pairs: &[(&str, Ballot<O>)]) -> Vec<(String, Ballot<O>)> {
        pairs
            .iter()
            .map(|(voter, b)| ((*voter).to_string(), b.clone()))
            .collect()
    }

    #[test]
    fn at_least_n_passes_once_threshold_met() {
        let options = ["yes", "no"];
        let quorum = Quorum::AtLeast(2);

        let one_vote = ballots(&[("a", Ballot::Cast("yes"))]);
        assert_eq!(tally(&options, &one_vote, &quorum), Outcome::Pending);

        let two_votes = ballots(&[("a", Ballot::Cast("yes")), ("b", Ballot::Cast("yes"))]);
        assert_eq!(tally(&options, &two_votes, &quorum), Outcome::Passed("yes"));

        let two_no = ballots(&[("a", Ballot::Cast("no")), ("b", Ballot::Cast("no"))]);
        assert_eq!(tally(&options, &two_no, &quorum), Outcome::Passed("no"));
    }

    #[test]
    fn matches_b00t_ipc_semantics_ported_to_bool_options() {
        // b00t-ipc's Proposal::is_passed/is_rejected: yes>=2 passes, no>=2
        // rejects, quorum default 2. Ported here with O = bool.
        let options = [true, false];
        let quorum = Quorum::AtLeast(2);

        let mixed = ballots(&[
            ("a", Ballot::Cast(true)),
            ("b", Ballot::Cast(false)),
            ("c", Ballot::Abstain),
        ]);
        assert_eq!(tally(&options, &mixed, &quorum), Outcome::Pending);
    }

    #[test]
    fn majority_needs_more_than_half_of_cast_ballots() {
        let options = ["a", "b", "c"];
        let quorum = Quorum::Majority;

        let three_two = ballots(&[
            ("1", Ballot::Cast("a")),
            ("2", Ballot::Cast("a")),
            ("3", Ballot::Cast("b")),
        ]);
        // 2 of 3 cast ballots > half (1.5) -> "a" passes.
        assert_eq!(tally(&options, &three_two, &quorum), Outcome::Passed("a"));

        let tie = ballots(&[("1", Ballot::Cast("a")), ("2", Ballot::Cast("b"))]);
        assert_eq!(tally(&options, &tie, &quorum), Outcome::Pending);
    }

    #[test]
    fn unanimous_requires_every_cast_ballot_to_agree() {
        let options = ["a", "b"];
        let quorum = Quorum::Unanimous;

        let split = ballots(&[("1", Ballot::Cast("a")), ("2", Ballot::Cast("b"))]);
        assert_eq!(tally(&options, &split, &quorum), Outcome::Pending);

        let agree = ballots(&[("1", Ballot::Cast("a")), ("2", Ballot::Cast("a"))]);
        assert_eq!(tally(&options, &agree, &quorum), Outcome::Passed("a"));
    }

    #[test]
    fn liberum_veto_blocks_on_any_bare_veto() {
        let options = ["a", "b"];
        let quorum = Quorum::LiberumVeto;

        let vetoed = ballots(&[
            ("1", Ballot::Cast("a")),
            ("2", Ballot::Veto { alternative: None }),
        ]);
        assert_eq!(tally(&options, &vetoed, &quorum), Outcome::Rejected);
    }

    #[test]
    fn bare_veto_only_blocks_under_liberum_veto_quorum() {
        let options = ["a", "b"];
        let quorum = Quorum::AtLeast(1);

        // Same ballots as `liberum_veto_blocks_on_any_bare_veto`, but under
        // a plain AtLeast quorum: the veto doesn't block, and "a" passes on
        // its own single Cast vote.
        let vetoed = ballots(&[
            ("1", Ballot::Cast("a")),
            ("2", Ballot::Veto { alternative: None }),
        ]);
        assert_eq!(tally(&options, &vetoed, &quorum), Outcome::Passed("a"));
    }

    #[test]
    fn veto_with_alternative_counts_as_a_cast_vote() {
        let options = ["a", "b"];
        let quorum = Quorum::AtLeast(2);

        let votes = ballots(&[
            ("1", Ballot::Cast("b")),
            (
                "2",
                Ballot::Veto {
                    alternative: Some("b"),
                },
            ),
        ]);
        assert_eq!(tally(&options, &votes, &quorum), Outcome::Passed("b"));
    }
}
