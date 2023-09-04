#![no_std]
extern crate std as _no_std;
use wasm_bindgen::prelude::*;

const SQRT_N: usize = 3;
const N: usize = SQRT_N * SQRT_N;
const VARS: usize = N * N * N;
const CRUMBS: usize = 32 / 2;
const UNITS: usize = VARS / CRUMBS + 1;

struct Sudoku {
    units: [u32; UNITS],
}

const fn index(row: usize, column: usize, value: usize) -> usize {
    row.wrapping_mul(N)
        .wrapping_add(column)
        .wrapping_mul(N)
        .wrapping_add(value)
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
}

static mut SUDOKU: Sudoku = Sudoku { units: [0; UNITS] };

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

#[wasm_bindgen(start)]
pub fn start() {
    // SAFETY: Entrypoint; no concurrent access.
    unsafe {
        SUDOKU.assign(3, 4, 8);
    }
}
