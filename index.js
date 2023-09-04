import { memory } from './pkg/index_bg.wasm';
import { assign, units_ptr, units_len } from './pkg/index.js';
const UNITS = units_len();
const units = () => new Uint32Array(memory.buffer, units_ptr(), UNITS);

const CELL_SIZE = 16;
const GRID_COLOR = "#CCCCCC";
const FALSE_COLOR = "#FFCCCC";
const TRUE_COLOR = "#000000";
const SIDE = 9 * 3;

const canvas = document.getElementById("sudoku");
canvas.height = (CELL_SIZE + 1) * SIDE + 1;
canvas.width = (CELL_SIZE + 1) * SIDE + 1;

const ctx = canvas.getContext('2d');

const renderLoop = () => {
  drawGrid();
  drawCells();

  requestAnimationFrame(renderLoop);
};

const drawGrid = () => {
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
};

const getIndex = (row, column) => {
  const ROW = row / 3 | 0;
  const COL = column / 3 | 0;
  const VAL = (row - ROW * 3) * 3 + column - COL * 3;
  return (ROW * 9 + COL) * 9 + VAL;
};

const drawCells = () => {
  const cells = units();

  ctx.font = CELL_SIZE + "px sans-serif";
  ctx.textAlign = "center";

  ctx.strokeStyle = TRUE_COLOR;
  for (let row = 0; row < SIDE; row++) {
    for (let col = 0; col < SIDE; col++) {
      const idx = getIndex(row, col);
      const value = (cells[idx / 16 | 0] >> (idx % 16 * 2)) & 3;
      if (value !== 2) {
        continue;
      }
      const text = idx % 9 + 1;
      ctx.strokeText(
        text,
        col * (CELL_SIZE + 1) + CELL_SIZE / 2 + 1,
        row * (CELL_SIZE + 1) + CELL_SIZE - 1,
      );
    }
  }

  ctx.strokeStyle = FALSE_COLOR;
  for (let row = 0; row < SIDE; row++) {
    for (let col = 0; col < SIDE; col++) {
      const idx = getIndex(row, col);
      const value = (cells[idx / 16 | 0] >> (idx % 16 * 2)) & 3;
      if (value !== 1) {
        continue;
      }
      const text = idx % 9 + 1;
      ctx.strokeText(
        text,
        col * (CELL_SIZE + 1) + CELL_SIZE / 2 + 1,
        row * (CELL_SIZE + 1) + CELL_SIZE - 1,
      );
    }
  }
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

requestAnimationFrame(renderLoop);
