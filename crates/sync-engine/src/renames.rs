//! Conservative content-based rename ambiguity. No rename is inferred from
//! timestamps or equal directory markers. The caller supplies verified paths
//! from this discovery; unrelated cached records are not deletion evidence.
use crate::{Action, Mode, Operation, fs::path_key};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const AMBIGUOUS_RENAME: &str =
    "Ambiguous rename candidates share baseline content; originals preserved until explicit review";

pub(crate) fn protect_ambiguous(
    mode: Mode,
    operations: &mut [Operation],
    baselines: &BTreeMap<String, String>,
) {
    if mode != Mode::TwoWay {
        return;
    }
    let mut origins: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut targets: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    for (i, operation) in operations.iter().enumerate() {
        let baseline = baselines.get(&path_key(&operation.path));
        if let Some(digest) = baseline {
            if digest != "directory" && (operation.a.is_none() || operation.b.is_none()) {
                origins.entry(digest).or_default().push(i);
            }
        } else {
            for entry in [&operation.a, &operation.b].into_iter().flatten() {
                if !entry.directory {
                    targets.entry(&entry.digest).or_default().insert(i);
                }
            }
        }
    }
    let mut ambiguous = BTreeSet::new();
    for (digest, old) in origins {
        if let Some(new) = targets.get(digest)
            && (old.len() > 1 || new.len() > 1)
        {
            ambiguous.extend(old);
            ambiguous.extend(new);
        }
    }
    for i in ambiguous {
        operations[i].action = Action::Conflict;
        operations[i].reason = AMBIGUOUS_RENAME.into();
    }
}
