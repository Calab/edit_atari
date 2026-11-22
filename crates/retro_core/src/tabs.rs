//! Tab stops modelled on a screen editor's tab map.
//!
//! Stops are stored per *physical* screen column of a multi-row logical line
//! (the Atari's TABMAP is 120 bits: 3 rows x 40 columns). Text columns are
//! converted to physical positions using the machine's screen geometry, so
//! tabbing lands on the same characters whichever wrap mode is displayed.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabStops {
    /// One flag per physical position in the logical line.
    stops: Vec<bool>,
    /// Physical columns per screen row (40 on the Atari).
    phys_cols: usize,
    /// Left margin in physical columns (2 on the Atari).
    margin: usize,
    /// Rows in a logical line (3 on the Atari).
    rows: usize,
}

impl TabStops {
    /// Stops every `every` physical columns starting at `first`, across a
    /// logical line of `rows` rows of `phys_cols` columns.
    pub fn every(every: usize, first: usize, phys_cols: usize, margin: usize, rows: usize) -> Self {
        let mut stops = vec![false; phys_cols * rows];
        for p in (first..stops.len()).step_by(every.max(1)) {
            stops[p] = true;
        }
        Self {
            stops,
            phys_cols,
            margin,
            rows,
        }
    }

    fn text_cols(&self) -> usize {
        self.phys_cols - self.margin
    }

    /// Longest line (in characters) the logical line can hold.
    pub fn max_col(&self) -> usize {
        self.text_cols() * self.rows
    }

    /// Physical position of text column `col`, if it is inside the logical line.
    fn phys(&self, col: usize) -> Option<usize> {
        let row = col / self.text_cols();
        (row < self.rows).then(|| row * self.phys_cols + self.margin + col % self.text_cols())
    }

    /// The next stop after text column `col`, or `None` if there are no
    /// more stops in the logical line.
    pub fn next(&self, col: usize) -> Option<usize> {
        (col + 1..self.max_col()).find(|&c| self.phys(c).is_some_and(|p| self.stops[p]))
    }

    pub fn is_stop(&self, col: usize) -> bool {
        self.phys(col).is_some_and(|p| self.stops[p])
    }

    /// Set a stop at text column `col`. Returns false if `col` is outside the
    /// logical line.
    pub fn set(&mut self, col: usize) -> bool {
        self.phys(col).map(|p| self.stops[p] = true).is_some()
    }

    /// Clear the stop at text column `col`. Returns false if `col` is outside
    /// the logical line.
    pub fn clear(&mut self, col: usize) -> bool {
        self.phys(col).map(|p| self.stops[p] = false).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Atari power-up tab map: physical columns 7, 15, ... 119.
    fn atari() -> TabStops {
        TabStops::every(8, 7, 40, 2, 3)
    }

    #[test]
    fn atari_default_stops() {
        let t = atari();
        assert_eq!(t.max_col(), 114);
        let mut stops = vec![];
        let mut c = 0;
        while let Some(n) = t.next(c) {
            stops.push(n);
            c = n;
        }
        // Text columns 5,13,21,29,37 on each of the three 38-column rows.
        assert_eq!(
            stops,
            [5, 13, 21, 29, 37, 43, 51, 59, 67, 75, 81, 89, 97, 105, 113]
        );
        assert_eq!(t.next(113), None);
        assert_eq!(t.next(500), None);
    }

    #[test]
    fn set_and_clear() {
        let mut t = atari();
        assert!(t.set(2));
        assert_eq!(t.next(0), Some(2));
        assert!(t.clear(2));
        assert!(t.clear(5));
        assert_eq!(t.next(0), Some(13));
        assert!(!t.set(114));
        assert!(t.is_stop(13));
    }
}
