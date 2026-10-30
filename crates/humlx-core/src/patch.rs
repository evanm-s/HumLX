//! The patch: which fixtures exist and where they sit in the universe.

use serde::{Deserialize, Serialize};

use crate::dmx::{Address, UNIVERSE_SIZE};
use crate::error::{Error, Result};
use crate::fixture::{Library, Mode};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchedFixture {
    /// The fixture number people use to select it, unique within the patch.
    pub id: u32,
    pub name: String,
    /// Id of the fixture's profile in the library.
    pub profile: String,
    pub mode: String,
    /// First DMX channel, 1..=512.
    pub address: Address,
}

/// A request to patch one or more fixtures of the same type.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFixtures {
    pub profile: String,
    pub mode: String,
    pub count: u32,
    /// Number for the first fixture; the rest follow on. Defaults to the
    /// next number after the highest in use.
    pub first_id: Option<u32>,
    /// Address for the first fixture; the rest follow on back to back.
    /// Defaults to the first free block big enough for all of them.
    pub address: Option<Address>,
    /// Name for the fixtures, numbered when more than one is added.
    /// Defaults to the profile's model.
    pub name: Option<String>,
}

/// Changes to one patched fixture. Fields left as `None` are unchanged.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureChanges {
    pub id: Option<u32>,
    pub name: Option<String>,
    pub mode: Option<String>,
    pub address: Option<Address>,
}

/// Changes to the fixture currently numbered `id`, as part of a bulk edit.
#[derive(Debug, Clone, Deserialize)]
pub struct FixtureUpdate {
    pub id: u32,
    pub changes: FixtureChanges,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Patch {
    fixtures: Vec<PatchedFixture>,
}

impl Patch {
    pub fn fixtures(&self) -> &[PatchedFixture] {
        &self.fixtures
    }

    pub fn get(&self, id: u32) -> Option<&PatchedFixture> {
        self.fixtures.iter().find(|f| f.id == id)
    }

    /// Patches new fixtures and returns their numbers. Nothing is added if
    /// any of them would be invalid.
    pub fn add(&mut self, library: &Library, request: &NewFixtures) -> Result<Vec<u32>> {
        let profile = library
            .get(&request.profile)
            .ok_or_else(|| Error::UnknownProfile(request.profile.clone()))?;
        let footprint = library.mode(&request.profile, &request.mode)?.channels.len();
        let count = request.count.max(1);
        let total = footprint * count as usize;

        let first_id = match request.first_id {
            Some(0) => return Err(Error::InvalidFixtureId),
            Some(id) => id,
            None => self.fixtures.iter().map(|f| f.id).max().unwrap_or(0) + 1,
        };
        let first_address = match request.address {
            Some(address) => address,
            None => self.find_free(library, total).ok_or(Error::UniverseFull(total))?,
        };

        let base_name = request
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or(&profile.model)
            .to_string();

        let mut candidate = self.clone();
        let mut ids = Vec::with_capacity(count as usize);
        for i in 0..count {
            let id = first_id + i;
            let offset = Address::try_from(footprint * i as usize).unwrap_or(Address::MAX);
            let fixture = PatchedFixture {
                id,
                name: if count > 1 {
                    format!("{base_name} {}", i + 1)
                } else {
                    base_name.clone()
                },
                profile: request.profile.clone(),
                mode: request.mode.clone(),
                address: first_address.saturating_add(offset),
            };
            candidate.check(library, &fixture, None)?;
            candidate.fixtures.push(fixture);
            ids.push(id);
        }
        candidate.sort();
        *self = candidate;
        Ok(ids)
    }

    pub fn update(&mut self, library: &Library, id: u32, changes: &FixtureChanges) -> Result<()> {
        let current = self.get(id).ok_or(Error::UnknownFixture(id))?;
        let mut updated = current.clone();
        if let Some(new_id) = changes.id {
            updated.id = new_id;
        }
        if let Some(name) = &changes.name {
            updated.name = name.trim().to_string();
        }
        if let Some(mode) = &changes.mode {
            updated.mode = mode.clone();
        }
        if let Some(address) = changes.address {
            updated.address = address;
        }
        self.check(library, &updated, Some(id))?;
        if let Some(slot) = self.fixtures.iter_mut().find(|f| f.id == id) {
            *slot = updated;
        }
        self.sort();
        Ok(())
    }

    /// Applies changes to several fixtures at once, keyed by their current
    /// numbers. The changes are checked together, so fixtures can swap
    /// numbers or shuffle addresses; nothing changes if any would clash.
    pub fn update_many(&mut self, library: &Library, updates: &[FixtureUpdate]) -> Result<()> {
        let mut candidate = self.clone();
        let mut changed = Vec::with_capacity(updates.len());
        for update in updates {
            let index = self
                .fixtures
                .iter()
                .position(|f| f.id == update.id)
                .ok_or(Error::UnknownFixture(update.id))?;
            let fixture = &mut candidate.fixtures[index];
            let changes = &update.changes;
            if let Some(new_id) = changes.id {
                fixture.id = new_id;
            }
            if let Some(name) = &changes.name {
                fixture.name = name.trim().to_string();
            }
            if let Some(mode) = &changes.mode {
                fixture.mode = mode.clone();
            }
            if let Some(address) = changes.address {
                fixture.address = address;
            }
            changed.push(index);
        }
        for &index in &changed {
            let fixture = &candidate.fixtures[index];
            if candidate.fixtures.iter().filter(|f| f.id == fixture.id).count() > 1 {
                return Err(Error::DuplicateFixtureId(fixture.id));
            }
            candidate.check(library, fixture, Some(fixture.id))?;
        }
        candidate.sort();
        *self = candidate;
        Ok(())
    }

    pub fn remove(&mut self, ids: &[u32]) {
        self.fixtures.retain(|f| !ids.contains(&f.id));
    }

    /// Which fixture owns each channel of the universe (index 0 is address 1).
    pub fn occupancy(&self, library: &Library) -> Vec<Option<u32>> {
        let mut owners = vec![None; UNIVERSE_SIZE];
        for fixture in &self.fixtures {
            let footprint = footprint(library, fixture).unwrap_or(0);
            let start = usize::from(fixture.address).saturating_sub(1);
            for owner in owners.iter_mut().skip(start).take(footprint) {
                *owner = Some(fixture.id);
            }
        }
        owners
    }

    /// The first address with `channels` free channels in a row.
    pub fn find_free(&self, library: &Library, channels: usize) -> Option<Address> {
        if channels == 0 || channels > UNIVERSE_SIZE {
            return None;
        }
        let owners = self.occupancy(library);
        let mut run = 0;
        for (index, owner) in owners.iter().enumerate() {
            run = if owner.is_none() { run + 1 } else { 0 };
            if run == channels {
                return Address::try_from(index + 2 - channels).ok();
            }
        }
        None
    }

    /// Checks `fixture` could join the patch, ignoring the fixture currently
    /// numbered `replacing` (when it is being edited).
    fn check(&self, library: &Library, fixture: &PatchedFixture, replacing: Option<u32>) -> Result<()> {
        if fixture.id == 0 {
            return Err(Error::InvalidFixtureId);
        }
        let mode = library.mode(&fixture.profile, &fixture.mode)?;
        let others = || self.fixtures.iter().filter(|f| Some(f.id) != replacing);
        if others().any(|f| f.id == fixture.id) {
            return Err(Error::DuplicateFixtureId(fixture.id));
        }

        let start = usize::from(fixture.address);
        let end = start + mode.channels.len() - 1;
        if start == 0 || end > UNIVERSE_SIZE {
            return Err(Error::AddressOutOfRange {
                address: fixture.address,
                footprint: mode.channels.len(),
            });
        }
        for other in others() {
            let other_start = usize::from(other.address);
            let other_end = other_start + footprint(library, other).unwrap_or(1) - 1;
            if start <= other_end && other_start <= end {
                return Err(Error::AddressOverlap {
                    address: fixture.address,
                    other: other.id,
                });
            }
        }
        Ok(())
    }

    fn sort(&mut self) {
        self.fixtures.sort_by_key(|f| f.id);
    }
}

/// The mode a patched fixture uses, if its profile is still in the library.
pub fn mode_of<'a>(library: &'a Library, fixture: &PatchedFixture) -> Option<&'a Mode> {
    library.mode(&fixture.profile, &fixture.mode).ok()
}

fn footprint(library: &Library, fixture: &PatchedFixture) -> Option<usize> {
    mode_of(library, fixture).map(|mode| mode.channels.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(count: u32, address: Option<Address>) -> NewFixtures {
        NewFixtures {
            profile: "generic/rgbw-par".into(),
            mode: "6ch DRGBW Strobe".into(),
            count,
            first_id: None,
            address,
            name: None,
        }
    }

    #[test]
    fn adds_fixtures_back_to_back() {
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        let ids = patch.add(&library, &request(3, Some(10))).unwrap();
        assert_eq!(ids, vec![1, 2, 3]);
        let addresses: Vec<_> = patch.fixtures().iter().map(|f| f.address).collect();
        assert_eq!(addresses, vec![10, 16, 22]);
        assert_eq!(patch.get(2).unwrap().name, "RGBW Par 2");
    }

    #[test]
    fn finds_free_space_and_next_number() {
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        patch.add(&library, &request(1, Some(1))).unwrap();
        patch.add(&library, &request(1, Some(13))).unwrap();
        // 6 free channels at 7..=12 fit exactly.
        let ids = patch.add(&library, &request(1, None)).unwrap();
        assert_eq!(ids, vec![3]);
        assert_eq!(patch.get(3).unwrap().address, 7);
        // 12 channels do not fit in a gap, so they go after the last fixture.
        patch.add(&library, &request(2, None)).unwrap();
        assert_eq!(patch.get(4).unwrap().address, 19);
    }

    #[test]
    fn rejects_overlaps_and_leaves_patch_unchanged() {
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        patch.add(&library, &request(1, Some(1))).unwrap();
        let before = patch.clone();
        let err = patch.add(&library, &request(2, Some(510))).unwrap_err();
        assert!(matches!(err, Error::AddressOutOfRange { .. }), "{err}");
        let err = patch.add(&library, &request(1, Some(6))).unwrap_err();
        assert!(matches!(err, Error::AddressOverlap { other: 1, .. }), "{err}");
        assert_eq!(patch, before);
    }

    #[test]
    fn rejects_duplicate_numbers() {
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        patch.add(&library, &request(2, None)).unwrap();
        let mut again = request(1, None);
        again.first_id = Some(2);
        assert!(matches!(patch.add(&library, &again), Err(Error::DuplicateFixtureId(2))));
    }

    #[test]
    fn updates_fixture() {
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        patch.add(&library, &request(2, Some(1))).unwrap();

        // Moving onto itself is fine; moving onto a neighbour is not.
        let changes = FixtureChanges {
            address: Some(3),
            ..Default::default()
        };
        assert!(patch.update(&library, 1, &changes).is_err());
        let changes = FixtureChanges {
            id: Some(10),
            mode: Some("4ch RGBW".into()),
            name: Some(" Front left ".into()),
            ..Default::default()
        };
        patch.update(&library, 1, &changes).unwrap();
        let fixture = patch.get(10).unwrap();
        assert_eq!(fixture.name, "Front left");
        assert_eq!(fixture.mode, "4ch RGBW");
        assert_eq!(patch.fixtures().last().unwrap().id, 10);
    }

    #[test]
    fn updates_many_together() {
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        patch.add(&library, &request(3, Some(1))).unwrap();
        let update = |id, new_id, address| FixtureUpdate {
            id,
            changes: FixtureChanges {
                id: Some(new_id),
                address: Some(address),
                ..Default::default()
            },
        };

        // Swapping numbers and addresses only works when checked together.
        patch
            .update_many(&library, &[update(1, 2, 7), update(2, 1, 1)])
            .unwrap();
        assert_eq!(patch.get(1).unwrap().name, "RGBW Par 2");
        assert_eq!(patch.get(1).unwrap().address, 1);
        assert_eq!(patch.get(2).unwrap().address, 7);

        // A clash with a fixture outside the edit leaves the patch unchanged.
        let before = patch.clone();
        let err = patch
            .update_many(&library, &[update(1, 3, 100), update(2, 4, 200)])
            .unwrap_err();
        assert!(matches!(err, Error::DuplicateFixtureId(3)), "{err}");
        let err = patch
            .update_many(&library, &[update(1, 10, 13), update(2, 11, 50)])
            .unwrap_err();
        assert!(matches!(err, Error::AddressOverlap { other: 3, .. }), "{err}");
        assert_eq!(patch, before);
    }

    #[test]
    fn occupancy_and_remove() {
        let library = Library::with_bundled();
        let mut patch = Patch::default();
        patch.add(&library, &request(2, Some(1))).unwrap();
        let owners = patch.occupancy(&library);
        assert_eq!(owners[0], Some(1));
        assert_eq!(owners[6], Some(2));
        assert_eq!(owners[12], None);
        patch.remove(&[1]);
        assert_eq!(patch.occupancy(&library)[0], None);
    }
}
