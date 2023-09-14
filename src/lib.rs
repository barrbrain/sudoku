#![no_std]
use core::mem::MaybeUninit;
use wasm_bindgen::prelude::*;

const SQRT_N: usize = 3;
const N: usize = SQRT_N * SQRT_N;
const GRID: usize = N * N;
const VARS: usize = GRID * N;
const CRUMBS: usize = 32 / 2;
const UNITS: usize = VARS / CRUMBS + 1;

// Extended CNF encoding (9x9)
const LITERALS: usize = 26_244;
const CLAUSES: usize = 11_988;

const fn assert(condition: bool) -> Result<(), ()> {
    if condition {
        Ok(())
    } else {
        Err(())
    }
}
fn try_write_slice<'a, const LEN: usize>(
    dst: &'a mut [MaybeUninit<u16>; LEN],
    src: &[u16; LEN],
    len: usize,
) -> Result<&'a mut [u16], ()> {
    //SAFETY: This is the canonical way to fill an uninit slice.
    unsafe {
        assert(len <= LEN)?;
        let src: &[MaybeUninit<u16>] = core::mem::transmute(&src[..len]);
        let dst = &mut dst[..len];
        dst.copy_from_slice(src);
        Ok(&mut *(dst as *mut [MaybeUninit<u16>] as *mut [u16]))
    }
}

struct Units {
    raw: [u32; UNITS],
    log: [u16; VARS],
    cursor: [u16; GRID],
    next_cursor: usize,
    next_log: usize,
    new_units: bool,
}

struct Sudoku {
    next_literal: usize,
    next_clause: usize,
    lfsr: u32,
    units: Units,
    clauses: [u16; CLAUSES],
    literals: [u16; LITERALS],
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

impl Units {
    #[inline]
    fn set(&mut self, index: usize, value: bool) {
        let index = VARS.min(index);
        let literal = index * 2 | value as usize;
        //SAFETY: `raw` is sized so that this is in range.
        unsafe {
            let v = self.raw.get_unchecked_mut(literal >> 5);
            let mask = 1u32 << (literal & 31);
            if (*v & mask) == 0 && self.next_log < VARS {
                self.log[self.next_log] = literal as u16;
                self.next_log += 1;
                self.new_units = true;
            }
            *v |= mask;
        }
    }
    fn snapshot(&mut self) {
        if self.next_cursor >= GRID {
            return;
        }
        self.cursor[self.next_cursor] = self.next_log as u16;
        self.next_cursor += 1;
    }
    fn drop_snapshot(&mut self) {
        if self.next_cursor == 0 {
            return;
        }
        self.next_cursor -= 1;
    }
    fn rollback(&mut self) {
        if self.next_cursor == 0 || self.next_cursor > GRID {
            return;
        }
        self.next_cursor -= 1;
        let snapshot = self.cursor[self.next_cursor] as usize;
        while self.next_log > snapshot {
            self.next_log -= 1;
            unsafe {
                //SAFETY: `next_log` is always < VARS here.
                let literal = *self.log.get_unchecked(self.next_log) as usize;
                //SAFETY: `raw` is sized so that this is in range.
                *self.raw.get_unchecked_mut(literal / 32) ^= 1 << (literal & 31);
            }
        }
    }
    #[inline]
    fn get(&self, index: usize) -> u32 {
        let index = VARS.min(index);
        //SAFETY: `units` is sized so that this is in range.
        unsafe { (*self.raw.get_unchecked(index / CRUMBS) >> (index % CRUMBS * 2)) & 3 }
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
    fn set_false_or_assign(&mut self, index: usize, value: bool) {
        if value {
            // Effectively assignment, apply the rules.
            let row = index / (N * N);
            let column = index / N % N;
            let value = index % N;
            self.assign(row, column, value);
        } else {
            self.set(index, false);
        }
    }
    const fn new() -> Self {
        Self {
            raw: [0; UNITS],
            log: [0; VARS],
            cursor: [0; GRID],
            next_cursor: 0,
            next_log: 0,
            new_units: false,
        }
    }
}

impl Sudoku {
    fn try_insert(&mut self, clause: &[u16]) -> Result<(), ()> {
        let first_literal = self.next_literal;
        let mut next_literal = first_literal;
        for &literal in clause {
            let value = self.units.get((literal >> 1) as usize);
            if value == 0 {
                // Literal is indeterminate.
                assert(next_literal < LITERALS)?;
                self.literals[next_literal] = literal;
                next_literal += 1;
            } else if value == (1 << (literal & 1)) {
                // Literal is true, skip clause.
                return Ok(());
            }
            // Literal is false, skip literal.
        }
        // Empty clause should be unreachable.
        assert(first_literal < next_literal)?;
        // Unit clause.
        if next_literal.wrapping_sub(first_literal) == 1 {
            assert(first_literal < LITERALS)?;
            let literal = self.literals[first_literal];
            let index = (literal >> 1) as usize;
            let value = (literal & 1) != 0;
            self.units.set_false_or_assign(index, value);
            return Ok(());
        }
        let next_clause = self.next_clause;
        assert(next_clause < CLAUSES)?;
        self.clauses[next_clause] = first_literal as u16;
        self.next_literal = next_literal;
        self.next_clause = next_clause + 1;
        Ok(())
    }
    fn cell_uniqueness(&mut self, row: usize, column: usize) -> Result<(), ()> {
        Ok(for high_value in 1..N {
            for low_value in 0..high_value {
                self.try_insert(&[
                    not(index(row, column, low_value)),
                    not(index(row, column, high_value)),
                ])?;
            }
        })
    }
    fn cell_definedness(&mut self, row: usize, column: usize) -> Result<(), ()> {
        let mut clause = [0; N];
        for (value, literal) in clause.iter_mut().enumerate() {
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn row_uniqueness(&mut self, row: usize, value: usize) -> Result<(), ()> {
        Ok(for high_column in 1..N {
            for low_column in 0..high_column {
                self.try_insert(&[
                    not(index(row, low_column, value)),
                    not(index(row, high_column, value)),
                ])?;
            }
        })
    }
    fn row_definedness(&mut self, row: usize, value: usize) -> Result<(), ()> {
        let mut clause = [0; N];
        for (column, literal) in clause.iter_mut().enumerate() {
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn column_uniqueness(&mut self, column: usize, value: usize) -> Result<(), ()> {
        Ok(for high_row in 1..N {
            for low_row in 0..high_row {
                self.try_insert(&[
                    not(index(low_row, column, value)),
                    not(index(high_row, column, value)),
                ])?;
            }
        })
    }
    fn column_definedness(&mut self, column: usize, value: usize) -> Result<(), ()> {
        let mut clause = [0; N];
        for (row, literal) in clause.iter_mut().enumerate() {
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn block_uniqueness(
        &mut self,
        block_row: usize,
        block_column: usize,
        value: usize,
    ) -> Result<(), ()> {
        Ok(for high_offset in 1..N {
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
        })
    }
    fn block_definedness(
        &mut self,
        block_row: usize,
        block_column: usize,
        value: usize,
    ) -> Result<(), ()> {
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
        self.next_clause = 0;
        self.next_literal = 0;
        self.units.new_units = false;
        for row in 0..N {
            for column in 0..N {
                self.cell_definedness(row, column)?;
            }
        }
        for row in 0..N {
            for value in 0..N {
                self.row_definedness(row, value)?;
            }
        }
        for column in 0..N {
            for value in 0..N {
                self.column_definedness(column, value)?;
            }
        }
        for block_row in (0..N).step_by(SQRT_N) {
            for block_column in (0..N).step_by(SQRT_N) {
                for value in 0..N {
                    self.block_definedness(block_row, block_column, value)?;
                }
            }
        }
        for row in 0..N {
            for column in 0..N {
                self.cell_uniqueness(row, column)?;
            }
        }
        for row in 0..N {
            for value in 0..N {
                self.row_uniqueness(row, value)?;
            }
        }
        for column in 0..N {
            for value in 0..N {
                self.column_uniqueness(column, value)?;
            }
        }
        for block_row in (0..N).step_by(SQRT_N) {
            for block_column in (0..N).step_by(SQRT_N) {
                for value in 0..N {
                    self.block_uniqueness(block_row, block_column, value)?;
                }
            }
        }
        Ok(())
    }
    fn reduce_clauses(&mut self) -> Result<(), ()> {
        use core::iter::once;
        self.units.new_units = false;
        let len = self.next_clause;
        if len == 0 {
            return Ok(());
        }
        let mut clauses = unsafe { MaybeUninit::uninit().assume_init() };
        let clauses = try_write_slice(&mut clauses, &self.clauses, len)?;
        let tail = {
            let [.., last] = clauses else { Err(())? };
            [*last, self.next_literal as u16]
        };
        self.next_clause = 0;
        self.next_literal = 0;
        for bounds in clauses.windows(2).chain(once(&tail[..])) {
            let &[first_literal, next_literal] = bounds else {
                Err(())?
            };
            let first_literal = first_literal as usize;
            let next_literal = next_literal as usize;
            assert(first_literal <= next_literal)?;
            assert(next_literal <= LITERALS)?;
            let src = &self.literals[first_literal..next_literal];
            let len = src.len();
            assert(len <= N)?;
            let mut clause = [0; N];
            let clause = &mut clause[..len];
            clause.copy_from_slice(src);
            self.try_insert(clause)?;
        }
        Ok(())
    }
    fn randint(&mut self, limit: usize) -> usize {
        let mut x = self.lfsr;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.lfsr = x;
        ((x as u64 * limit as u64) >> 32) as usize
    }
    fn dpll(&mut self) -> bool {
        while self.units.new_units && self.next_clause != 0 {
            if self.reduce_clauses().is_err() {
                return false;
            }
        }
        if self.next_clause == 0 {
            return true;
        }
        self.units.snapshot();
        let (index, value) = {
            let literal = unsafe {
                //SAFETY: `next_literal` is always <= LITERALS.
                let choice = self.randint(self.next_literal);
                *self.literals.get_unchecked(choice)
            };
            (literal as usize >> 1, literal & 1 == 0)
        };
        self.units.set(index, value);
        if self.dpll() {
            self.units.drop_snapshot();
            return true;
        }
        self.units.rollback();
        self.units.set(index, !value);
        if self.generate_clauses().is_err() {
            return false;
        }
        self.dpll()
    }
    fn generate_instance(&mut self) {
        let cursors = self.units.cursor;
        let literals = self.units.log;
        self.next_literal = 0;
        self.next_clause = 0;
        self.units = Units::new();
        for cursor in cursors {
            let literal = unsafe {
                //SAFETY: `cursor` is always < LITERALS.
                *literals.get_unchecked(cursor as usize) as usize
            };
            if (literal & 1) != 0 {
                self.units.set_false_or_assign(literal >> 1, true);
            }
        }
    }
}

static mut SUDOKU: Sudoku = Sudoku {
    next_literal: 0,
    next_clause: 0,
    lfsr: 0,
    units: Units::new(),
    clauses: [0; CLAUSES],
    literals: [0; LITERALS],
};

#[wasm_bindgen]
pub fn assign(index: usize) {
    //SAFETY: Not guaranteed yet.
    unsafe {
        if SUDOKU.units.get(index) != 0 {
            return;
        }
        let row = index / (N * N);
        let column = index / N % N;
        let value = index % N;
        SUDOKU.units.assign(row, column, value);
    }
}

#[wasm_bindgen]
pub fn units_ptr() -> *const u32 {
    //SAFETY: Short-lived immutable view.
    unsafe { SUDOKU.units.raw.as_ptr() }
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

#[wasm_bindgen]
pub fn new_units() -> bool {
    //SAFETY: Primitive value.
    unsafe { SUDOKU.units.new_units }
}

#[wasm_bindgen]
pub fn generate_clauses() {
    //SAFETY: If single-threaded.
    unsafe {
        let _ = SUDOKU.generate_clauses();
    }
}

#[wasm_bindgen]
pub fn reduce_clauses() {
    //SAFETY: If single-threaded.
    unsafe {
        let _ = SUDOKU.reduce_clauses();
    }
}

#[wasm_bindgen]
pub fn dpll() -> bool {
    //SAFETY: If single-threaded.
    unsafe { SUDOKU.dpll() }
}

#[wasm_bindgen]
pub fn generate_instance(seed: u32) {
    //SAFETY: If single-threaded.
    unsafe {
        let sudoku = &mut SUDOKU;
        sudoku.units = Units::new();
        sudoku.lfsr = seed;
        let _ = sudoku.generate_clauses();
        sudoku.dpll();
        sudoku.generate_instance();
    }
}
