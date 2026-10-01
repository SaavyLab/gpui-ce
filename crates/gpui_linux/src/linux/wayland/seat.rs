#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SeatGlobal {
    pub name: u32,
    pub version: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SeatRemoval {
    /// The seat in use is unaffected.
    Unaffected,
    /// The seat in use is gone, and `replacement` takes its place if one is left.
    InUse { replacement: Option<SeatGlobal> },
}

/// Helper for choosing which of the compositor's seats the client uses.
///
/// The client uses the first seat advertised and keeps it for as long as it exists. A seat
/// added later, by the compositor or one of its plugins, must not take input away from it.
#[derive(Debug)]
pub(crate) struct SeatSelection {
    // Oldest first
    advertised: Vec<SeatGlobal>,
    in_use: Option<u32>,
}

impl SeatSelection {
    pub fn new() -> Self {
        Self {
            advertised: Vec::new(),
            in_use: None,
        }
    }

    /// Returns the seat if the client should start using it.
    pub fn add(&mut self, seat: SeatGlobal) -> Option<SeatGlobal> {
        self.advertised.push(seat);
        if self.in_use.is_some() {
            return None;
        }
        self.in_use = Some(seat.name);
        Some(seat)
    }

    /// Handles the removal of the global `name`, which is not necessarily a seat.
    pub fn remove(&mut self, name: u32) -> SeatRemoval {
        self.advertised.retain(|seat| seat.name != name);
        if self.in_use != Some(name) {
            return SeatRemoval::Unaffected;
        }
        let replacement = self.advertised.first().copied();
        self.in_use = replacement.map(|seat| seat.name);
        SeatRemoval::InUse { replacement }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(name: u32) -> SeatGlobal {
        SeatGlobal { name, version: 9 }
    }

    #[test]
    fn test_uses_the_first_of_several_seats() {
        let mut selection = SeatSelection::new();

        assert_eq!(selection.add(seat(10)), Some(seat(10)));
        assert_eq!(selection.add(seat(11)), None);
        assert_eq!(selection.add(seat(12)), None);
    }

    #[test]
    fn test_keeps_its_seat_when_another_is_added() {
        let mut selection = SeatSelection::new();
        selection.add(seat(10));

        assert_eq!(selection.add(seat(11)), None);
        // Removing the newcomer shows that the first seat is still the one in use.
        assert_eq!(selection.remove(11), SeatRemoval::Unaffected);
    }

    #[test]
    fn test_replaces_a_removed_seat_with_the_oldest_remaining() {
        let mut selection = SeatSelection::new();
        selection.add(seat(10));
        selection.add(seat(11));
        selection.add(seat(12));

        assert_eq!(
            selection.remove(10),
            SeatRemoval::InUse {
                replacement: Some(seat(11))
            }
        );
        assert_eq!(
            selection.remove(11),
            SeatRemoval::InUse {
                replacement: Some(seat(12))
            }
        );
    }

    #[test]
    fn test_never_replaces_with_a_seat_that_was_removed() {
        let mut selection = SeatSelection::new();
        selection.add(seat(10));
        selection.add(seat(11));
        selection.add(seat(12));

        assert_eq!(selection.remove(11), SeatRemoval::Unaffected);
        assert_eq!(
            selection.remove(10),
            SeatRemoval::InUse {
                replacement: Some(seat(12))
            }
        );
    }

    #[test]
    fn test_uses_the_next_seat_after_the_last_one_is_removed() {
        let mut selection = SeatSelection::new();
        selection.add(seat(10));

        assert_eq!(
            selection.remove(10),
            SeatRemoval::InUse { replacement: None }
        );
        assert_eq!(selection.add(seat(11)), Some(seat(11)));
    }
}
