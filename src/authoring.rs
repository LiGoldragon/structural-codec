//! Authoring helpers deliberately stop at typed records. There is no product
//! normalizer or duplicate delimiter field to leak into a table pre-image.

use raw_discovery::TriggerIdentifier;

use crate::error::AuthoringError;
use crate::form::{ApplicationDelimitedRule, AtomDescriptor, SharedDescriptor, StructuralRule};

pub struct ApplicationDelimitedAuthoring {
    pub operator: TriggerIdentifier,
    pub boundary: TriggerIdentifier,
    pub head: AtomDescriptor,
    pub element: SharedDescriptor,
}

impl ApplicationDelimitedAuthoring {
    pub fn normalize(self) -> Result<StructuralRule, AuthoringError> {
        Ok(StructuralRule::ApplicationDelimited(
            ApplicationDelimitedRule::new(
                self.operator,
                self.boundary,
                SharedDescriptor::Atom(self.head),
                self.element,
                1,
                Some(1),
            )?,
        ))
    }
}
