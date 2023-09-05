#![no_std]
extern crate std as _no_std;
use wasm_bindgen::prelude::*;

const SQRT_N: usize = 3;
const N: usize = SQRT_N * SQRT_N;
const VARS: usize = N * N * N;
const CRUMBS: usize = 32 / 2;
const UNITS: usize = VARS / CRUMBS + 1;

// Extended CNF encoding (9x9)
const LITERALS: usize = 26_973;
const CLAUSES: usize = 12_717;

const fn assert(condition: bool) -> Result<(), ()> {
    if condition {
        Ok(())
    } else {
        Err(())
    }
}

struct Sudoku {
    next_literal: usize,
    next_clause: usize,
    units: [u32; UNITS],
}

const fn index(row: usize, column: usize, value: usize) -> usize {
    row.wrapping_mul(N)
        .wrapping_add(column)
        .wrapping_mul(N)
        .wrapping_add(value)
}
const fn is(index: usize) -> u16 {
    ((index << 1) | 1) as u16
}
const fn not(index: usize) -> u16 {
    (index << 1) as u16
}

impl Sudoku {
    #[inline]
    fn set(&mut self, index: usize, value: bool) {
        let index = VARS.min(index);
        //SAFETY: `units` is sized so that this is in range.
        unsafe {
            *self.units.get_unchecked_mut(index / CRUMBS) |=
                1u32 << (index % CRUMBS * 2 + value as usize);
        }
    }
    #[inline]
    fn get(&self, index: usize) -> u32 {
        let index = VARS.min(index);
        //SAFETY: `units` is sized so that this is in range.
        unsafe { (*self.units.get_unchecked(index / CRUMBS) >> (index % CRUMBS * 2)) & 3 }
    }
    fn assign(&mut self, row: usize, column: usize, value: usize) {
        self.set(index(row, column, value), true);
        for row in (0..N).filter(|&i| i != row) {
            self.set(index(row, column, value), false);
        }
        for column in (0..N).filter(|&i| i != column) {
            self.set(index(row, column, value), false);
        }
        for value in (0..N).filter(|&i| i != value) {
            self.set(index(row, column, value), false);
        }
        let block_row = row - row % SQRT_N;
        let block_column = column - column % SQRT_N;
        for block_row in block_row..block_row + SQRT_N {
            for block_column in block_column..block_column + SQRT_N {
                if block_row != row || block_column != column {
                    self.set(index(block_row, block_column, value), false);
                }
            }
        }
    }
    fn try_insert(&mut self, clause: &[u16]) -> Result<(), ()> {
        let first_literal = self.next_literal;
        let next_clause = self.next_clause;
        let mut next_literal = first_literal;
        for &literal in clause {
            let value = self.get((literal >> 1) as usize);
            if value == 0 {
                assert(next_literal < LITERALS)?;
            }
            next_literal += 1;
        }
        assert(next_clause < CLAUSES)?;
        self.next_literal = next_literal;
        self.next_clause = next_clause + 1;
        Ok(())
    }
    fn cell_clauses(&mut self, row: usize, column: usize) -> Result<(), ()> {
        // Uniqueness
        for high_value in 1..N {
            for low_value in 0..high_value {
                self.try_insert(&[
                    not(index(row, column, low_value)),
                    not(index(row, column, high_value)),
                ])?;
            }
        }
        // Definedness
        let mut clause = [0; N];
        for (value, literal) in clause.iter_mut().enumerate() {
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn row_clauses(&mut self, row: usize, value: usize) -> Result<(), ()> {
        // Uniqueness
        for high_column in 1..N {
            for low_column in 0..high_column {
                self.try_insert(&[
                    not(index(row, low_column, value)),
                    not(index(row, high_column, value)),
                ])?;
            }
        }
        // Definedness
        let mut clause = [0; N];
        for (column, literal) in clause.iter_mut().enumerate() {
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn column_clauses(&mut self, column: usize, value: usize) -> Result<(), ()> {
        // Uniqueness
        for high_row in 1..N {
            for low_row in 0..high_row {
                self.try_insert(&[
                    not(index(low_row, column, value)),
                    not(index(high_row, column, value)),
                ])?;
            }
        }
        // Definedness
        let mut clause = [0; N];
        for (row, literal) in clause.iter_mut().enumerate() {
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn block_clauses(
        &mut self,
        block_row: usize,
        block_column: usize,
        value: usize,
    ) -> Result<(), ()> {
        // Uniqueness
        for high_offset in 1..N {
            let high_row = block_row + high_offset / SQRT_N;
            let high_column = block_column + high_offset % SQRT_N;
            for low_offset in 0..high_offset {
                let low_row = block_row + low_offset / SQRT_N;
                let low_column = block_column + low_offset % SQRT_N;
                self.try_insert(&[
                    not(index(low_row, low_column, value)),
                    not(index(high_row, high_column, value)),
                ])?;
            }
        }
        // Definedness
        let mut clause = [0; N];
        for (offset, subclause) in clause.chunks_exact_mut(SQRT_N).enumerate() {
            let row = block_row + offset;
            for (offset, literal) in subclause.iter_mut().enumerate() {
                let column = block_column + offset;
                *literal = is(index(row, column, value));
            }
        }
        self.try_insert(&clause)
    }
    fn generate_clauses(&mut self) -> Result<(), ()> {
        // Cells
        for row in 0..N {
            for column in 0..N {
                self.cell_clauses(row, column)?;
            }
        }
        // Rows
        for row in 0..N {
            for value in 0..N {
                self.row_clauses(row, value)?;
            }
        }
        // Columns
        for column in 0..N {
            for value in 0..N {
                self.column_clauses(column, value)?;
            }
        }
        // Blocks
        for block_row in (0..N).step_by(SQRT_N) {
            for block_column in (0..N).step_by(SQRT_N) {
                for value in 0..N {
                    self.block_clauses(block_row, block_column, value)?;
                }
            }
        }
        Ok(())
    }
}

static mut SUDOKU: Sudoku = Sudoku {
    next_literal: 0,
    next_clause: 0,
    units: [0; UNITS],
};

#[wasm_bindgen]
pub fn assign(index: usize) {
    //SAFETY: Not guaranteed yet.
    unsafe {
        if SUDOKU.get(index) != 0 {
            return;
        }
        let row = index / (N * N);
        let column = index / N % N;
        let value = index % N;
        SUDOKU.assign(row, column, value);
    }
}

#[wasm_bindgen]
pub fn units_ptr() -> *const u32 {
    //SAFETY: Short-lived immutable view.
    unsafe { SUDOKU.units.as_ptr() }
}

#[wasm_bindgen]
pub fn units_len() -> usize {
    UNITS
}

#[wasm_bindgen]
pub fn literals() -> usize {
    //SAFETY: Primitive value.
    unsafe { SUDOKU.next_literal }
}

#[wasm_bindgen]
pub fn clauses() -> usize {
    //SAFETY: Primitive value.
    unsafe { SUDOKU.next_clause }
}

#[wasm_bindgen(start)]
pub fn start() {
    // SAFETY: Entrypoint; no concurrent access.
    unsafe {
        SUDOKU.assign(3, 4, 8);
        let _ = SUDOKU.generate_clauses();
    }
}
