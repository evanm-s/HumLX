//! Where cue lists sit on the Playback screen: the button grid, the fader
//! slots, and which list is selected. Cue lists themselves live in the show's
//! library of lists; this only refers to them by number.

use serde::{Deserialize, Serialize};

use crate::playback::CueList;

/// Buttons on each grid page: 8 columns by 6 rows.
pub const GRID_COLUMNS: usize = 8;
pub const GRID_ROWS: usize = 6;
pub const GRID_BUTTONS: usize = GRID_COLUMNS * GRID_ROWS;
/// Fader slots in the Playback screen's lower panel.
pub const SLOTS: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPage {
    #[serde(default)]
    pub name: String,
    /// The cue list on each button, row by row, or none.
    #[serde(default = "empty_buttons")]
    pub buttons: Vec<Option<u32>>,
}

impl Default for GridPage {
    fn default() -> Self {
        Self {
            name: String::new(),
            buttons: empty_buttons(),
        }
    }
}

fn empty_buttons() -> Vec<Option<u32>> {
    vec![None; GRID_BUTTONS]
}

fn empty_slots() -> Vec<Option<u32>> {
    vec![None; SLOTS]
}

fn one_page() -> Vec<GridPage> {
    vec![GridPage::default()]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// Pages of the button grid; there is always at least one.
    #[serde(default = "one_page")]
    pub pages: Vec<GridPage>,
    /// The cue list in each fader slot, or none.
    #[serde(default = "empty_slots")]
    pub slots: Vec<Option<u32>>,
    /// The list that GO, Back, the NX-K's Next and Last and Record act on.
    #[serde(default)]
    pub selected: Option<u32>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            pages: one_page(),
            slots: empty_slots(),
            selected: None,
        }
    }
}

impl Layout {
    /// A layout for a show saved before there was one: its lists go into the
    /// fader slots in number order, as each list had its own fader, and lists
    /// with a single cue also go onto the first grid page, ready to tap.
    pub fn migrate(lists: &[CueList]) -> Self {
        let mut layout = Self::default();
        let mut ids: Vec<&CueList> = lists.iter().collect();
        ids.sort_by_key(|l| l.id);
        for (slot, list) in layout.slots.iter_mut().zip(&ids) {
            *slot = Some(list.id);
        }
        let looks = ids.iter().filter(|l| l.cues.len() == 1);
        for (button, list) in layout.pages[0].buttons.iter_mut().zip(looks) {
            *button = Some(list.id);
        }
        layout.selected = ids.first().map(|l| l.id);
        layout
    }

    /// Makes the layout well formed after loading: at least one page, every
    /// page and the slots the right size, and no references to missing lists.
    pub fn tidy(&mut self, exists: impl Fn(u32) -> bool) {
        if self.pages.is_empty() {
            self.pages = one_page();
        }
        for page in &mut self.pages {
            page.buttons.resize(GRID_BUTTONS, None);
        }
        self.slots.resize(SLOTS, None);
        let keep = |id: &mut Option<u32>| {
            if id.is_some_and(|i| !exists(i)) {
                *id = None;
            }
        };
        self.pages.iter_mut().flat_map(|p| &mut p.buttons).for_each(keep);
        self.slots.iter_mut().for_each(keep);
        keep(&mut self.selected);
    }

    /// Takes a deleted list off every button and slot.
    pub fn forget(&mut self, list: u32) {
        self.tidy(|id| id != list);
    }

    /// The cue list on grid button `button` (from 0) of page `page`, to change.
    pub fn button_mut(&mut self, page: usize, button: usize) -> Option<&mut Option<u32>> {
        self.pages.get_mut(page)?.buttons.get_mut(button)
    }

    /// Swaps two grid buttons, on the same page or different ones. Returns
    /// false if either doesn't exist.
    pub fn swap_buttons(&mut self, a: (usize, usize), b: (usize, usize)) -> bool {
        let (Some(x), Some(y)) = (
            self.button_mut(a.0, a.1).map(|v| *v),
            self.button_mut(b.0, b.1).map(|v| *v),
        ) else {
            return false;
        };
        *self.button_mut(a.0, a.1).unwrap() = y;
        *self.button_mut(b.0, b.1).unwrap() = x;
        true
    }

    /// Adds an empty grid page at the end and returns its index.
    pub fn add_page(&mut self, name: &str) -> usize {
        self.pages.push(GridPage {
            name: name.trim().to_string(),
            ..Default::default()
        });
        self.pages.len() - 1
    }

    /// Removes a grid page, leaving its lists in the library. The last page
    /// can't be removed. Returns false if nothing was removed.
    pub fn remove_page(&mut self, page: usize) -> bool {
        if self.pages.len() <= 1 || page >= self.pages.len() {
            return false;
        }
        self.pages.remove(page);
        true
    }

    /// The first empty fader slot.
    pub fn free_slot(&self) -> Option<usize> {
        self.slots.iter().position(Option::is_none)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::Cue;

    fn list(id: u32, cues: usize) -> CueList {
        CueList {
            id,
            name: format!("List {id}"),
            cues: vec![Cue::default(); cues],
            ..Default::default()
        }
    }

    #[test]
    fn migrates_lists_to_slots_and_looks_to_the_grid() {
        let lists: Vec<CueList> = [(3, 1), (1, 4), (2, 1)]
            .into_iter()
            .chain((4..=10).map(|id| (id, 2)))
            .map(|(id, cues)| list(id, cues))
            .collect();
        let layout = Layout::migrate(&lists);
        assert_eq!(layout.slots, (1..=8).map(Some).collect::<Vec<_>>());
        assert_eq!(&layout.pages[0].buttons[..3], &[Some(2), Some(3), None]);
        assert_eq!(layout.selected, Some(1));
    }

    #[test]
    fn swaps_buttons_and_manages_pages() {
        let mut layout = Layout::default();
        *layout.button_mut(0, 0).unwrap() = Some(1);
        let page = layout.add_page(" Show B ");
        assert_eq!((page, layout.pages[1].name.as_str()), (1, "Show B"));
        assert!(layout.swap_buttons((0, 0), (1, 47)));
        assert_eq!(
            (layout.pages[0].buttons[0], layout.pages[1].buttons[47]),
            (None, Some(1))
        );
        assert!(!layout.swap_buttons((0, 0), (2, 0)));
        assert!(layout.remove_page(0));
        assert!(!layout.remove_page(0));
        assert_eq!(layout.pages[0].buttons[47], Some(1));
    }

    #[test]
    fn tidy_fixes_sizes_and_drops_missing_lists() {
        let mut layout = Layout {
            pages: vec![],
            slots: vec![Some(1), Some(9)],
            selected: Some(9),
        };
        layout.tidy(|id| id == 1);
        assert_eq!(layout.pages.len(), 1);
        assert_eq!(layout.pages[0].buttons.len(), GRID_BUTTONS);
        assert_eq!(layout.slots.len(), SLOTS);
        assert_eq!(&layout.slots[..2], &[Some(1), None]);
        assert_eq!(layout.selected, None);
        assert_eq!(layout.free_slot(), Some(1));
    }
}
