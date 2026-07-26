use crate::ConformanceHarness;
use crate::fixture::{COMMIT_SEQUENCE, FixtureBuilder};

#[test]
fn conformance_harness_requires_the_same_sealed_profile_as_direct_evaluation() {
    let table = FixtureBuilder::new().build().expect("table");
    let profile = FixtureBuilder::token_profile();
    let _harness =
        ConformanceHarness::new(&table, &profile, COMMIT_SEQUENCE).expect("profile bound harness");
}
