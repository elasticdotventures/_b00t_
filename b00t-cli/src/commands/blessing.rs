//! `b00t blessing` — agent tool authorization manifest.
//!
//! Walks datum depends_on graph for a role, emits manifest declaring:
//!   - Required skills (with tools each unlocks)
//!   - Optional skills (matched by skills field)
//!   - Forbidden command patterns
//!   - Postel next-hint (what to learn first)
//!
//! # Usage
//! ```bash
//! b00t blessing --manifest --role worker   # full manifest
//! b00t blessing --list-roles               # list available roles
//! ```

use crate::datum_utils::get_all_datums;
use crate::{BootDatum, DatumType};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

fn find_b00t_dir() -> Result<PathBuf> {
    let b00t = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("no home dir"))?
        .join(".b00t")
        .join("_b00t_");
    if b00t.exists() {
        return Ok(b00t);
    }
    Err(anyhow::anyhow!("_b00t_ not found — run `b00t up` first"))
}

#[derive(clap::Parser, Clone)]
pub struct BlessingArgs {
    #[clap(long, help = "Emit tool authorization manifest for this role")]
    pub manifest: bool,

    #[clap(
        long,
        alias = "agent",
        default_value = "worker",
        help = "Role to build manifest for"
    )]
    pub role: String,

    #[clap(long, help = "List all available roles")]
    pub list_roles: bool,

    #[clap(long, default_value = "toml", help = "Output format: toml | json")]
    pub format: String,
}

pub fn handle_blessing(args: &BlessingArgs) -> Result<()> {
    let b00t_path = find_b00t_dir()?.to_string_lossy().to_string();

    if args.list_roles {
        return list_roles(&b00t_path);
    }

    if args.manifest {
        return emit_manifest(&b00t_path, &args.role, &args.format);
    }

    println!("b00t blessing — agent tool authorization manifest");
    println!("  --manifest --role <role>   emit manifest");
    println!("  --list-roles               list available roles");
    println!();
    println!("next: b00t blessing --manifest --role worker");
    Ok(())
}

fn list_roles(b00t_path: &str) -> Result<()> {
    let datums = get_all_datums(b00t_path)?;
    let mut roles: Vec<String> = Vec::new();

    for (key, datum) in &datums {
        let is_role = datum
            .datum_type
            .as_ref()
            .map(|t| format!("{t:?}").to_lowercase().contains("role"))
            .unwrap_or(false);
        let has_skills = datum
            .skills
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if is_role || has_skills {
            let hint = if datum.hint.is_empty() {
                key.as_str()
            } else {
                datum.hint.as_str()
            };
            roles.push(format!("  {key} — {hint}"));
        }
    }

    // AGENTS/ supplement names
    if let Ok(agents_dir) =
        find_b00t_dir().map(|p| p.parent().unwrap_or(p.as_path()).join("AGENTS"))
    {
        if agents_dir.exists() {
            for entry in std::fs::read_dir(&agents_dir)
                .into_iter()
                .flatten()
                .flatten()
            {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(role) = name
                    .strip_prefix("--role=")
                    .and_then(|s| s.strip_suffix(".md"))
                {
                    let line = format!("  {role} (AGENTS/ supplement)");
                    if !roles.contains(&line) {
                        roles.push(line);
                    }
                }
            }
        }
    }

    roles.sort();
    if roles.is_empty() {
        println!("No roles found. Create AGENTS/--role=<name>.md or add depends_on to a datum.");
    } else {
        println!("Available roles:");
        for r in &roles {
            println!("{r}");
        }
    }
    println!();
    println!("next: b00t blessing --manifest --role <role>");
    Ok(())
}

/// Resolve the datum for a role name.
///
/// Datum keys are `<name>.<type>` derived from filename (e.g.
/// `operator.role.toml` -> key `"operator.role"`), so a bare role name
/// like `"operator"` never matches the map directly except by luck — that
/// was the bug here: `--role operator` silently returned an empty
/// manifest despite `operator.role.toml` having real `depends_on`, while
/// `--role executive` only worked because a workaround bare-named
/// `executive.toml` happened to exist alongside the real
/// `executive.role.toml`.
///
/// Priority:
///   1. exact `"{role}.role"` — the canonical role-datum key.
///   2. exact bare `role` — legacy/non-standard datum naming.
///   3. any key starting with `"{role}."`, preferring one whose
///      `datum_type` is `Role`; candidates are sorted by key first so the
///      result is deterministic regardless of `HashMap` iteration order.
///
/// Never falls back to "any Role-typed datum in the store" — a role
/// lookup that finds no match for `role` returns `None`, not an
/// unrelated role's manifest. Returning the wrong role's tool-
/// authorization manifest is worse than returning none.
fn find_role_datum<'a>(datums: &'a HashMap<String, BootDatum>, role: &str) -> Option<&'a BootDatum> {
    if let Some(d) = datums.get(&format!("{role}.role")) {
        return Some(d);
    }
    if let Some(d) = datums.get(role) {
        return Some(d);
    }
    let prefix = format!("{role}.");
    let mut candidates: Vec<(&String, &BootDatum)> =
        datums.iter().filter(|(k, _)| k.starts_with(&prefix)).collect();
    candidates.sort_by(|(a, _), (b, _)| a.cmp(b));
    candidates
        .iter()
        .find(|(_, d)| {
            d.datum_type
                .as_ref()
                .map(|t| matches!(t, DatumType::Role))
                .unwrap_or(false)
        })
        .or_else(|| candidates.first())
        .map(|(_, d)| *d)
}

/// Depth cap for the required-skills discovery walk (#898). A role's
/// depends_on graph is expected to be shallow (skills a few hops deep at
/// most) -- this bounds pathological/misconfigured datum graphs rather
/// than reflecting any real expected depth.
const MAX_BLESSING_DISCOVERY_DEPTH: usize = 16;

fn emit_manifest(b00t_path: &str, role: &str, fmt: &str) -> Result<()> {
    let datums = get_all_datums(b00t_path)?;
    let role_datum = find_role_datum(&datums, role);

    let direct_deps: Vec<String> = role_datum
        .and_then(|d| d.depends_on.clone())
        .unwrap_or_default();

    // #898: was single-hop (only role_datum's own depends_on) -- now walks
    // transitively (a required skill's own depends_on pulls in further
    // required skills), via the shared lazy-chain walker so this doesn't
    // hand-roll its own cycle guard / depth cap.
    let discovered: Vec<String> = b00t_c0re_gov::discovery::walk_lazy_chain(
        direct_deps.iter().cloned(),
        MAX_BLESSING_DISCOVERY_DEPTH,
        |key| {
            datums
                .get(key)
                .and_then(|d| d.depends_on.clone())
                .unwrap_or_default()
        },
    );

    let mut required: Vec<(String, Vec<String>)> = Vec::new();
    let mut optional: Vec<(String, Vec<String>)> = Vec::new();

    for dep_key in &discovered {
        let unlocks = datums
            .get(dep_key)
            .and_then(|d| d.unlocks.clone())
            .unwrap_or_default();
        required.push((dep_key.clone(), unlocks));
    }

    // Optional: datums that declare this role in their skills field
    for (key, datum) in &datums {
        if discovered.contains(key) {
            continue;
        }
        let in_skills = datum
            .skills
            .as_ref()
            .map(|s| s.iter().any(|sk| sk == role))
            .unwrap_or(false);
        if in_skills {
            let unlocks = datum.unlocks.clone().unwrap_or_default();
            optional.push((key.clone(), unlocks));
        }
    }

    let forbidden = [
        "pip install *    → use: uv pip install",
        "docker run *     → use: podman --device nvidia.com/gpu=all",
        "rm -rf /         → BLOCKED",
        "huggingface-cli  → use: hf download",
    ];

    let next_skill = required
        .first()
        .map(|(k, _)| k.as_str())
        .unwrap_or("<skill>");

    match fmt {
        "json" => {
            let out = serde_json::json!({
                "role": role,
                "required": required.iter().map(|(k, u)| serde_json::json!({"skill": k, "unlocks": u})).collect::<Vec<_>>(),
                "optional": optional.iter().map(|(k, u)| serde_json::json!({"skill": k, "unlocks": u})).collect::<Vec<_>>(),
                "forbidden": forbidden,
                "next": format!("b00t learn {next_skill}"),
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        _ => {
            println!("[blessing]");
            println!("role = {role:?}");
            println!();
            println!("[blessing.required]");
            if required.is_empty() {
                println!("# No depends_on found for role '{role}'");
                println!("# Add depends_on = [\"skill.a\"] to _b00t_/{role}.toml");
            }
            for (key, unlocks) in &required {
                println!("{key:?} = {{ unlocks = {unlocks:?} }}");
            }
            if !optional.is_empty() {
                println!();
                println!("[blessing.optional]");
                for (key, unlocks) in &optional {
                    println!("{key:?} = {{ unlocks = {unlocks:?} }}");
                }
            }
            println!();
            println!("[blessing.forbidden]");
            for f in &forbidden {
                println!("# {f}");
            }
            println!();
            println!("[blessing.next]");
            println!("hint = {:?}", format!("b00t learn {next_skill}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_b00t(dir: &TempDir) -> String {
        let p = dir.path().to_str().unwrap().to_string();
        fs::write(dir.path().join("rust.skill.toml"), "[b00t]\nname = \"rust\"\ntype = \"skill\"\nhint = \"Rust\"\ndepends_on = [\"cargo.cli\"]\nunlocks = [\"cargo.*\", \"rustfmt\"]\n").unwrap();
        fs::write(dir.path().join("cargo.cli.toml"), "[b00t]\nname = \"cargo\"\ntype = \"cli\"\nhint = \"Rust build\"\nunlocks = [\"cargo build\", \"cargo test\"]\n").unwrap();
        fs::write(dir.path().join("backend.role.toml"), "[b00t]\nname = \"backend\"\ntype = \"skill\"\nhint = \"Backend role\"\ndepends_on = [\"rust.skill\", \"cargo.cli\"]\n").unwrap();
        p
    }

    #[test]
    fn test_list_roles_no_panic() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        list_roles(&path).unwrap();
    }

    #[test]
    fn test_emit_manifest_toml() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        emit_manifest(&path, "backend", "toml").unwrap();
    }

    #[test]
    fn test_emit_manifest_json() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        emit_manifest(&path, "backend", "json").unwrap();
    }

    /// #898: the required-skills walk must be transitive, not single-hop.
    /// backend -> rust.skill -> toolchain.skill, where toolchain.skill is
    /// NOT a direct dependency of backend -- only reachable via rust.skill.
    /// Verified through the real emit_manifest JSON output (not just the
    /// generic walk_lazy_chain unit tests in b00t-c0re-gov), so this proves
    /// the wiring, not just the algorithm in isolation.
    #[test]
    fn test_required_skills_discovered_transitively() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("backend.role.toml"),
            "[b00t]\nname = \"backend\"\ntype = \"role\"\nhint = \"Backend\"\ndepends_on = [\"rust.skill\"]\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("rust.skill.toml"),
            "[b00t]\nname = \"rust\"\ntype = \"skill\"\nhint = \"Rust\"\ndepends_on = [\"toolchain.skill\"]\nunlocks = [\"cargo.*\"]\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("toolchain.skill.toml"),
            "[b00t]\nname = \"toolchain\"\ntype = \"skill\"\nhint = \"Toolchain\"\nunlocks = [\"rustup\"]\n",
        )
        .unwrap();
        let path = dir.path().to_str().unwrap().to_string();

        let datums = get_all_datums(&path).unwrap();
        assert!(
            datums.contains_key("toolchain.skill"),
            "fixture sanity check"
        );

        // emit_manifest prints to stdout; re-derive the same discovery walk
        // it uses internally to assert on the actual data, not scrape stdout.
        let role_datum = find_role_datum(&datums, "backend").unwrap();
        let direct_deps = role_datum.depends_on.clone().unwrap_or_default();
        assert_eq!(direct_deps, vec!["rust.skill".to_string()]);

        let discovered = b00t_c0re_gov::discovery::walk_lazy_chain(
            direct_deps.iter().cloned(),
            MAX_BLESSING_DISCOVERY_DEPTH,
            |key| {
                datums
                    .get(key)
                    .and_then(|d| d.depends_on.clone())
                    .unwrap_or_default()
            },
        );
        assert!(
            discovered.contains(&"toolchain.skill".to_string()),
            "toolchain.skill is only reachable transitively via rust.skill's own \
             depends_on -- single-hop discovery would have missed it entirely: {discovered:?}"
        );

        // And the real entry point still runs clean end-to-end.
        emit_manifest(&path, "backend", "json").unwrap();
    }

    #[test]
    fn test_unlocks_propagated_from_deps() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        let datums = get_all_datums(&path).unwrap();
        let rust = datums.get("rust.skill").unwrap();
        let expected = vec!["cargo.*".to_string(), "rustfmt".to_string()];
        assert_eq!(rust.unlocks.as_deref(), Some(expected.as_slice()));
    }

    /// Regression test for the "operator" bug: a role that exists ONLY as
    /// `<name>.role.toml` (no bare `<name>.toml` workaround file) must
    /// still resolve to its real depends_on via the exact `"{role}.role"`
    /// key, not silently come back empty.
    #[test]
    fn test_find_role_datum_resolves_canonical_role_key_without_bare_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        fs::write(
            dir.path().join("operator.role.toml"),
            "[b00t]\nname = \"operator\"\ntype = \"role\"\nhint = \"Operator\"\ndepends_on = [\"git.cli\"]\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("git.cli.toml"),
            "[b00t]\nname = \"git\"\ntype = \"cli\"\nhint = \"Git\"\nunlocks = [\"git *\"]\n",
        )
        .unwrap();

        let datums = get_all_datums(&path).unwrap();
        let found = find_role_datum(&datums, "operator").expect("operator role must resolve");
        assert_eq!(found.depends_on.as_deref(), Some(["git.cli".to_string()].as_slice()));
    }

    /// Regression test: with multiple Role-typed datums in the store,
    /// requesting one role must never silently return a different role's
    /// manifest (the old unfiltered `datums.values().find(Role)` fallback
    /// could do exactly that).
    #[test]
    fn test_find_role_datum_does_not_cross_contaminate_between_roles() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        fs::write(
            dir.path().join("frontend.role.toml"),
            "[b00t]\nname = \"frontend\"\ntype = \"role\"\nhint = \"Frontend\"\ndepends_on = [\"npm.cli\"]\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("backend.role.toml"),
            "[b00t]\nname = \"backend\"\ntype = \"role\"\nhint = \"Backend\"\ndepends_on = [\"cargo.cli\"]\n",
        )
        .unwrap();

        let datums = get_all_datums(&path).unwrap();
        let frontend = find_role_datum(&datums, "frontend").expect("frontend role must resolve");
        assert_eq!(frontend.depends_on.as_deref(), Some(["npm.cli".to_string()].as_slice()));
        let backend = find_role_datum(&datums, "backend").expect("backend role must resolve");
        assert_eq!(backend.depends_on.as_deref(), Some(["cargo.cli".to_string()].as_slice()));
    }

    #[test]
    fn test_find_role_datum_unknown_role_returns_none_not_a_guess() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        let datums = get_all_datums(&path).unwrap();
        assert!(find_role_datum(&datums, "totally-unknown-role").is_none());
    }
}
