/* EasyCue3 Remote — Framework7 client.
 *
 * Thin stateless view over the console's WebSocket: renders whatever the
 * server pushes, sends commands back, applies optimistic local updates that
 * the next authoritative push reconciles.
 */
'use strict';

// Bump this whenever the embedded client changes — it's shown in the Cmd tab
// footer so you can spot a stale build at a glance.
const CLIENT_VERSION = '0.8.0-c13';

const f7 = new Framework7({
  el: '#app',
  name: 'EasyCue3 Remote',
  theme: 'auto',
  darkMode: true,
});

// ---------------------------------------------------------------- state ----

const S = {
  token: localStorage.getItem('ec3_token') || '',
  ws: null,
  wsOk: false,
  reconnectDelay: 1000,
  structure: null,          // {show_title, cues, patch, profiles, groups, active_universes}
  universes: {},            // id -> Array(512) of 0-100
  playback: {},
  curUniverse: 1,
  selected: new Set(),      // selected channel numbers (current universe)
  selFixtures: new Set(),   // selected fixture ids (Fixtures select + Groups recall)
  selMode: false,           // true when tapping a fixture toggles selection
  recalledGroup: null,      // id of the group currently recalled on the Groups tab
  cmdContext: 'fixture',
  hold: {},                 // control key -> timestamp until which pushes are ignored
  suppress: false,          // true while we set range values programmatically
  fixtureSheet: null,       // {sheet, fixtureId, ranges: {key: range}}
  colorWheel: null,         // {popup, patch, prof, ranges: {r,g,b}, h, s}
  logCount: 0,
};

const $$ = Dom7;

function holdKey(key, ms) { S.hold[key] = Date.now() + (ms || 600); }
function isHeld(key) { return (S.hold[key] || 0) > Date.now(); }

// Framework7 8 range callbacks provide the range instance; read the value
// from it rather than relying on a nonexistent second callback argument.
function scalarRangeValue(range) {
  const value = range.getValue();
  return Array.isArray(value) ? value[0] : value;
}

// ----------------------------------------------------------- colour math ----
// Mirrors the desktop circular HSV wheel (src/ui/color_wheel.rs): hue by
// angle (red at 3 o'clock, clockwise), saturation by radius, value fixed 1.

function hsvToRgb(h, s, v) {
  h = ((h % 1) + 1) % 1;
  if (s < 1e-6) return [v * 255, v * 255, v * 255];
  const h6 = h * 6, i = Math.floor(h6) % 6, f = h6 - Math.floor(h6);
  const p = v * (1 - s), q = v * (1 - s * f), t = v * (1 - s * (1 - f));
  let r, g, b;
  switch (i) {
    case 0: [r, g, b] = [v, t, p]; break;
    case 1: [r, g, b] = [q, v, p]; break;
    case 2: [r, g, b] = [p, v, t]; break;
    case 3: [r, g, b] = [p, q, v]; break;
    case 4: [r, g, b] = [t, p, v]; break;
    default: [r, g, b] = [v, p, q]; break;
  }
  return [r * 255, g * 255, b * 255];
}

function rgbToHsv(r, g, b) {
  const max = Math.max(r, g, b), min = Math.min(r, g, b), delta = max - min;
  const v = max;
  const s = max < 1e-6 ? 0 : delta / max;
  let h;
  if (delta < 1e-6) h = 0;
  else if (max === r) h = ((g - b) / delta) % 6;
  else if (max === g) h = (b - r) / delta + 2;
  else h = (r - g) / delta + 4;
  return [((h / 6) % 1 + 1) % 1, s, v];
}

// Pre-rendered hue/sat disk (value 1), cached once.
let wheelCanvas = null;
function getWheelCanvas() {
  if (wheelCanvas) return wheelCanvas;
  wheelCanvas = document.createElement('canvas');
  const N = 256;
  wheelCanvas.width = N; wheelCanvas.height = N;
  const g = wheelCanvas.getContext('2d');
  const img = g.createImageData(N, N);
  const cx = (N - 1) / 2, cy = (N - 1) / 2, radius = N / 2;
  for (let y = 0; y < N; y++) {
    for (let x = 0; x < N; x++) {
      const dx = x - cx, dy = y - cy, dist = Math.hypot(dx, dy);
      const i = (y * N + x) * 4;
      if (dist > radius) { img.data[i + 3] = 0; continue; }
      const hue = ((Math.atan2(dy, dx) / (Math.PI * 2)) + 1) % 1;
      const sat = Math.min(1, dist / radius);
      const [r, g2, b] = hsvToRgb(hue, sat, 1);
      img.data[i] = r; img.data[i + 1] = g2; img.data[i + 2] = b; img.data[i + 3] = 255;
    }
  }
  g.putImageData(img, 0, 0);
  return wheelCanvas;
}

// ------------------------------------------------------------- transport ----

function send(type, payload) {
  if (S.ws && S.ws.readyState === WebSocket.OPEN) {
    S.ws.send(JSON.stringify(payload === undefined ? { type } : { type, payload }));
  }
}

let disconnectToast = null;

async function connect() {
  // Validate the token over REST first — WebSocket can't surface a 401.
  let resp;
  try {
    resp = await fetch('/api/ping', { headers: { 'x-easycue-token': S.token } });
  } catch (e) {
    scheduleReconnect();
    return;
  }
  if (resp.status === 401) {
    f7.loginScreen.open('#pin-screen', false);
    return;
  }

  const proto = location.protocol === 'https:' ? 'wss' : 'ws';
  const ws = new WebSocket(
    proto + '://' + location.host + '/ws?token=' + encodeURIComponent(S.token)
  );
  S.ws = ws;

  ws.onopen = () => {
    S.wsOk = true;
    S.reconnectDelay = 1000;
    f7.loginScreen.close('#pin-screen');
    if (disconnectToast) { disconnectToast.close(); disconnectToast = null; }
  };
  ws.onmessage = (ev) => {
    let msg;
    try { msg = JSON.parse(ev.data); } catch (e) { return; }
    handleMessage(msg);
  };
  ws.onclose = () => {
    if (S.ws !== ws) return;
    S.wsOk = false;
    scheduleReconnect();
  };
  ws.onerror = () => ws.close();
}

function scheduleReconnect() {
  if (!disconnectToast) {
    disconnectToast = f7.toast.create({
      text: 'Connection lost — reconnecting…',
      position: 'top',
    });
    disconnectToast.open();
  }
  setTimeout(connect, S.reconnectDelay);
  S.reconnectDelay = Math.min(S.reconnectDelay * 1.5, 8000);
}

// iOS Safari kills sockets in background PWAs — reconnect on return.
document.addEventListener('visibilitychange', () => {
  if (!document.hidden && (!S.ws || S.ws.readyState !== WebSocket.OPEN)) {
    connect();
  }
});

// PIN form
$$('#pin-submit').on('click', (e) => {
  e.preventDefault();
  S.token = $$('#pin-input').val().trim();
  localStorage.setItem('ec3_token', S.token);
  connect();
});
$$('#pin-form').on('submit', (e) => {
  e.preventDefault();
  $$('#pin-submit').trigger('click');
});

// -------------------------------------------------------------- messages ----

function handleMessage(msg) {
  const p = msg.payload || {};
  switch (msg.type) {
    case 'snapshot':
      S.structure = p.structure || S.structure;
      (p.universes || []).forEach((u) => { S.universes[u.universe] = u.values; });
      S.playback = p.playback || {};
      renderAll();
      break;
    case 'structure':
      S.structure = p;
      renderStructure();
      renderPlayback();
      renderLiveValues();
      break;
    case 'channels':
      S.universes[p.universe] = p.values;
      renderLiveValues();
      break;
    case 'playback':
      S.playback = p;
      renderPlayback();
      break;
    case 'log':
      appendLog(p.text, p.reply);
      // Patch operation results surface as a toast on whatever page is open.
      if (p.text === 'patch') {
        f7.toast.create({ text: p.reply, closeTimeout: 2500 }).open();
      }
      break;
  }
}

function renderAll() {
  renderStructure();
  renderPlayback();
  renderLiveValues();
}

// ------------------------------------------------------------------ cues ----

function esc(s) {
  return String(s == null ? '' : s)
    .replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function fmtCueNum(n) {
  return (Math.round(n * 10) / 10).toFixed(Math.abs(n % 1) > 0.001 ? 1 : 0);
}

function kindBadge(kind) {
  if (kind === 'audio') return '<span class="badge color-purple">SND</span>';
  if (kind === 'adjust') return '<span class="badge color-orange">ADJ</span>';
  return '<span class="badge color-blue">LX</span>';
}

function renderCues() {
  const st = S.structure;
  if (!st) return;
  $$('#cues-title').text(st.show_title || 'EasyCue3');
  const rows = st.cues.map((c, i) => {
    let fades = '';
    if (c.kind === 'lighting') {
      fades = '&uarr;' + c.fade_up + 's &darr;' + c.fade_down + 's';
      if (c.autofollow != null) fades += ' &rarr; auto ' + c.autofollow + 's';
    }
    return (
      '<li data-index="' + i + '">' +
        '<a href="#" class="item-link item-content cue-row">' +
          '<div class="item-media"><strong>' + fmtCueNum(c.number) + '</strong></div>' +
          '<div class="item-inner">' +
            '<div class="item-title">' + esc(c.label || ('Cue ' + fmtCueNum(c.number))) +
              '<div class="item-footer">' + fades + '</div></div>' +
            '<div class="item-after">' + kindBadge(c.kind) +
              '<span class="cue-marker"></span></div>' +
          '</div>' +
        '</a>' +
      '</li>'
    );
  });
  $$('#cue-list ul').html(rows.join(''));
}

// Double-tap a cue row to fire it (single taps are ignored so scrolling
// through the list can't accidentally jump the show).
let lastCueTap = { idx: -1, t: 0 };
$$(document).on('click', '.cue-row', function (e) {
  e.preventDefault();
  const idx = parseInt($$(this).parent('li').attr('data-index'), 10);
  const cue = S.structure && S.structure.cues[idx];
  if (!cue) return;
  const now = Date.now();
  if (lastCueTap.idx === idx && now - lastCueTap.t < 400) {
    lastCueTap = { idx: -1, t: 0 };
    send('cue_goto', { number: cue.number });
  } else {
    lastCueTap = { idx: idx, t: now };
  }
});

function renderPlayback() {
  const pb = S.playback || {};
  $$('#cues-status').text(pb.status || ' ');
  $$('#cue-list li').forEach((li) => {
    const idx = parseInt(li.getAttribute('data-index'), 10);
    li.classList.toggle('ec-cue-live', idx === pb.current_index);
    li.classList.toggle('ec-cue-next', idx === pb.next_index && idx !== pb.current_index);
    const marker = li.querySelector('.cue-marker');
    if (marker) {
      marker.textContent =
        idx === pb.current_index ? ' ▶' : (idx === pb.next_index ? ' ⏵ next' : '');
    }
  });
  const progress = pb.playing && pb.progress != null ? Math.round(pb.progress * 100) : 0;
  f7.progressbar.set('#cue-progress', progress);

  // Dom7's toggleClass has no second "state" argument (unlike jQuery).
  if (pb.blackout) $$('#blackout-btn').addClass('text-color-red');
  else $$('#blackout-btn').removeClass('text-color-red');
  $$('#blackout-btn').text(pb.blackout ? 'BO ●' : 'BO');

  if (masterRange && !isHeld('master')) {
    S.suppress = true;
    masterRange.setValue(Math.round((pb.master != null ? pb.master : 1) * 100));
    S.suppress = false;
  }

  if (soundMasterRange && !isHeld('sndmaster')) {
    S.suppress = true;
    soundMasterRange.setValue(Math.round((pb.sound_master != null ? pb.sound_master : 1) * 100));
    S.suppress = false;
  }

  renderAudioStatus();
}

$$('#btn-go').on('click', (e) => { e.preventDefault(); send('cue_go'); });
$$('#btn-back').on('click', (e) => { e.preventDefault(); send('cue_back'); });
$$('#btn-stop').on('click', (e) => { e.preventDefault(); send('cue_stop'); });

$$('#blackout-btn').on('click', function (e) {
  e.preventDefault();
  const active = !(S.playback && S.playback.blackout);
  if (active) {
    f7.dialog.confirm('Black out all lighting output?', 'Blackout',
      () => send('set_blackout', { active: true }));
  } else {
    send('set_blackout', { active: false });
  }
});

let masterRange = null;

function renderAudioStatus() {
  const pb = S.playback || {};
  const st = S.structure;
  const el = $$('#audio-status');
  const cue = st && pb.audio_cue_index != null ? st.cues[pb.audio_cue_index] : null;
  if (pb.audio_state === 'stopped' && !cue) {
    el.css('display', 'none');
    return;
  }
  el.css('display', 'block');
  const label = cue
    ? 'Q' + fmtCueNum(cue.number) + (cue.label ? ' ' + esc(cue.label) : '')
    : 'SND';
  let text;
  if (pb.audio_state === 'fading_in') {
    text = 'SND: ' + label + ' — fading in ' + Math.round((pb.audio_progress || 0) * 100) + '%';
  } else if (pb.audio_state === 'fading_out') {
    text = 'SND: ' + label + ' — fading out ' + Math.round((1 - (pb.audio_progress || 0)) * 100) + '%';
  } else {
    text = 'SND: ' + label + ' — playing';
  }
  $$('#audio-status-text').html(text);
}

// -------------------------------------------------------------- fixtures ----

function profileOf(patch) {
  return (S.structure && S.structure.profiles[patch.profile_id]) || null;
}

function fixtureIntensity(patch) {
  const prof = profileOf(patch);
  const uni = S.universes[patch.universe];
  if (!prof || !uni) return 0;
  const intensityParam = prof.parameters.find((p) => p.is_intensity);
  if (intensityParam) {
    return uni[patch.start_address - 1 + intensityParam.offset] || 0;
  }
  let max = 0;
  prof.parameters.forEach((p) => {
    if (p.is_color) max = Math.max(max, uni[patch.start_address - 1 + p.offset] || 0);
  });
  return max;
}

function fixtureSwatch(patch) {
  const prof = profileOf(patch);
  const uni = S.universes[patch.universe];
  if (!prof || !uni || !prof.is_rgb) return '';
  const get = (key) => {
    const p = prof.parameters.find((q) => q.key === key);
    return p ? Math.round((uni[patch.start_address - 1 + p.offset] || 0) * 2.55) : 0;
  };
  return '<span class="ec-swatch" style="background: rgb(' +
    get('red') + ',' + get('green') + ',' + get('blue') + ')"></span>';
}

function renderFixtures() {
  const st = S.structure;
  if (!st) return;
  const has = st.patch.length > 0;
  $$('#fixture-empty').css('display', has ? 'none' : 'block');
  const rows = st.patch.map((p) => {
    const prof = profileOf(p);
    const sel = S.selFixtures.has(p.id) ? ' ec-fx-sel' : '';
    return (
      '<li data-fixture="' + p.id + '">' +
        '<a href="#" class="item-link item-content fixture-row' + sel + '">' +
          '<div class="item-media"><span class="badge">' + p.id + '</span></div>' +
          '<div class="item-inner">' +
            '<div class="item-title-row"><div class="item-title">' + esc(p.label) + '</div>' +
              '<div class="item-after">' +
                '<span class="fixture-swatch">' + fixtureSwatch(p) + '</span> ' +
                '<span class="fixture-int">' + fixtureIntensity(p) + '%</span></div></div>' +
            '<div class="item-subtitle">' + esc(prof ? prof.name : p.profile_id) +
              ' &middot; U' + p.universe + ':' + p.start_address + '</div>' +
          '</div>' +
        '</a>' +
      '</li>'
    );
  });
  $$('#fixture-list ul').html(rows.join(''));
  updateFixtureSelUI();
}

function updateFixtureSelUI() {
  const n = S.selFixtures.size;
  const show = S.selMode || n > 0;
  $$('#fixture-level-block').css('display', show ? 'block' : 'none');
  if (show) ensureFixtureRange();
  $$('#fixture-sel-label').text(
    n === 0 ? 'No fixtures selected' : n + ' fixture' + (n > 1 ? 's' : '') + ' selected'
  );
  $$('#fixture-list li').forEach((li) => {
    const id = parseInt(li.getAttribute('data-fixture'), 10);
    const row = li.querySelector('.fixture-row');
    if (row) row.classList.toggle('ec-fx-sel', S.selFixtures.has(id));
  });
}

function setSelectedFixturesLevel(level, quiet) {
  level = Math.round(level);
  if (!Number.isFinite(level)) return;
  if (S.selFixtures.size === 0) {
    if (!quiet) {
      f7.toast.create({ text: 'Select fixtures first', closeTimeout: 1500 }).open();
    }
    return;
  }
  const ids = Array.from(S.selFixtures);
  send('set_intensity', { fixture_ids: ids, intensity: level / 100 });
  // Optimistic update where the intensity channel is visible to the client
  // (RGB-only fixtures get virtual intensity server-side; the next push shows it).
  ids.forEach((id) => {
    const patch = S.structure && S.structure.patch.find((p) => p.id === id);
    if (!patch) return;
    const prof = profileOf(patch);
    const uni = S.universes[patch.universe];
    if (!prof || !uni) return;
    const ip = prof.parameters.find((p) => p.is_intensity);
    if (ip) uni[patch.start_address - 1 + ip.offset] = level;
  });
  updateFixtureRows();
}

function updateFixtureRows() {
  const st = S.structure;
  if (!st) return;
  $$('#fixture-list li').forEach((li) => {
    const id = parseInt(li.getAttribute('data-fixture'), 10);
    const patch = st.patch.find((p) => p.id === id);
    if (!patch) return;
    const intEl = li.querySelector('.fixture-int');
    if (intEl) intEl.textContent = fixtureIntensity(patch) + '%';
    const swEl = li.querySelector('.fixture-swatch');
    if (swEl) swEl.innerHTML = fixtureSwatch(patch);
  });
}

$$(document).on('click', '.fixture-row', function (e) {
  e.preventDefault();
  const id = parseInt($$(this).parent('li').attr('data-fixture'), 10);
  if (S.selMode) {
    if (S.selFixtures.has(id)) S.selFixtures.delete(id);
    else S.selFixtures.add(id);
    updateFixtureSelUI();
    return;
  }
  openFixtureSheet(id);
});

$$('#fixture-select-btn').on('click', (e) => {
  e.preventDefault();
  S.selMode = !S.selMode;
  if (!S.selMode && S.selFixtures.size === 0) {
    $$('#fixture-level-block').css('display', 'none');
  }
  $$('#fixture-select-btn').text(S.selMode ? 'Done' : 'Select');
  updateFixtureSelUI();
});

$$('#fixture-level-block .button[data-level]').on('click', function (e) {
  e.preventDefault();
  setSelectedFixturesLevel(parseInt(this.getAttribute('data-level'), 10));
});

$$('#fixture-clear-sel').on('click', (e) => {
  e.preventDefault();
  S.selFixtures.clear();
  updateFixtureSelUI();
});

function channelValue(patch, offset) {
  const uni = S.universes[patch.universe];
  return uni ? (uni[patch.start_address - 1 + offset] || 0) : 0;
}

function openFixtureSheet(fixtureId) {
  const st = S.structure;
  const patch = st.patch.find((p) => p.id === fixtureId);
  if (!patch) return;
  const prof = profileOf(patch);
  if (!prof) return;

  closeFixtureSheet();

  const hasVirtualInt = !prof.has_intensity && prof.is_rgb;
  const sliderParams = prof.parameters.filter((p) => {
    if (prof.is_rgb && ['red', 'green', 'blue'].includes(p.key)) return false; // color wheel
    return true;
  });
  const intParams = sliderParams.filter((p) => p.is_intensity);
  const otherParams = sliderParams.filter((p) => !p.is_intensity);

  let inner = '';
  // Intensity first (dedicated channel or virtual for RGB-only fixtures).
  if (hasVirtualInt) {
    inner +=
      '<div class="block-title">Intensity</div>' +
        '<div class="block"><div class="range-slider" data-param="virtual_int">' +
          '<input type="range" min="0" max="100" step="1" value="0"></div></div>';
  } else {
    intParams.forEach((p) => {
      inner +=
        '<div class="block-title">' + esc(p.label) + '</div>' +
        '<div class="block"><div class="range-slider" data-param="offset:' + p.offset + '">' +
          '<input type="range" min="0" max="100" step="1" value="0"></div></div>';
    });
  }
  // RGB colour picker right after intensity, previewing the current colour.
  if (prof.is_rgb) {
    inner +=
      '<div class="block"><a href="#" class="button button-outline" id="fixture-color-btn" ' +
      'style="display:flex;align-items:center;justify-content:center;gap:8px;">' +
      '<span class="ec-swatch" id="fixture-color-swatch"></span> RGB colour</a></div>';
  }
  // The rest of the parameters.
  otherParams.forEach((p) => {
    inner +=
      '<div class="block-title">' + esc(p.label) + '</div>' +
      '<div class="block"><div class="range-slider" data-param="offset:' + p.offset + '">' +
        '<input type="range" min="0" max="100" step="1" value="0"></div></div>';
  });

  const sheet = f7.sheet.create({
    content:
      '<div class="sheet-modal" style="height: 70%;">' +
        '<div class="toolbar">' +
          '<div class="toolbar-inner">' +
            '<div class="left" style="padding-left: 16px;"><b>' + esc(patch.label) + '</b></div>' +
            '<div class="right"><a href="#" class="link sheet-close">Done</a></div>' +
          '</div>' +
        '</div>' +
        '<div class="sheet-modal-inner"><div class="page-content" id="fixture-sheet-content">' +
          inner +
        '</div></div>' +
      '</div>',
    backdrop: true,
    on: { closed: () => closeFixtureSheet() },
  });
  sheet.open();

  const ranges = {};
  $$('#fixture-sheet-content .range-slider').forEach((el) => {
    const key = el.getAttribute('data-param');
    let value;
    if (key === 'virtual_int') {
      value = fixtureIntensity(patch);
    } else {
      value = channelValue(patch, parseInt(key.split(':')[1], 10));
    }
    ranges[key] = f7.range.create({
      el,
      min: 0,
      max: 100,
      step: 1,
      value,
      label: true,
      on: {
        change(range) {
          if (S.suppress) return;
          holdKey('fx' + fixtureId + ':' + key);
          const value = scalarRangeValue(range);
          if (Number.isFinite(value)) sendFixtureParam(patch, prof, key, Math.round(value));
        },
      },
    });
  });

  if (prof.is_rgb) {
    $$('#fixture-color-btn').on('click', (e) => {
      e.preventDefault();
      openColorWheel(patch, prof);
    });
  }

  S.fixtureSheet = { sheet, fixtureId, patch, prof, ranges };
  updateFixtureColorButton();
}

/// CSS `rgb(...)` for a fixture's current colour ('' when dark), used by the
/// colour button swatch.
function fixtureRgbStyle(patch, prof) {
  const uni = S.universes[patch.universe];
  if (!uni) return '';
  const get = (k) => {
    const p = cwParam(prof, k);
    return p ? Math.round((channelValue(patch, p.offset) || 0) * 2.55) : 0;
  };
  return 'rgb(' + get('red') + ',' + get('green') + ',' + get('blue') + ')';
}

function updateFixtureColorButton() {
  const fs = S.fixtureSheet;
  if (!fs || !fs.prof.is_rgb) return;
  const swatch = document.querySelector('#fixture-color-swatch');
  if (!swatch) return;
  const bg = fixtureRgbStyle(fs.patch, fs.prof);
  swatch.style.background = bg || '#333';
}

function closeFixtureSheet() {
  const fs = S.fixtureSheet;
  if (!fs) return;
  S.fixtureSheet = null;
  closeColorWheel();
  Object.values(fs.ranges).forEach((r) => { try { r.destroy(); } catch (e) {} });
  // destroy() alone leaves the sheet element (and its duplicate IDs) in the
  // DOM — remove it explicitly or stale sheets swallow later interactions.
  const el = fs.sheet && fs.sheet.el;
  try { fs.sheet.destroy(); } catch (e) {}
  if (el && el.parentNode) el.parentNode.removeChild(el);
}

function sendFixtureParam(patch, prof, key, value) {
  if (key === 'virtual_int') {
    send('set_intensity', { fixture_ids: [patch.id], intensity: value / 100 });
    return;
  }
  const offset = parseInt(key.split(':')[1], 10);
  sendParams(patch, { [offset]: value });
}

function sendParams(patch, values) {
  send('set_params', { fixture_id: patch.id, values });
  // Optimistic local update so swatches/rows track the finger.
  const uni = S.universes[patch.universe];
  if (uni) {
    Object.keys(values).forEach((off) => {
      uni[patch.start_address - 1 + parseInt(off, 10)] = values[off];
    });
  }
  updateFixtureRows();
}

// ------------------------------------------------------- colour wheel ----
// Custom canvas wheel matching the desktop circular HSV picker (value = 1);
// brightness is left to the intensity/virtual-intensity slider.

function cwParam(prof, key) {
  const profileKey = { r: 'red', g: 'green', b: 'blue' }[key] || key;
  return prof.parameters.find((p) => p.key === profileKey) || null;
}

function openColorWheel(patch, prof) {
  buildColorWheelPopup(patch.label || ('#' + patch.id), () => {
    const rgb = { r: 0, g: 0, b: 0 };
    Object.keys(rgb).forEach((k) => {
      const p = cwParam(prof, k);
      if (p) rgb[k] = channelValue(patch, p.offset);
    });
    return rgb;
  }, (rgb) => {
    const values = {};
    Object.keys(rgb).forEach((k) => {
      const p = cwParam(prof, k);
      if (p) values[p.offset] = Math.round(rgb[k]);
    });
    if (Object.keys(values).length) {
      sendParams(patch, values);
      updateFixtureColorButton();
    }
  });
}

/// Colour wheel over the current fixture selection (Groups recall): applies
/// the picked colour to every RGB-capable selected fixture.
function openGroupColorWheel() {
  const targets = Array.from(S.selFixtures)
    .map((id) => {
      const patch = S.structure && S.structure.patch.find((p) => p.id === id);
      const prof = patch && profileOf(patch);
      if (!patch || !prof || !prof.is_rgb) return null;
      return { patch, prof };
    })
    .filter(Boolean);
  if (targets.length === 0) {
    f7.toast.create({ text: 'No RGB fixtures in selection', closeTimeout: 2000 }).open();
    return;
  }
  buildColorWheelPopup('Group colour', () => {
    const t = targets[0];
    const rgb = { r: 0, g: 0, b: 0 };
    Object.keys(rgb).forEach((k) => {
      const p = cwParam(t.prof, k);
      if (p) rgb[k] = channelValue(t.patch, p.offset);
    });
    return rgb;
  }, (rgb) => {
    targets.forEach((t) => {
      const values = {};
      Object.keys(rgb).forEach((k) => {
        const p = cwParam(t.prof, k);
        if (p) values[p.offset] = Math.round(rgb[k]);
      });
      if (Object.keys(values).length) sendParams(t.patch, values);
    });
  });
}

/// Shared circular HSV wheel overlay. `readRgb` returns the current colour as
/// {r,g,b} 0–100; `apply(rgb)` is called on every change. A plain DOM overlay
/// (not an F7 popup) so the wheel is fully synchronous and reliable.
function buildColorWheelPopup(title, readRgb, apply) {
  closeColorWheel();
  const SIZE = 280;

  const overlay = document.createElement('div');
  overlay.className = 'ec-wheel-overlay';
  overlay.innerHTML =
    '<div class="ec-wheel-card">' +
      '<div class="ec-wheel-title">' +
        '<span>' + esc(title) + '</span>' +
        '<a href="#" class="link cw-close">Done</a>' +
      '</div>' +
      '<div class="ec-wheel-wrap">' +
        '<canvas class="ec-wheel" id="cw-canvas" width="' + SIZE + '" height="' + SIZE + '"></canvas>' +
        '<div class="block-title" style="margin-top:12px;margin-bottom:0;">RGB</div>' +
        '<div style="width:100%;margin-top:4px;">' +
          '<div class="range-slider" data-cw="r"><input type="range" min="0" max="100" step="1" value="0"></div>' +
          '<div class="range-slider" data-cw="g"><input type="range" min="0" max="100" step="1" value="0"></div>' +
          '<div class="range-slider" data-cw="b"><input type="range" min="0" max="100" step="1" value="0"></div>' +
        '</div>' +
      '</div>' +
    '</div>';
  document.body.appendChild(overlay);

  const canvas = overlay.querySelector('#cw-canvas');
  const ctx = canvas.getContext('2d');

  // Current colour from the universe (0–100 internal range).
  const rgb = readRgb();
  const cw = { overlay, canvas, ctx, rgb, h: 0, s: 0 };
  const [h0, s0] = rgbToHsv(rgb.r / 100, rgb.g / 100, rgb.b / 100);
  cw.h = h0; cw.s = s0;

  ctx.clearRect(0, 0, SIZE, SIZE);
  ctx.drawImage(getWheelCanvas(), 0, 0, SIZE, SIZE);

  function drawCrosshair() {
    const cx = SIZE / 2, cy = SIZE / 2, radius = SIZE / 2;
    const ang = cw.h * Math.PI * 2;
    const dist = cw.s * radius;
    const x = cx + Math.cos(ang) * dist;
    const y = cy + Math.sin(ang) * dist;
    ctx.beginPath(); ctx.arc(x, y, 8.5, 0, Math.PI * 2);
    ctx.strokeStyle = '#000'; ctx.lineWidth = 3; ctx.stroke();
    ctx.beginPath(); ctx.arc(x, y, 7, 0, Math.PI * 2);
    ctx.strokeStyle = '#fff'; ctx.lineWidth = 1.5; ctx.stroke();
  }
  drawCrosshair();

  function commit(rgbNew, h, s) {
    cw.rgb = rgbNew;
    if (h != null) { cw.h = h; cw.s = s; }
    else { const [hh, ss] = rgbToHsv(cw.rgb.r / 100, cw.rgb.g / 100, cw.rgb.b / 100); cw.h = hh; cw.s = ss; }
    ctx.clearRect(0, 0, SIZE, SIZE);
    ctx.drawImage(getWheelCanvas(), 0, 0, SIZE, SIZE);
    drawCrosshair();
    syncSliders();
    holdKey('cw-popup', 700);
    apply(cw.rgb);
  }

  function pickFromEvent(ev) {
    const rect = canvas.getBoundingClientRect();
    const cx = rect.width / 2, cy = rect.height / 2;
    const radius = Math.min(rect.width, rect.height) / 2;
    const dx = ev.clientX - rect.left - cx;
    const dy = ev.clientY - rect.top - cy;
    const dist = Math.hypot(dx, dy);
    if (dist > radius) return;
    const h = ((Math.atan2(dy, dx) / (Math.PI * 2)) + 1) % 1;
    const s = Math.min(1, dist / radius);
    const [r, g, b] = hsvToRgb(h, s, 1);
    commit({
      r: Math.round(r / 2.55),
      g: Math.round(g / 2.55),
      b: Math.round(b / 2.55),
    }, h, s);
  }

  // Track drag state explicitly — iOS Safari reports `buttons: 0` on touch
  // pointermove, so the usual `ev.buttons & 1` guard would swallow drags.
  let dragging = false;
  canvas.addEventListener('pointerdown', (ev) => {
    ev.preventDefault();
    dragging = true;
    pickFromEvent(ev);
    try { canvas.setPointerCapture(ev.pointerId); } catch (e) {}
  });
  canvas.addEventListener('pointermove', (ev) => {
    if (!dragging) return;
    ev.preventDefault();
    pickFromEvent(ev);
  });
  const endDrag = (ev) => {
    dragging = false;
    try { canvas.releasePointerCapture(ev.pointerId); } catch (e) {}
  };
  canvas.addEventListener('pointerup', endDrag);
  canvas.addEventListener('pointercancel', endDrag);

  // RGB sliders mirror the wheel.
  const ranges = { r: null, g: null, b: null };
  Object.keys(ranges).forEach((k) => {
    const sliderEl = overlay.querySelector('.range-slider[data-cw="' + k + '"]');
    ranges[k] = f7.range.create({
      el: sliderEl,
      min: 0, max: 100, step: 1, value: cw.rgb[k], label: true,
      on: {
        change(range) {
          if (S.suppress) return;
          const rgbNew = { r: cw.rgb.r, g: cw.rgb.g, b: cw.rgb.b };
          const value = scalarRangeValue(range);
          if (!Number.isFinite(value)) return;
          rgbNew[k] = Math.round(value);
          commit(rgbNew);
        },
      },
    });
  });

  function syncSliders() {
    Object.keys(ranges).forEach((k) => {
      if (!isHeld('cw-popup')) {
        S.suppress = true;
        ranges[k].setValue(cw.rgb[k]);
        S.suppress = false;
      }
    });
  }

  overlay.querySelector('.cw-close').addEventListener('click', (e) => {
    e.preventDefault();
    closeColorWheel();
  });
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) closeColorWheel();
  });

  S.colorWheel = { overlay, canvas, ctx, ranges, rgb, h: cw.h, s: cw.s, syncSliders, readRgb };
}

function closeColorWheel() {
  const cw = S.colorWheel;
  if (!cw) return;
  S.colorWheel = null;
  Object.values(cw.ranges).forEach((r) => { try { r.destroy(); } catch (e) {} });
  const el = cw.overlay;
  if (el && el.parentNode) el.parentNode.removeChild(el);
}

/// Refresh an open colour wheel from pushed channel values (unless dragging).
function updateColorWheel() {
  const cw = S.colorWheel;
  if (!cw || isHeld('cw-popup')) return;
  cw.rgb = cw.readRgb();
  const [h, s] = rgbToHsv(cw.rgb.r / 100, cw.rgb.g / 100, cw.rgb.b / 100);
  cw.h = h; cw.s = s;
  const SIZE = cw.canvas.width;
  const ctx = cw.ctx;
  ctx.clearRect(0, 0, SIZE, SIZE);
  ctx.drawImage(getWheelCanvas(), 0, 0, SIZE, SIZE);
  const cx = SIZE / 2, cy = SIZE / 2, radius = SIZE / 2;
  const ang = h * Math.PI * 2, dist = s * radius;
  const x = cx + Math.cos(ang) * dist, y = cy + Math.sin(ang) * dist;
  ctx.beginPath(); ctx.arc(x, y, 8.5, 0, Math.PI * 2);
  ctx.strokeStyle = '#000'; ctx.lineWidth = 3; ctx.stroke();
  ctx.beginPath(); ctx.arc(x, y, 7, 0, Math.PI * 2);
  ctx.strokeStyle = '#fff'; ctx.lineWidth = 1.5; ctx.stroke();
  cw.syncSliders();
}

/// Refresh open sheet controls from pushed state (unless the user is dragging).
function updateFixtureSheet() {
  const fs = S.fixtureSheet;
  if (!fs) return;
  Object.keys(fs.ranges).forEach((key) => {
    if (isHeld('fx' + fs.fixtureId + ':' + key)) return;
    let value;
    if (key === 'virtual_int') {
      value = fixtureIntensity(fs.patch);
    } else {
      value = channelValue(fs.patch, parseInt(key.split(':')[1], 10));
    }
    S.suppress = true;
    fs.ranges[key].setValue(value);
    S.suppress = false;
  });
  updateFixtureColorButton();
}

// -------------------------------------------------------------- channels ----

let channelRange = null;
let channelRangePending = false;

function ensureChannelRange() {
  if (channelRange || channelRangePending) return channelRange;
  channelRangePending = true;
  requestAnimationFrame(() => {
    channelRangePending = false;
    const el = document.querySelector('#channel-range');
    if (!el || el.offsetWidth === 0) {
      setTimeout(ensureChannelRange, 100);
      return;
    }
    channelRange = f7.range.create({
      el,
      min: 0,
      max: 100,
      step: 1,
      value: 0,
      label: true,
      on: {
        // Live while dragging (throttled so venue wifi isn't flooded)…
        change(range) {
          if (S.suppress) return;
          const now = Date.now();
          if (now - channelRangeLastSend >= 80) {
            channelRangeLastSend = now;
            const value = scalarRangeValue(range);
            if (Number.isFinite(value)) sendSelectedLevel(Math.round(value), true);
          }
        },
        // …and the exact final value on release.
        changed(range) {
          if (S.suppress) return;
          const value = scalarRangeValue(range);
          if (Number.isFinite(value)) sendSelectedLevel(Math.round(value));
        },
      },
    });
  });
  return channelRange;
}

function patchedChannels(universeId) {
  const set = {};
  if (!S.structure) return set;
  S.structure.patch.forEach((p) => {
    if (p.universe !== universeId) return;
    const prof = profileOf(p);
    const count = prof ? prof.channel_count : 1;
    for (let c = p.start_address; c < p.start_address + count && c <= 512; c++) {
      set[c] = true;
    }
  });
  return set;
}

function renderChannelGrid() {
  const uni = S.universes[S.curUniverse] || [];
  const patched = patchedChannels(S.curUniverse);
  let html = '';
  for (let ch = 1; ch <= 512; ch++) {
    const v = uni[ch - 1] || 0;
    const cls =
      'ec-cell' +
      (v > 0 ? ' ec-on' : '') +
      (S.selected.has(ch) ? ' ec-sel' : '') +
      (patched[ch] ? ' ec-patched' : '');
    html +=
      '<div class="' + cls + '" data-ch="' + ch + '">' +
        '<span class="ec-num">' + ch + '</span>' +
        '<span class="ec-val">' + (v > 0 ? v : '–') + '</span>' +
      '</div>';
  }
  $$('#channel-grid').html(html);
  $$('#universe-btn').text('U' + S.curUniverse);
  updateChannelSelLabel();
}

function updateChannelCells() {
  const uni = S.universes[S.curUniverse] || [];
  $$('#channel-grid .ec-cell').forEach((el) => {
    const ch = parseInt(el.getAttribute('data-ch'), 10);
    const v = uni[ch - 1] || 0;
    el.querySelector('.ec-val').textContent = v > 0 ? v : '–';
    el.classList.toggle('ec-on', v > 0);
    el.classList.toggle('ec-sel', S.selected.has(ch));
  });
}

function updateChannelSelLabel() {
  const n = S.selected.size;
  $$('#channel-sel-label').text(
    n === 0 ? 'No channels selected' : n + ' channel' + (n > 1 ? 's' : '') + ' selected'
  );
}

$$(document).on('click', '#channel-grid .ec-cell', function () {
  const ch = parseInt(this.getAttribute('data-ch'), 10);
  if (S.selected.has(ch)) S.selected.delete(ch);
  else S.selected.add(ch);
  this.classList.toggle('ec-sel');
  updateChannelSelLabel();
});

$$('#channel-clear-sel').on('click', (e) => {
  e.preventDefault();
  S.selected.clear();
  updateChannelCells();
  updateChannelSelLabel();
});

function sendSelectedLevel(level, quiet) {
  level = Math.round(level);
  if (!Number.isFinite(level)) return;
  if (S.selected.size === 0) {
    if (!quiet) {
      f7.toast.create({ text: 'Tap channels to select them first', closeTimeout: 1500 }).open();
    }
    return;
  }
  const channels = Array.from(S.selected).map((ch) => ({ channel: ch, value: level }));
  send('set_channels', { universe: S.curUniverse, channels });
  const uni = S.universes[S.curUniverse];
  if (uni) channels.forEach((c) => { uni[c.channel - 1] = c.value; });
  updateChannelCells();
  updateFixtureRows();
}

$$('#channel-level-block .button[data-level]').on('click', function (e) {
  e.preventDefault();
  sendSelectedLevel(parseInt(this.getAttribute('data-level'), 10));
});

$$('#universe-btn').on('click', (e) => {
  e.preventDefault();
  const st = S.structure;
  const ids = st ? st.active_universes : [1];
  f7.actions.create({
    buttons: [
      ids.map((id) => ({
        text: 'Universe ' + id,
        bold: id === S.curUniverse,
        onClick: () => {
          S.curUniverse = id;
          S.selected.clear();
          renderChannelGrid();
        },
      })),
      [{ text: 'Cancel', color: 'red' }],
    ],
  }).open();
});

// ------------------------------------------------------------------ patch ----

function renderPatch() {
  const st = S.structure;
  if (!st) return;
  const rows = [...st.patch].sort((a, b) => a.id - b.id).map((p) => {
    const prof = profileOf(p);
    const count = prof ? prof.channel_count : 1;
    const end = p.start_address + count - 1;
    const range = 'U' + p.universe + ':' + p.start_address + (count > 1 ? '–' + end : '');
    return (
      '<li data-patch="' + p.id + '">' +
        '<a href="#" class="item-link item-content patch-row">' +
          '<div class="item-media"><span class="badge">' + p.id + '</span></div>' +
          '<div class="item-inner">' +
            '<div class="item-title-row"><div class="item-title">' + esc(p.label) + '</div>' +
              '<div class="item-after">' + range + '</div></div>' +
            '<div class="item-subtitle">' + esc(prof ? prof.name : p.profile_id) +
              ' &middot; ' + count + ' ch</div>' +
          '</div>' +
        '</a>' +
      '</li>'
    );
  });
  $$('#patch-list ul').html(rows.join(''));
  $$('#patch-empty').css('display', st.patch.length ? 'none' : 'block');
}

function patchInputLi(label, id, type, value, extra) {
  return (
    '<li class="item-content item-input">' +
      '<div class="item-inner">' +
        '<div class="item-title item-label">' + label + '</div>' +
        '<div class="item-input-wrap">' +
          '<input type="' + type + '" id="' + id + '" value="' + esc(value) + '"' +
          (type === 'number' ? ' inputmode="numeric" min="1"' : '') + (extra || '') + '>' +
        '</div>' +
      '</div>' +
    '</li>'
  );
}

function openPatchSheet(existing) {
  const st = S.structure;
  if (!st) return;
  const isEdit = !!existing;

  let profileLi = '';
  let heading;
  let prefill;
  if (isEdit) {
    heading = 'Edit #' + existing.id;
    prefill = existing;
  } else {
    heading = 'Add Fixture';
    const nextId = st.patch.reduce((m, p) => Math.max(m, p.id), 0) + 1;
    // Suggest the first free address after everything patched in universe 1.
    const nextAddr = st.patch
      .filter((p) => p.universe === 1)
      .reduce((m, p) => {
        const prof = profileOf(p);
        return Math.max(m, p.start_address + (prof ? prof.channel_count : 1));
      }, 1);
    prefill = { id: nextId, label: 'Fixture ' + nextId, universe: 1, start_address: nextAddr };
  }
  // Profile picker — always a dropdown so existing fixtures can change profile
  // too (the console re-patches, validating the new address range).
  const options = Object.keys(st.profiles)
    .sort((a, b) => st.profiles[a].name.localeCompare(st.profiles[b].name))
    .map((pid) =>
      '<option value="' + esc(pid) + '"' +
      (isEdit && pid === existing.profile_id ? ' selected' : '') + '>' +
      esc(st.profiles[pid].name) + ' (' + st.profiles[pid].channel_count + ' ch)</option>')
    .join('');
  profileLi =
    '<li class="item-content item-input">' +
      '<div class="item-inner">' +
        '<div class="item-title item-label">Profile</div>' +
        '<div class="item-input-wrap"><select id="patch-profile">' + options + '</select></div>' +
      '</div>' +
    '</li>';

  const sheet = f7.sheet.create({
    content:
      '<div class="sheet-modal" style="height: auto;">' +
        '<div class="toolbar">' +
          '<div class="toolbar-inner">' +
            '<div class="left" style="padding-left: 16px;"><b>' + heading + '</b></div>' +
            '<div class="right"><a href="#" class="link sheet-close">Cancel</a></div>' +
          '</div>' +
        '</div>' +
        '<div class="sheet-modal-inner"><div class="page-content">' +
          '<div class="list"><ul>' +
            patchInputLi('Label', 'patch-label', 'text', prefill.label) +
            profileLi +
            (isEdit ? patchInputLi('Fixture #', 'patch-id', 'number', prefill.id) : '') +
            patchInputLi('Universe', 'patch-universe', 'number', prefill.universe, ' max="8"') +
            patchInputLi('DMX address', 'patch-address', 'number', prefill.start_address, ' max="512"') +
          '</ul></div>' +
          '<div class="block">' +
            '<div class="grid ' + (isEdit ? 'grid-cols-2' : 'grid-cols-1') + ' grid-gap">' +
              '<a href="#" class="button button-fill" id="patch-save">' +
                (isEdit ? 'Save' : 'Patch It') + '</a>' +
              (isEdit ? '<a href="#" class="button button-outline color-red" id="patch-delete">Delete</a>' : '') +
            '</div>' +
          '</div>' +
        '</div></div>' +
      '</div>',
    backdrop: true,
    on: {
      closed(s) {
        // Remove the element too — destroy() alone leaks it into the DOM
        // with duplicate input IDs.
        const el = s.el;
        try { s.destroy(); } catch (err) {}
        if (el && el.parentNode) el.parentNode.removeChild(el);
      },
    },
  });
  sheet.open();

  $$('#patch-save').on('click', (e) => {
    e.preventDefault();
    const label = $$('#patch-label').val().trim() || prefill.label;
    const universe = parseInt($$('#patch-universe').val(), 10);
    const address = parseInt($$('#patch-address').val(), 10);
    if (!(universe >= 1 && universe <= 8) || !(address >= 1 && address <= 512)) {
      f7.toast.create({ text: 'Universe 1–8, address 1–512', closeTimeout: 2000 }).open();
      return;
    }
    if (isEdit) {
      const newId = parseInt($$('#patch-id').val(), 10);
      if (!(newId >= 1)) {
        f7.toast.create({ text: 'Fixture # must be 1 or higher', closeTimeout: 2000 }).open();
        return;
      }
      send('patch_update', {
        id: existing.id,
        label,
        new_id: newId,
        profile_id: $$('#patch-profile').val(),
        universe,
        start_address: address,
      });
    } else {
      send('patch_add', {
        label,
        profile_id: $$('#patch-profile').val(),
        universe,
        start_address: address,
      });
    }
    sheet.close();
  });

  if (isEdit) {
    $$('#patch-delete').on('click', (e) => {
      e.preventDefault();
      f7.dialog.confirm('Remove ' + esc(existing.label) + ' from the patch?', 'Delete Fixture', () => {
        send('patch_remove', { id: existing.id });
        sheet.close();
      });
    });
  }
}

$$('#patch-add-btn').on('click', (e) => {
  e.preventDefault();
  openPatchSheet(null);
});

$$(document).on('click', '.patch-row', function (e) {
  e.preventDefault();
  const id = parseInt($$(this).parent('li').attr('data-patch'), 10);
  const patch = S.structure && S.structure.patch.find((p) => p.id === id);
  if (patch) openPatchSheet(patch);
});

// ---------------------------------------------------------------- groups ----

function renderGroups() {
  const st = S.structure;
  if (!st) return;
  const has = st.groups.length > 0;
  $$('#group-empty').css('display', has ? 'none' : 'block');
  const rows = st.groups.map((g) => {
    const names = (g.fixtures || []).map((id) => {
      const p = st.patch.find((q) => q.id === id);
      return p ? ('#' + id + (p.label ? ' ' + p.label : '')) : ('#' + id);
    });
    const sel = S.recalledGroup === g.id ? ' ec-gp-sel' : '';
    return (
      '<li data-group="' + g.id + '">' +
        '<a href="#" class="item-link item-content group-row' + sel + '">' +
          '<div class="item-media"><span class="badge">G' + g.id + '</span></div>' +
          '<div class="item-inner">' +
            '<div class="item-title-row"><div class="item-title">' +
              esc(g.label || ('Group ' + g.id)) + '</div>' +
              '<div class="item-after">' + g.fixtures.length + ' fx</div></div>' +
            '<div class="item-subtitle">' + esc(names.join(', ')) + '</div>' +
          '</div>' +
        '</a>' +
      '</li>'
    );
  });
  $$('#group-list ul').html(rows.join(''));
}

$$(document).on('click', '.group-row', function (e) {
  e.preventDefault();
  const gid = parseInt($$(this).parent('li').attr('data-group'), 10);
  const g = S.structure && S.structure.groups.find((x) => x.id === gid);
  if (!g) return;
  S.selFixtures = new Set(g.fixtures || []);
  S.selMode = true;
  S.recalledGroup = gid;
  $$('#fixture-select-btn').text('Done');
  $$('#group-list li').forEach((li) => {
    const id = parseInt(li.getAttribute('data-group'), 10);
    const row = li.querySelector('.group-row');
    if (row) row.classList.toggle('ec-gp-sel', id === gid);
  });
  $$('#group-recalled-label').html(
    'Recalled <b>G' + gid + '</b> — ' + g.fixtures.length + ' fixture' +
    (g.fixtures.length === 1 ? '' : 's') + '. Set a level:'
  );
  $$('#group-recalled').css('display', 'block');
  ensureGroupRange();
  updateFixtureSelUI();
  f7.toast.create({
    text: 'Recalled G' + gid + ' (' + g.fixtures.length + ' fx)',
    closeTimeout: 1500,
  }).open();
});

$$('#group-recalled .button[data-level]').on('click', function (e) {
  e.preventDefault();
  setSelectedFixturesLevel(parseInt(this.getAttribute('data-level'), 10));
});

$$('#group-color-btn').on('click', (e) => {
  e.preventDefault();
  openGroupColorWheel();
});

$$('#group-clear-btn').on('click', (e) => {
  e.preventDefault();
  S.selFixtures.clear();
  S.recalledGroup = null;
  $$('#group-recalled').css('display', 'none');
  $$('#group-list li .group-row').forEach((row) => row.classList.remove('ec-gp-sel'));
  updateFixtureSelUI();
});

$$('#client-version').text('EasyCue3 Remote v' + CLIENT_VERSION);

// ------------------------------------------------------------ command line ----

$$('#ctx-fixture').on('click', function (e) {
  e.preventDefault();
  S.cmdContext = 'fixture';
  $$('#ctx-fixture').addClass('tab-link-active');
  $$('#ctx-channel').removeClass('tab-link-active');
});
$$('#ctx-channel').on('click', function (e) {
  e.preventDefault();
  S.cmdContext = 'channel';
  $$('#ctx-channel').addClass('tab-link-active');
  $$('#ctx-fixture').removeClass('tab-link-active');
});

function sendCommand() {
  const input = $$('#cmd-input');
  const text = input.val().trim();
  if (!text) return;
  send('command_line', { text, context: S.cmdContext });
  input.val('');
}

$$('#cmd-send').on('click', (e) => { e.preventDefault(); sendCommand(); });
$$('#cmd-form').on('submit', (e) => { e.preventDefault(); sendCommand(); });

function appendLog(text, reply) {
  S.logCount++;
  const html =
    '<li class="item-content">' +
      '<div class="item-inner">' +
        '<div class="item-title">' +
          '<div class="item-header">&gt; ' + esc(text) + '</div>' +
          esc(reply || '') +
        '</div>' +
      '</div>' +
    '</li>';
  const list = $$('#cmd-log ul');
  list.prepend(html);
  const items = list.children('li');
  if (items.length > 100) items.eq(items.length - 1).remove();
}

// ------------------------------------------------------------- rendering ----

function renderStructure() {
  renderCues();
  renderFixtures();
  renderChannelGrid();
  renderGroups();
  renderPatch();
}

let liveRaf = null;
function renderLiveValues() {
  // Coalesce bursts of channel pushes into one paint.
  if (liveRaf) return;
  liveRaf = requestAnimationFrame(() => {
    liveRaf = null;
    updateChannelCells();
    updateFixtureRows();
    updateFixtureSheet();
    updateColorWheel();
  });
}

// ----------------------------------------------------------------- init ----

masterRange = f7.range.create({
  el: '#master-range',
  min: 0,
  max: 100,
  step: 1,
  value: 100,
  label: true,
  on: {
    change(range) {
      if (S.suppress) return;
      holdKey('master');
      const value = scalarRangeValue(range);
      if (Number.isFinite(value)) send('set_master', { value: value / 100 });
    },
  },
});

let soundMasterRange = null;
soundMasterRange = f7.range.create({
  el: '#sound-master-range',
  min: 0,
  max: 100,
  step: 1,
  value: 100,
  label: true,
  on: {
    change(range) {
      if (S.suppress) return;
      holdKey('sndmaster');
      const value = scalarRangeValue(range);
      if (Number.isFinite(value)) send('set_sound_master', { value: value / 100 });
    },
  },
});

let channelRangeLastSend = 0;

let fixtureRangeLastSend = 0;
let fixtureRange = null;
let fixtureRangePending = false;
// Created lazily on first show — F7 range components created while their
// parent is display:none can report undefined values on first interaction.
function ensureFixtureRange() {
  if (fixtureRange || fixtureRangePending) return fixtureRange;
  fixtureRangePending = true;
  requestAnimationFrame(() => {
    fixtureRangePending = false;
    const el = document.querySelector('#fixture-range');
    if (!el || el.offsetWidth === 0) {
      setTimeout(ensureFixtureRange, 100);
      return;
    }
    fixtureRange = f7.range.create({
      el,
      min: 0,
      max: 100,
      step: 1,
      value: 0,
      label: true,
      on: {
        change(range) {
          if (S.suppress) return;
          const now = Date.now();
          if (now - fixtureRangeLastSend >= 80) {
            fixtureRangeLastSend = now;
            const value = scalarRangeValue(range);
            if (Number.isFinite(value)) setSelectedFixturesLevel(Math.round(value), true);
          }
        },
        changed(range) {
          if (S.suppress) return;
          const value = scalarRangeValue(range);
          if (Number.isFinite(value)) setSelectedFixturesLevel(Math.round(value));
        },
      },
    });
  });
  return fixtureRange;
}

let groupRangeLastSend = 0;
let groupRange = null;
let groupRangePending = false;
function ensureGroupRange() {
  if (groupRange || groupRangePending) return groupRange;
  groupRangePending = true;
  requestAnimationFrame(() => {
    groupRangePending = false;
    const el = document.querySelector('#group-range');
    if (!el || el.offsetWidth === 0) {
      setTimeout(ensureGroupRange, 100);
      return;
    }
    groupRange = f7.range.create({
      el,
      min: 0,
      max: 100,
      step: 1,
      value: 0,
      label: true,
      on: {
        change(range) {
          if (S.suppress) return;
          const now = Date.now();
          if (now - groupRangeLastSend >= 80) {
            groupRangeLastSend = now;
            const value = scalarRangeValue(range);
            if (Number.isFinite(value)) setSelectedFixturesLevel(Math.round(value), true);
          }
        },
        changed(range) {
          if (S.suppress) return;
          const value = scalarRangeValue(range);
          if (Number.isFinite(value)) setSelectedFixturesLevel(Math.round(value));
        },
      },
    });
  });
  return groupRange;
}

// Ensure the level sliders are built while their view is actually visible
// (F7 range components misbehave when created inside a hidden tab).
$$('.tab-link[href="#view-fixtures"]').on('click', () => setTimeout(ensureFixtureRange, 60));
$$('.tab-link[href="#view-groups"]').on('click', () => setTimeout(ensureGroupRange, 60));
$$('.tab-link[href="#view-channels"]').on('click', () => setTimeout(ensureChannelRange, 60));

connect();
