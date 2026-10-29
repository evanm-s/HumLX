//! DMX512 universe buffer.

/// Number of channels in a DMX universe.
pub const UNIVERSE_SIZE: usize = 512;

/// A DMX address, 1-based as shown to users (1..=512).
pub type Address = u16;

/// The 512 channel values of one universe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Universe {
    data: [u8; UNIVERSE_SIZE],
}

impl Default for Universe {
    fn default() -> Self {
        Self {
            data: [0; UNIVERSE_SIZE],
        }
    }
}

impl Universe {
    /// Sets a channel by its 1-based address. Out-of-range addresses are ignored.
    pub fn set(&mut self, address: Address, value: u8) {
        if let Some(slot) = Self::index(address).map(|i| &mut self.data[i]) {
            *slot = value;
        }
    }

    /// Reads a channel by its 1-based address. Out-of-range addresses read as 0.
    pub fn get(&self, address: Address) -> u8 {
        Self::index(address).map_or(0, |i| self.data[i])
    }

    pub fn as_bytes(&self) -> &[u8; UNIVERSE_SIZE] {
        &self.data
    }

    pub fn clear(&mut self) {
        self.data = [0; UNIVERSE_SIZE];
    }

    fn index(address: Address) -> Option<usize> {
        let index = usize::from(address).checked_sub(1)?;
        (index < UNIVERSE_SIZE).then_some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_one_based() {
        let mut universe = Universe::default();
        universe.set(1, 10);
        universe.set(512, 20);
        assert_eq!(universe.as_bytes()[0], 10);
        assert_eq!(universe.as_bytes()[511], 20);
        assert_eq!(universe.get(1), 10);
    }

    #[test]
    fn out_of_range_is_ignored() {
        let mut universe = Universe::default();
        universe.set(0, 10);
        universe.set(513, 10);
        assert_eq!(universe, Universe::default());
        assert_eq!(universe.get(0), 0);
        assert_eq!(universe.get(600), 0);
    }
}
