//! The truth-side identity contract for a language's strict encoded body.
//!
//! This narrow contract is restored by the 2026-08-07 `TrueNamed` ruling. It
//! intentionally carries no conversion, textual projection, or wrapper value:
//! an encoded form is the strict value whose portable rkyv bytes establish its
//! own true name.

use name_table::TrueNamed;
use rkyv::Deserialize as RkyvDeserialize;
use rkyv::api::high::HighDeserializer;
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::{self, Strategy};
use rkyv::validation::Validator;
use rkyv::validation::archive::ArchiveValidator;
use rkyv::validation::shared::SharedValidator;

/// A strict, name-free encoded value family for one language.
///
/// Its `TrueNamed` supertrait fixes identity to its existing portable rkyv
/// wire bytes. Implementors must keep their own `TextualName` out of the
/// value and carry references as `EncodedName` values.
pub trait EncodedForm: TrueNamed
where
    Self::Archived: RkyvDeserialize<Self, HighDeserializer<rancor::Error>>
        + for<'validation> CheckBytes<
            Strategy<Validator<ArchiveValidator<'validation>, SharedValidator>, rancor::Error>,
        >,
{
    /// The language this encoded value family belongs to.
    type Language;
}

#[cfg(test)]
mod tests {
    use name_table::{EncodedName, TrueNamed};

    use super::EncodedForm;

    enum TestLanguage {}

    #[derive(Clone, rkyv::Archive, rkyv::Deserialize, rkyv::Serialize)]
    struct TestBody {
        reference: EncodedName,
        payload: u16,
    }

    impl TrueNamed for TestBody {}

    impl EncodedForm for TestBody {
        type Language = TestLanguage;
    }

    #[test]
    fn encoded_form_requires_a_true_named_strict_body() {
        let body = TestBody {
            reference: EncodedName::from_archive_bytes([3; 16]),
            payload: 9,
        };

        assert!(body.true_name().is_ok());
    }
}
