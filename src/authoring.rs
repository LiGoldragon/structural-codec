//! Authoring helpers deliberately stop at typed records. There is no product
//! normalizer or duplicate delimiter field to leak into a table pre-image.

use raw_discovery::TriggerIdentifier;

use crate::error::AuthoringError;
use crate::form::{ApplicationDelimitedRule, SharedDescriptor, StructuralRule};

pub struct ApplicationDelimitedAuthoring<Root> {
    pub operator: TriggerIdentifier,
    pub boundary: TriggerIdentifier,
    pub head: SharedDescriptor<Root>,
    pub element: SharedDescriptor<Root>,
}

impl<Root> ApplicationDelimitedAuthoring<Root> {
    pub fn normalize(self) -> Result<StructuralRule<Root>, AuthoringError> {
        Ok(StructuralRule::ApplicationDelimited(
            ApplicationDelimitedRule::new(
                self.operator,
                self.boundary,
                self.head,
                self.element,
                1,
                Some(1),
            )?,
        ))
    }
}
