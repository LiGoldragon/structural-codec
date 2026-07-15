//! The canonical Block→text writer. Kept in this crate for now; if it is later
//! judged to belong in raw-discovery (as the inverse of `Recognizer::recognize`),
//! that is its eventual home — but raw-discovery is NOT edited for it here (flagged
//! placement, per the mission). Expressed as a local extension trait so the writing
//! logic still lives on a data-bearing type (`Block`), never a free function.

use raw_discovery::Block;

/// Render a discovered block back to its canonical NOTA text.
pub trait CanonicalText {
    fn canonical_text(&self) -> String;
}

impl CanonicalText for Block {
    fn canonical_text(&self) -> String {
        match self {
            Block::Atom(atom) => atom.text().to_owned(),
            Block::PipeText(pipe) => format!("(|{}|)", pipe.text()),
            Block::Application { head, payload } => {
                format!("{}.{}", head.canonical_text(), payload.canonical_text())
            }
            Block::Delimited {
                delimiter,
                root_objects,
            } => delimiter.wrap(root_objects.iter().map(CanonicalText::canonical_text)),
        }
    }
}
