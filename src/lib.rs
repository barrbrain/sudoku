#![no_std]
use core::mem::MaybeUninit;
use wasm_bindgen::prelude::*;

mod smallindex;
use smallindex::SmallIndex;

const SQRT_N: usize = 3;
const N: usize = SQRT_N * SQRT_N;
const GRID: usize = N * N;
const VARS: usize = GRID * N;
const VALUES: usize = VARS * 2;
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
fn try_write_slice<'a, const LEN: usize, T: Sized + Copy>(
    dst: &'a mut [MaybeUninit<T>; LEN],
    src: &[T; LEN],
    len: usize,
) -> Result<&'a mut [T], ()> {
    //SAFETY: This is the canonical way to fill an uninit slice.
    unsafe {
        assert(len <= LEN)?;
        let src: &[MaybeUninit<T>] = core::mem::transmute(&src[..len]);
        let dst = &mut dst[..len];
        dst.copy_from_slice(src);
        Ok(&mut *(dst as *mut [MaybeUninit<T>] as *mut [T]))
    }
}

struct Units {
    raw: [u32; UNITS],
    log: [SmallIndex<VALUES>; VARS],
    cursor: [SmallIndex<VARS>; GRID],
    next_cursor: usize,
    next_log: usize,
    new_units: bool,
}

struct Sudoku {
    next_literal: usize,
    next_clause: usize,
    lfsr: u32,
    units: Units,
    clauses: [SmallIndex<LITERALS>; CLAUSES],
    literals: [SmallIndex<VALUES>; LITERALS],
}

fn index(row: SmallIndex<N>, column: SmallIndex<N>, value: SmallIndex<N>) -> SmallIndex<VARS> {
    SmallIndex::new_unchecked(
        row.raw()
            .wrapping_mul(N as u16)
            .wrapping_add(column.raw())
            .wrapping_mul(N as u16)
            .wrapping_add(value.raw()),
    )
}
fn is(index: SmallIndex<VARS>) -> SmallIndex<VALUES> {
    SmallIndex::from_var(index, 1)
}
fn not(index: SmallIndex<VARS>) -> SmallIndex<VALUES> {
    SmallIndex::from_var(index, 0)
}

impl Units {
    #[inline]
    fn set(&mut self, index: SmallIndex<VARS>, value: bool) {
        let literal = SmallIndex::from_var(index, value as u16);
        let (index, bit) = literal.raw_bit();
        let v = index.get_mut(&mut self.raw);
        let mask = 1 << bit;
        if (*v & mask) == 0 && self.next_log < VARS {
            self.log[self.next_log] = literal;
            self.next_log += 1;
            self.new_units = true;
        }
        *v |= mask;
    }
    fn snapshot(&mut self) {
        if self.next_cursor >= GRID {
            return;
        }
        self.cursor[self.next_cursor] = SmallIndex::new(self.next_log as u16);
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
        let snapshot = self.cursor[self.next_cursor].raw().into();
        while self.next_log > snapshot {
            self.next_log -= 1;
            let literal = SmallIndex::new_unchecked(self.next_log as u16).get(&self.log);
            let (index, bit) = literal.raw_bit();
            *index.get_mut(&mut self.raw) ^= 1 << bit;
        }
    }
    #[inline]
    fn get(&self, index: SmallIndex<VARS>) -> u32 {
        let (index, crumb) = index.raw_crumb();
        (index.get(&self.raw) >> (crumb * 2)) & 3
    }
    fn assign(&mut self, triple: [SmallIndex<N>; 3]) {
        let [row, column, value] = triple;
        self.set(index(row, column, value), true);
        for row in SmallIndex::all().filter(|&i| i != row) {
            self.set(index(row, column, value), false);
        }
        for column in SmallIndex::all().filter(|&i| i != column) {
            self.set(index(row, column, value), false);
        }
        for value in SmallIndex::all().filter(|&i| i != value) {
            self.set(index(row, column, value), false);
        }
        let block_row = row.raw() - row.raw() % SQRT_N as u16;
        let block_column = column.raw() - column.raw() % SQRT_N as u16;
        for block_row in block_row..block_row + SQRT_N as u16 {
            let block_row = SmallIndex::new_unchecked(block_row);
            for block_column in block_column..block_column + SQRT_N as u16 {
                let block_column = SmallIndex::new_unchecked(block_column);
                if block_row != row || block_column != column {
                    self.set(index(block_row, block_column, value), false);
                }
            }
        }
    }
    fn set_false_or_assign(&mut self, index: SmallIndex<VARS>, value: bool) {
        if value {
            self.assign(index.into());
        } else {
            self.set(index, false);
        }
    }
    const fn new() -> Self {
        Self {
            raw: [0; UNITS],
            log: SmallIndex::array(),
            cursor: SmallIndex::array(),
            next_cursor: 0,
            next_log: 0,
            new_units: false,
        }
    }
}

impl Sudoku {
    fn try_insert(&mut self, clause: &[SmallIndex<VALUES>]) -> Result<(), ()> {
        let first_literal = self.next_literal;
        let mut next_literal = first_literal;
        for &literal in clause {
            let (index, bit) = literal.to_var();
            let value = self.units.get(index);
            if value == 0 {
                // Literal is indeterminate.
                assert(next_literal < LITERALS)?;
                self.literals[next_literal] = literal;
                next_literal += 1;
            } else if value == (1 << bit) {
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
            let (index, bit) = literal.to_var();
            self.units.set_false_or_assign(index, bit != 0);
            return Ok(());
        }
        let next_clause = self.next_clause;
        assert(next_clause < CLAUSES)?;
        self.clauses[next_clause] = SmallIndex::new_unchecked(first_literal as u16);
        self.next_literal = next_literal;
        self.next_clause = next_clause + 1;
        Ok(())
    }
    fn cell_uniqueness(&mut self, row: SmallIndex<N>, column: SmallIndex<N>) -> Result<(), ()> {
        Ok(for subset in SmallIndex::<N>::pairs() {
            for (high_value, low_value) in subset {
                self.try_insert(&[
                    not(index(row, column, low_value)),
                    not(index(row, column, high_value)),
                ])?;
            }
        })
    }
    fn cell_definedness(&mut self, row: SmallIndex<N>, column: SmallIndex<N>) -> Result<(), ()> {
        let mut clause = SmallIndex::array();
        for value in SmallIndex::all() {
            let literal = value.get_mut(&mut clause);
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn row_uniqueness(&mut self, row: SmallIndex<N>, value: SmallIndex<N>) -> Result<(), ()> {
        Ok(for subset in SmallIndex::<N>::pairs() {
            for (high_column, low_column) in subset {
                self.try_insert(&[
                    not(index(row, low_column, value)),
                    not(index(row, high_column, value)),
                ])?;
            }
        })
    }
    fn row_definedness(&mut self, row: SmallIndex<N>, value: SmallIndex<N>) -> Result<(), ()> {
        let mut clause = SmallIndex::array();
        for column in SmallIndex::all() {
            let literal = column.get_mut(&mut clause);
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn column_uniqueness(&mut self, column: SmallIndex<N>, value: SmallIndex<N>) -> Result<(), ()> {
        Ok(for subset in SmallIndex::<N>::pairs() {
            for (high_row, low_row) in subset {
                self.try_insert(&[
                    not(index(low_row, column, value)),
                    not(index(high_row, column, value)),
                ])?;
            }
        })
    }
    fn column_definedness(
        &mut self,
        column: SmallIndex<N>,
        value: SmallIndex<N>,
    ) -> Result<(), ()> {
        let mut clause = SmallIndex::array();
        for row in SmallIndex::all() {
            let literal = row.get_mut(&mut clause);
            *literal = is(index(row, column, value));
        }
        self.try_insert(&clause)
    }
    fn block_uniqueness(
        &mut self,
        block_row: SmallIndex<N>,
        block_column: SmallIndex<N>,
        value: SmallIndex<N>,
    ) -> Result<(), ()> {
        Ok(for subset in SmallIndex::<N>::pairs() {
            for (high_offset, low_offset) in subset {
                let high_row =
                    SmallIndex::new_unchecked(block_row.raw() + high_offset.raw() / SQRT_N as u16);
                let high_column = SmallIndex::new_unchecked(
                    block_column.raw() + high_offset.raw() % SQRT_N as u16,
                );
                let low_row =
                    SmallIndex::new_unchecked(block_row.raw() + low_offset.raw() / SQRT_N as u16);
                let low_column = SmallIndex::new_unchecked(
                    block_column.raw() + low_offset.raw() % SQRT_N as u16,
                );
                self.try_insert(&[
                    not(index(low_row, low_column, value)),
                    not(index(high_row, high_column, value)),
                ])?;
            }
        })
    }
    fn block_definedness(
        &mut self,
        block_row: SmallIndex<N>,
        block_column: SmallIndex<N>,
        value: SmallIndex<N>,
    ) -> Result<(), ()> {
        let mut clause = SmallIndex::array::<N>();
        for (offset, subclause) in clause.chunks_exact_mut(SQRT_N).enumerate() {
            let row = SmallIndex::new_unchecked(block_row.raw() + offset as u16);
            for (offset, literal) in subclause.iter_mut().enumerate() {
                let column = SmallIndex::new_unchecked(block_column.raw() + offset as u16);
                *literal = is(index(row, column, value));
            }
        }
        self.try_insert(&clause)
    }
    fn generate_clauses(&mut self) -> Result<(), ()> {
        self.next_clause = 0;
        self.next_literal = 0;
        self.units.new_units = false;
        for row in SmallIndex::all() {
            for column in SmallIndex::all() {
                self.cell_definedness(row, column)?;
            }
        }
        for row in SmallIndex::all() {
            for value in SmallIndex::all() {
                self.row_definedness(row, value)?;
            }
        }
        for column in SmallIndex::all() {
            for value in SmallIndex::all() {
                self.column_definedness(column, value)?;
            }
        }
        for block_row in SmallIndex::all().step_by(SQRT_N) {
            for block_column in SmallIndex::all().step_by(SQRT_N) {
                for value in SmallIndex::all() {
                    self.block_definedness(block_row, block_column, value)?;
                }
            }
        }
        for row in SmallIndex::all() {
            for column in SmallIndex::all() {
                self.cell_uniqueness(row, column)?;
            }
        }
        for row in SmallIndex::all() {
            for value in SmallIndex::all() {
                self.row_uniqueness(row, value)?;
            }
        }
        for column in SmallIndex::all() {
            for value in SmallIndex::all() {
                self.column_uniqueness(column, value)?;
            }
        }
        for block_row in SmallIndex::all().step_by(SQRT_N) {
            for block_column in SmallIndex::all().step_by(SQRT_N) {
                for value in SmallIndex::all() {
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
            [*last, SmallIndex::new_unchecked(self.next_literal as u16)]
        };
        self.next_clause = 0;
        self.next_literal = 0;
        for bounds in clauses.windows(2).chain(once(&tail[..])) {
            let &[first_literal, next_literal] = bounds else {
                Err(())?
            };
            let first_literal = first_literal.raw() as usize;
            let next_literal = next_literal.raw() as usize;
            assert(first_literal <= next_literal)?;
            assert(next_literal <= LITERALS)?;
            let src = &self.literals[first_literal..next_literal];
            let len = src.len();
            assert(len <= N)?;
            let mut clause = SmallIndex::array::<N>();
            let clause = &mut clause[..len];
            clause.copy_from_slice(src);
            self.try_insert(clause)?;
        }
        Ok(())
    }
    fn randint(&mut self, limit: usize) -> u16 {
        let mut x = self.lfsr;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.lfsr = x;
        ((x as u64 * limit as u64) >> 32) as u16
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
        let (index, bit) = SmallIndex::new_unchecked(self.randint(self.next_literal))
            .get(&self.literals)
            .to_var();
        let value = bit == 0;
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
            let (index, bit) = cursor.get(&literals).to_var();
            if bit != 0 {
                self.units.set_false_or_assign(index, true);
            }
        }
    }
}

static mut SUDOKU: Sudoku = Sudoku {
    next_literal: 0,
    next_clause: 0,
    lfsr: 0,
    units: Units::new(),
    clauses: SmallIndex::array(),
    literals: SmallIndex::array(),
};

#[wasm_bindgen]
pub fn assign(index: usize) {
    //SAFETY: Not guaranteed yet.
    unsafe {
        let index = SmallIndex::new(index as u16);
        if SUDOKU.units.get(index) != 0 {
            return;
        }
        SUDOKU.units.assign(index.into());
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
pub fn solve() -> bool {
    //SAFETY: If single-threaded.
    unsafe { SUDOKU.generate_clauses().is_ok() && SUDOKU.dpll() }
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
