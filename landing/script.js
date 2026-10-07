const slider = document.querySelector('#demo-volume');
const percent = document.querySelector('#percent');
const bar = document.querySelector('#bar');
const finger = document.querySelector('.finger');
const label = document.querySelector('#volume-label');
let previous = Number(slider.value);
slider.addEventListener('input', () => {
  const value = Number(slider.value);
  percent.textContent = String(value);
  bar.style.width = `${value}%`;
  finger.style.bottom = `${15 + value * 0.7}%`;
  label.textContent = value === 100 ? 'Maximum volume' : value === 0 ? 'Minimum volume' : value > previous ? 'Increasing volume' : value < previous ? 'Decreasing volume' : 'Output volume';
  previous = value;
});
