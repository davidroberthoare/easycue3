/* EasyCue3 Remote — Framework7 client.
 *
 * Thin stateless view over the console's WebSocket: renders whatever the
 * server pushes, sends commands back, applies optimistic local updates that
 * the next authoritative push reconciles.
 */
'use strict';

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
  cmdContext: 'fixture',
  hold: {},                 // control key -> timestamp until which pushes are ignored
  suppress: false,          // true while we set range values programmatically
  fixtureSheet: null,       // {sheet, fixtureId, ranges: {key: range}, colorPicker}
  logCount: 0,
};

const $$ = Dom7;

function holdKey(key, ms) { S.hold[key] = Date.now() + (ms || 600); }
function isHeld(key) { return (S.hold[key] || 0) > Date.now(); }

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
    return (
      '<li data-fixture="' + p.id + '">' +
        '<a href="#" class="item-link item-content fixture-row">' +
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
  openFixtureSheet(id);
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

  let inner = '';
  if (hasVirtualInt) {
    inner +=
      '<div class="block-title">Int (virtual)</div>' +
      '<div class="block"><div class="range-slider" data-param="virtual_int"></div></div>';
  }
  sliderParams.forEach((p) => {
    inner +=
      '<div class="block-title">' + esc(p.label) + '</div>' +
      '<div class="block"><div class="range-slider" data-param="offset:' + p.offset + '"></div></div>';
  });
  if (prof.is_rgb) {
    inner +=
      '<div class="block"><a href="#" class="button button-outline" id="fixture-color-btn">' +
      'Colour…</a></div>';
  }

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
        change(range, v) {
          if (S.suppress) return;
          holdKey('fx' + fixtureId + ':' + key);
          sendFixtureParam(patch, prof, key, Math.round(v));
        },
      },
    });
  });

  let colorPicker = null;
  if (prof.is_rgb) {
    const getRgb = (k) => {
      const p = prof.parameters.find((q) => q.key === k);
      return p ? Math.round(channelValue(patch, p.offset) * 2.55) : 0;
    };
    colorPicker = f7.colorPicker.create({
      targetEl: '#fixture-color-btn',
      // Popup gets full screen height — the wheel + sliders were cut off
      // at the bottom in a sheet. cssClass lifts it above the fixture sheet.
      openIn: 'popup',
      popupCssClass: 'ec-color-popup',
      modules: ['wheel', 'rgb-sliders'],
      value: { rgb: [getRgb('red'), getRgb('green'), getRgb('blue')] },
      on: {
        change(cp, value) {
          if (S.suppress || !value || !value.rgb) return;
          holdKey('fxcolor' + fixtureId, 800);
          const values = {};
          const map = { red: 0, green: 1, blue: 2 };
          Object.keys(map).forEach((k) => {
            const p = prof.parameters.find((q) => q.key === k);
            if (p) values[p.offset] = Math.round((value.rgb[map[k]] / 255) * 100);
          });
          sendParams(patch, values);
        },
      },
    });
    // targetEl auto-opens the picker on click — no manual handler needed.
  }

  S.fixtureSheet = { sheet, fixtureId, patch, prof, ranges, colorPicker };
}

function closeFixtureSheet() {
  const fs = S.fixtureSheet;
  if (!fs) return;
  S.fixtureSheet = null;
  Object.values(fs.ranges).forEach((r) => { try { r.destroy(); } catch (e) {} });
  if (fs.colorPicker) { try { fs.colorPicker.destroy(); } catch (e) {} }
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
}

// -------------------------------------------------------------- channels ----

let channelRange = null;

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
    const prof = profileOf(existing);
    heading = 'Edit #' + existing.id;
    prefill = existing;
    profileLi =
      '<li class="item-content"><div class="item-inner">' +
        '<div class="item-title">Profile</div>' +
        '<div class="item-after">' + esc(prof ? prof.name : existing.profile_id) + '</div>' +
      '</div></li>';
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
    const options = Object.keys(st.profiles)
      .sort((a, b) => st.profiles[a].name.localeCompare(st.profiles[b].name))
      .map((pid) =>
        '<option value="' + esc(pid) + '">' + esc(st.profiles[pid].name) +
        ' (' + st.profiles[pid].channel_count + ' ch)</option>')
      .join('');
    profileLi =
      '<li class="item-content item-input">' +
        '<div class="item-inner">' +
          '<div class="item-title item-label">Profile</div>' +
          '<div class="item-input-wrap"><select id="patch-profile">' + options + '</select></div>' +
        '</div>' +
      '</li>';
  }

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
    change(range, v) {
      if (S.suppress) return;
      holdKey('master');
      send('set_master', { value: v / 100 });
    },
  },
});

let channelRangeLastSend = 0;
channelRange = f7.range.create({
  el: '#channel-range',
  min: 0,
  max: 100,
  step: 1,
  value: 0,
  label: true,
  on: {
    // Live while dragging (throttled so venue wifi isn't flooded)…
    change(range, v) {
      if (S.suppress) return;
      const now = Date.now();
      if (now - channelRangeLastSend >= 80) {
        channelRangeLastSend = now;
        sendSelectedLevel(Math.round(v), true);
      }
    },
    // …and the exact final value on release.
    changed(range, v) {
      if (S.suppress) return;
      sendSelectedLevel(Math.round(v));
    },
  },
});

if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('sw.js').catch(() => {});
}

connect();
