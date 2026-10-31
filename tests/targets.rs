//! Guards the target matrix.
//!
//! A target is added to `Canopus.toml` only after its firmware ABI module is
//! recovered, and an adapted module is only kept once it is really built.
//! Editing one side alone would silently drop a device from the package or
//! claim support that was never compiled, so this test fails instead. Targets
//! recovered. Static legacy integrations may be packaged for isolated
//! validation even while `SUPPORTED = false`; activation rejects those targets
//! before firmware access. `Canopus.n66-validation.toml` selects N66 alone for
//! that validation. Neither file silently turns static recovery into support.

use std::{fs, path::Path};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn build_list() -> Vec<String> {
    let manifest = fs::read_to_string(root().join("Canopus.toml")).expect("Canopus.toml");
    let start = manifest.find("include = [").expect("[targets] include");
    let rest = &manifest[start + "include = [".len()..];
    let end = rest.find(']').expect("closing bracket");
    rest[..end]
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

struct AbiModule {
    id: String,
    adapted: bool,
    source: String,
}

/// Reads the target id from either an adapted module's `TARGET_ID` or an
/// unadapted module's `abi_unsupported!("<id>")` invocation.
fn read_abi(path: &Path) -> AbiModule {
    let source = fs::read_to_string(path).expect("abi module");
    let adapted = source.contains("pub const SUPPORTED: bool = true;");
    let marker = if source.contains("pub const TARGET_ID: &str = ") {
        "pub const TARGET_ID: &str = "
    } else {
        "crate::abi_unsupported!("
    };
    let Some(at) = source.find(marker) else {
        panic!("{} does not declare its target id", path.display());
    };
    let rest = source[at + marker.len()..].strip_prefix('"');
    let rest = rest.unwrap_or_else(|| panic!("{}: literal target id", path.display()));
    let end = rest
        .find('"')
        .unwrap_or_else(|| panic!("{}: closing quote", path.display()));
    AbiModule {
        id: rest[..end].to_string(),
        adapted,
        source,
    }
}

fn abi_modules() -> Vec<AbiModule> {
    let dir = root().join("src/abi");
    let mut modules = Vec::new();
    for entry in fs::read_dir(dir).expect("src/abi") {
        let path = entry.expect("entry").path();
        // `src/abi` holds one module per exact firmware plus the shared call-shape
        // shims, which are not a target and deliberately have no target id.
        if path.extension().and_then(|e| e.to_str()) != Some("rs")
            || matches!(
                path.file_name().and_then(|n| n.to_str()),
                Some("mod.rs") | Some("shims.rs")
            )
        {
            continue;
        }
        modules.push(read_abi(&path));
    }
    modules.sort_by(|a, b| a.id.cmp(&b.id));
    modules
}

#[test]
fn the_build_list_is_exactly_the_recovered_targets() {
    let mut listed = build_list();
    listed.sort();
    let mut adapted: Vec<String> = abi_modules()
        .into_iter()
        .filter(|module| {
            module.adapted
                || (module.source.contains("pub const SUPPORTED: bool = false;")
                    && module.source.contains("pub const TARGET_ID: &str = "))
        })
        .map(|module| module.id)
        .collect();
    adapted.sort();
    assert!(!adapted.is_empty(), "no adapted target");
    assert_eq!(listed, adapted);
}

#[test]
fn unadapted_targets_declare_themselves() {
    for module in abi_modules().into_iter().filter(|m| !m.adapted) {
        assert!(
            module.source.contains("abi_unsupported!")
                || module.source.contains("pub const SUPPORTED: bool = false;"),
            "{}: an unadapted target must explicitly disable firmware support",
            module.id
        );
    }
}

#[test]
fn adapted_targets_take_addresses_from_the_generated_pack() {
    for module in abi_modules().into_iter().filter(|m| m.adapted) {
        assert!(
            module.source.contains("canopus_target_generated"),
            "{}: an adapted target must take its addresses from the generated pack",
            module.id
        );
    }
}
