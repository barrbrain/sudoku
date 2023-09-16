import { memory } from './pkg/index_bg.wasm';
import { assign, generate_instance, solve, units_ptr, units_len } from './pkg/index.js';
const UNITS = units_len();
const units = () => new Uint32Array(memory.buffer, units_ptr(), UNITS);
const renderedUnits = new Uint32Array(UNITS);

const CELL_SIZE = 16;
const GRID_COLOR = "#CCCCCC";
const FALSE_COLOR = "#FFCCCC";
const TRUE_COLOR = "#000000";
const SIDE = 9 * 3;

const canvas = document.getElementById("sudoku");
canvas.height = (CELL_SIZE + 1) * SIDE + 1;
canvas.width = (CELL_SIZE + 1) * SIDE + 1;

const ctx = canvas.getContext('2d');

let renderLoopRequestID = 0;
const renderLoop = () => {
  drawCells();
  renderLoopRequestID = requestAnimationFrame(renderLoop);
};

const drawGrid = () => {
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  ctx.beginPath();
  ctx.strokeStyle = GRID_COLOR;

  for (let i = 0; i <= SIDE; i += 3) {
    ctx.moveTo(i * (CELL_SIZE + 1) + 1, 0);
    ctx.lineTo(i * (CELL_SIZE + 1) + 1, (CELL_SIZE + 1) * SIDE + 1);
  }

  for (let j = 0; j <= SIDE; j += 3) {
    ctx.moveTo(0,                          j * (CELL_SIZE + 1) + 1);
    ctx.lineTo((CELL_SIZE + 1) * SIDE + 1, j * (CELL_SIZE + 1) + 1);
  }

  ctx.stroke();

  renderedUnits.fill(0);
  renderLoop();
};

const getIndex = (row, column) => {
  const ROW = row / 3 | 0;
  const COL = column / 3 | 0;
  const VAL = (row - ROW * 3) * 3 + column - COL * 3;
  return (ROW * 9 + COL) * 9 + VAL;
};
const toRow = new Uint8Array(SIDE * SIDE);
const toCol = new Uint8Array(SIDE * SIDE);
(() => {
  for (let row = 0; row < SIDE; row++) {
    for (let col = 0; col < SIDE; col++) {
      const index = getIndex(row, col);
      toRow[index] = row;
      toCol[index] = col;
    }
  }
})();
const cellDrawer = (mask) => {
  return (cell, offset) => {
    let unrenderedCell = (cell ^ renderedUnits[offset]) & mask;
    while (unrenderedCell != 0) {
      const lz = Math.clz32(unrenderedCell);
      unrenderedCell ^= 0x80000000 >>> lz;
      const index = 15 - (lz >>> 1) + offset * 16;
      const row = toRow[index];
      const col = toCol[index];
      const text = index % 9 + 1;
      ctx.strokeText(
        text,
        col * (CELL_SIZE + 1) + CELL_SIZE / 2 + 1,
        row * (CELL_SIZE + 1) + CELL_SIZE - 1,
      );
    }
  };
};
const trueCellDrawer = cellDrawer(0xAAAAAAAA);
const falseCellDrawer = cellDrawer(0x55555555);
const drawCells = () => {
  const cells = units();
  if (!cells.some((v, i) => v !== renderedUnits[i])) {
    return;
  }

  ctx.font = CELL_SIZE + "px sans-serif";
  ctx.textAlign = "center";

  ctx.strokeStyle = TRUE_COLOR;
  cells.forEach(trueCellDrawer);

  ctx.strokeStyle = FALSE_COLOR;
  cells.forEach(falseCellDrawer);

  renderedUnits.set(cells);
};

canvas.addEventListener("click", event => {
  const boundingRect = canvas.getBoundingClientRect();

  const scaleX = canvas.width / boundingRect.width;
  const scaleY = canvas.height / boundingRect.height;

  const canvasLeft = (event.clientX - boundingRect.left) * scaleX;
  const canvasTop = (event.clientY - boundingRect.top) * scaleY;

  const row = Math.min(Math.floor(canvasTop / (CELL_SIZE + 1)), SIDE - 1);
  const col = Math.min(Math.floor(canvasLeft / (CELL_SIZE + 1)), SIDE - 1);

  assign(getIndex(row, col));
});

const solveButton = document.getElementById("solve");
solveButton.addEventListener("click", event => {
  console.log({solved: solve()})
});

const genButton = document.getElementById("generate");
genButton.addEventListener("click", event => {
  const seed = Math.random() * 0xffffffff | 0;
  cancelAnimationFrame(renderLoopRequestID);
  generate_instance(seed);
  requestAnimationFrame(drawGrid);
});

requestAnimationFrame(drawGrid);
