//! Which Place this is (ADR-0028).
//!
//! A Place's identity, and the reason it is in the Kernel rather than in the Engine.
//!
//! ADR-0021 designed this split — `PlaceConcept` classifies, `PlaceId` identifies — and ADR-0023
//! cited it as the case already done correctly when it made the same split for characters. The
//! build never received it. `PlaceId` existed only in the Engine's asset pipeline, as a handle
//! for resolving a pack's declarations into a composition, while everything about a character's
//! life was keyed on the concept.
//!
//! So `PresenceState::place` and `PresenceProfile::home` were a `PlaceConcept`, and two
//! characters could not live in two different laboratories because "laboratory" *was* the
//! address. That is precisely the defect ADR-0023 named for characters, one field below the
//! comment explaining why it must not happen.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A Place's identity. Stable for the Place's whole lifetime.
///
/// Name, artwork, concept, occupants, position and vitality may all change. This does not.
/// Everything inside Epoch references a Place by identity, never by appearance — which is what
/// lets a user rename a building without a road, a home or a piece of History pointing at
/// nothing afterwards (ADR-0022, ADR-0028).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlaceId(String);

impl PlaceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PlaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for PlaceId {
    fn from(id: &str) -> Self {
        Self::new(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_survives_serialisation_as_a_plain_string() {
        // `transparent`, so a vault file reads `home = "building_1"` rather than
        // `home = { 0 = "building_1" }`. The vault is meant to be opened and edited by hand.
        let id = PlaceId::new("building_1");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"building_1\"");
        assert_eq!(serde_json::from_str::<PlaceId>(&json).unwrap(), id);
    }

    #[test]
    fn two_places_of_the_same_kind_are_two_places() {
        // The whole point. Under the old model both of these were `PlaceConcept::ResearchLab`
        // and therefore the same address, so a second laboratory silently replaced the first.
        assert_ne!(PlaceId::new("building_1"), PlaceId::new("building_4"));
    }
}
