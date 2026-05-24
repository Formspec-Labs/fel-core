//! Phase 3 P0 proptest #2 — ExtensionRegistry idempotence + lookup closure. See thoughts/2026-05-23-phase-3-ci-gate-design.md.
//!
//! Properties tested:
//!
//! 1. **Lookup closure** — for any registered package's function set,
//!    `registry.get(name)` returns `Some` for every name in the package and
//!    `None` for any name NOT in any registered package. The lookup is closed
//!    under the union of registered name sets.
//! 2. **Package re-registration idempotence** — registering a package N times
//!    has the same observable effect as registering it once. `registry.get(name)`
//!    after N registrations matches the result after a single registration.
//! 3. **Disjoint package union** — registering two packages with disjoint
//!    function-name sets exposes the union of both name sets. Intersection-
//!    on-conflict behavior is intentionally out of scope here.
//!
//! Packages are hand-constructed (`Package::Universal` / `Package::Formspec`);
//! only the function-name SET is generated. Generating arbitrary `Package`
//! enum values would test the proptest strategy, not the registry semantics.

#![allow(clippy::missing_docs_in_private_items)]

use fel_core::extensions::{ExtensionRegistry, Package};
use fel_core::types::Value as TypeValue;
use proptest::prelude::*;
use std::collections::HashSet;

/// Reserved words and built-in function names are rejected by
/// `ExtensionRegistry::register`. Generated names must avoid them so the
/// proptest exercises lookup semantics, not the conflict-rejection branch.
/// Prefixing each generated name with `"ext_"` is sufficient: no reserved
/// word or built-in name starts with that prefix.
const NAME_PREFIX: &str = "ext_";

/// Strategy for a single safe extension-function name.
fn arb_safe_name() -> impl Strategy<Value = String> {
    "[a-z]{1,8}".prop_map(|s| format!("{NAME_PREFIX}{s}"))
}

/// Strategy for a (possibly empty) set of distinct safe names.
/// Sized 0..=8 to keep proptest cases fast.
fn arb_name_set() -> impl Strategy<Value = HashSet<String>> {
    prop::collection::hash_set(arb_safe_name(), 0..8)
}

/// Strategy for TWO disjoint name sets. The second is the difference of a
/// freshly generated set against the first.
fn arb_disjoint_name_sets() -> impl Strategy<Value = (HashSet<String>, HashSet<String>)> {
    (arb_name_set(), arb_name_set()).prop_map(|(a, b)| {
        let b_disjoint: HashSet<String> = b.difference(&a).cloned().collect();
        (a, b_disjoint)
    })
}

/// Register every name in `names` as a no-op extension under the given
/// (logical) package. `Package` is metadata-only here — the registry stores
/// no package label per entry; we use it to document the test intent and
/// keep symmetry with the catalog's package classification.
fn register_package(registry: &mut ExtensionRegistry, _package: Package, names: &HashSet<String>) {
    for name in names {
        // No-op body; we are testing registration/lookup, not invocation.
        registry
            .register(name.clone(), 0, None, |_| TypeValue::Null)
            .expect("generated names are conflict-free");
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        ..Default::default()
    })]

    /// Property 1 — Lookup closure.
    ///
    /// After registering one package's name set, `get(name)` returns `Some`
    /// for every name in the set, and `None` for every probe outside the set.
    #[test]
    fn lookup_is_closed_under_registered_name_set(
        names in arb_name_set(),
        probe in arb_safe_name(),
    ) {
        let mut registry = ExtensionRegistry::new();
        register_package(&mut registry, Package::Universal, &names);

        // Every registered name resolves.
        for name in &names {
            prop_assert!(
                registry.get(name).is_some(),
                "registered name {name:?} must resolve via get()"
            );
            prop_assert!(registry.contains(name));
        }

        // A name NOT in the set must NOT resolve. (If the probe happened to
        // land inside the generated set, the prior loop has already asserted
        // Some — the closure property holds in both directions.)
        if !names.contains(&probe) {
            prop_assert!(
                registry.get(&probe).is_none(),
                "unregistered name {probe:?} must NOT resolve via get()"
            );
            prop_assert!(!registry.contains(&probe));
        }
    }

    /// Property 2 — Package re-registration idempotence.
    ///
    /// Registering the same package N times (N ∈ {2,3,4,5}) yields the same
    /// observable `get(name)` outcome as registering it exactly once. This
    /// is the structural guarantee that the registry's `HashMap::insert`
    /// semantics carry the spec invariant: re-registration overwrites
    /// in-place; it does not duplicate, append, or diverge.
    #[test]
    fn re_registration_is_idempotent(
        names in arb_name_set(),
        n in 2u32..=5,
    ) {
        // Baseline — register once.
        let mut once = ExtensionRegistry::new();
        register_package(&mut once, Package::Universal, &names);

        // Replay — register N times into a fresh registry.
        let mut many = ExtensionRegistry::new();
        for _ in 0..n {
            register_package(&mut many, Package::Universal, &names);
        }

        // Observable equivalence: presence/absence of every registered name
        // matches. (The closures inside `ExtensionFunc` are not comparable,
        // so we assert via the public `contains` / `get(...).is_some()`
        // surface — the only contract callers depend on.)
        for name in &names {
            prop_assert_eq!(
                once.contains(name),
                many.contains(name),
                "name {} differs after {} re-registrations", name, n
            );
            prop_assert_eq!(
                once.get(name).is_some(),
                many.get(name).is_some(),
                "lookup outcome for {} differs after {} re-registrations", name, n
            );
        }
    }

    /// Property 3 — Disjoint package union.
    ///
    /// Registering two packages with disjoint name sets exposes the union of
    /// both sets via `get()`. Intersection-on-conflict semantics (last-write-
    /// wins) are not asserted here — that is a separate property best covered
    /// by an example test where the conflict policy is the explicit subject.
    #[test]
    fn disjoint_packages_compose_to_union(
        sets in arb_disjoint_name_sets(),
    ) {
        let (a, b) = sets;
        // Strategy guarantees disjointness; assert it as a sanity guard so a
        // future change to the strategy can't silently weaken the property.
        prop_assert!(a.is_disjoint(&b));

        let mut registry = ExtensionRegistry::new();
        register_package(&mut registry, Package::Universal, &a);
        register_package(&mut registry, Package::Formspec, &b);

        // Every name in either package resolves.
        for name in a.union(&b) {
            prop_assert!(
                registry.get(name).is_some(),
                "union-member {name:?} must resolve"
            );
        }
    }
}
