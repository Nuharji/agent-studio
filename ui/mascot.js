/* Bit, Agent Studio's mascot: a small round robot in the accent colors. It sits on the home page and shows what is
   going on — waiting (blinks), working (bobs, its antenna glows), needs you (raised brows, "!"), done today (smiles) or a
   problem (worried). Settings → General → "Show Bit" hides it. Still with "reduce motion". */
function mascotState() {
  const need = needItems().length, running = data.runs.filter(r => isRunning(r.id)).length;
  const today = data.runs.filter(r => (r.ended || 0) >= dayStart());
  const failed = today.length && ['failed'].includes(today[0].status);
  if (need) return {s: 'need', n: need};
  if (running) return {s: 'work', n: running};
  if (failed) return {s: 'problem', n: 1};
  const done = today.filter(r => r.status === 'done').length;
  return done ? {s: 'done', n: done} : {s: 'idle', n: 0};
}
function mascotSvg(state, size = 64) {
  const mouth = {
    idle: '<path d="M27 43 q5 3 10 0" />',
    work: '<path d="M28 43 h8" />',
    need: '<ellipse cx="32" cy="44" rx="3" ry="3.4" class="m-fill" />',
    done: '<path d="M25 41 q7 7 14 0" />',
    problem: '<path d="M27 45 q5 -3 10 0" />',
  }[state] || '';
  const brows = state === 'need' ? '<path d="M20 25 l6 -2 M38 23 l6 2" />' : state === 'problem' ? '<path d="M20 24 l6 2 M38 26 l6 -2" />' : '';
  const eyes = state === 'done'
    ? '<path d="M20 33 q3.5 -4 7 0 M37 33 q3.5 -4 7 0" />'
    : '<g class="m-eyes"><ellipse cx="23.5" cy="33" rx="3.6" ry="4.2" class="m-white" /><ellipse cx="40.5" cy="33" rx="3.6" ry="4.2" class="m-white" />'
      + `<circle cx="${state === 'work' ? 25 : 24}" cy="${state === 'work' ? 34 : 33.5}" r="1.8" class="m-fill" /><circle cx="${state === 'work' ? 42 : 41}" cy="${state === 'work' ? 34 : 33.5}" r="1.8" class="m-fill" /></g>`;
  return `<svg class="mascot m-${state}" width="${size}" height="${size}" viewBox="0 0 64 64" aria-hidden="true">
    <defs><linearGradient id="mGrad" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="var(--tile-a)" /><stop offset="1" stop-color="var(--tile-b)" /></linearGradient></defs>
    <g class="m-body">
      <line x1="32" y1="14" x2="32" y2="7" class="m-line" /><circle cx="32" cy="6" r="3.2" class="m-bulb" />
      <rect x="8" y="14" width="48" height="42" rx="16" fill="url(#mGrad)" />
      <rect x="13" y="21" width="38" height="30" rx="11" class="m-face" />
      <circle cx="17" cy="44" r="2.6" class="m-cheek" /><circle cx="47" cy="44" r="2.6" class="m-cheek" />
      <g class="m-line">${brows}${eyes}${mouth}</g>
    </g>
    ${state === 'need' ? '<text x="55" y="14" class="m-bang">!</text>' : ''}
  </svg>`;
}
function mascotCard() {
  if (cfg.mascot === false) return '';
  const {s, n} = mascotState();
  return `<div class="pcard mascot-card" data-act="${s === 'need' ? 'viewInbox' : 'mascotPoke'}">${mascotSvg(s, 56)}
    <div class="m-bubble"><b>${esc(t('masc.name'))}</b><span>${esc(tn('masc.' + s, n))}</span></div></div>`;
}
