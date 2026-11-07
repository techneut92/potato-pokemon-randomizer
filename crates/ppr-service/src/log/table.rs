//! A table of text, columns as wide as their widest cell and separated by `|`.

use std::fmt;

/// A table of left-aligned text cells. A cell is `None` until it is filled or the columns are evened out.
#[derive(Debug)]
pub(super) struct TextTable {
    columns: usize,
    rows: Vec<Vec<Option<String>>>,
}

impl TextTable {
    pub(super) fn new(columns: usize) -> Self {
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    /// Adds a row. Evens out the columns first, so the row starts below every column.
    pub(super) fn add_row<S: Into<String>>(&mut self, row: impl IntoIterator<Item = S>) {
        self.even_out();
        let row: Vec<Option<String>> = row.into_iter().map(|s| Some(s.into())).collect();
        debug_assert_eq!(row.len(), self.columns, "a row has a cell per column");
        self.rows.push(row);
    }

    /// Adds a cell below the last filled cell of column `col`.
    pub(super) fn add_cell(&mut self, col: usize, cell: impl Into<String>) {
        let row = if let Some(row) = self.rows.iter().position(|r| r[col].is_none()) {
            row
        } else {
            self.rows.push(vec![None; self.columns]);
            self.rows.len() - 1
        };
        self.rows[row][col] = Some(cell.into());
    }

    /// Unfilled cells become empty, so every column continues from the same row.
    pub(super) fn even_out(&mut self) {
        for cell in self.rows.iter_mut().flatten() {
            cell.get_or_insert_with(String::new);
        }
    }
}

impl fmt::Display for TextTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = |c: &Option<String>| c.as_deref().unwrap_or_default().chars().count();
        let widths: Vec<usize> = (0..self.columns)
            .map(|col| self.rows.iter().map(|r| text(&r[col])).max().unwrap_or(0))
            .collect();
        for row in &self.rows {
            for (col, cell) in row.iter().enumerate() {
                write!(f, "{:<w$}", cell.as_deref().unwrap_or_default(), w = widths[col])?;
                if col + 1 != self.columns {
                    f.write_str("|")?;
                }
            }
            f.write_str("\n")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_stack_per_column_until_evened_out() {
        let mut t = TextTable::new(3);
        t.add_row(["FROM", "TO", "METHOD"]);
        t.add_cell(0, "Eevee");
        t.add_cell(1, "Vaporeon");
        t.add_cell(1, "Jolteon");
        t.add_cell(2, "Use Water Stone");
        t.even_out();
        t.add_cell(1, "Flareon");
        assert_eq!(
            t.to_string(),
            "FROM |TO      |METHOD         \n\
             Eevee|Vaporeon|Use Water Stone\n     \
             |Jolteon |               \n     \
             |Flareon |               \n"
        );
    }
}
