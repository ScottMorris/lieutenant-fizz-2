// Starts FIZZ2.EXE in js-dos when Play is pressed, and wires the mode picker and the sound, fullscreen and restart buttons.
// The player follows Fizz 1 DOS's site/player.js (lieutenant-fizz-dos): the same js-dos options, mute handling and restart.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

(function () {
  var params = new URLSearchParams(window.location.search);
  var MODES = { live: 'Play', replay: 'Replay', check: 'Check screen', bench: 'Bench', help: 'Help', shell: 'DOS prompt' };
  var mode = params.get('mode') in MODES ? params.get('mode') : 'live';
  var stage = document.getElementById('stage');
  var host = document.getElementById('dos');
  var cover = document.getElementById('cover');
  var playButton = document.getElementById('play');
  var muteButton = document.getElementById('mute');
  var fullButton = document.getElementById('full');
  var restartButton = document.getElementById('restart');
  var ci = null;
  var muted = params.has('mute');
  var manifest = null;

  var nav = document.getElementById('modes');
  Object.keys(MODES).forEach(function (key) {
    var link = document.createElement('a');
    link.href = '?mode=' + key;
    link.textContent = MODES[key];
    if (key === mode) link.setAttribute('aria-current', 'page');
    nav.appendChild(link);
  });

  function showMuted() {
    muteButton.textContent = muted ? 'Sound off' : 'Sound on';
    muteButton.setAttribute('aria-pressed', String(muted));
  }

  function applyMute() {
    if (!ci) return;
    if (muted) ci.mute();
    else ci.unmute();
  }

  function launch() {
    host.hidden = false;
    ci = null;
    document.title = 'Lieutenant Fizz 2 DOS (js-dos, build ' + manifest.hash + ')';
    window.Dos(host, {
      url: manifest.bundles[mode],
      pathPrefix: 'js-dos/emulators/',
      autoStart: true,
      // js-dos's own side bar (saves, keyboard, fullscreen, turbo and the CPU load, settings) shows unless ?kiosk hides it, as Fizz 1's page does.
      kiosk: params.has('kiosk'),
      noCloud: true,
      theme: 'fizz',
      onEvent: function (event, instance) {
        if (event === 'ci-ready') {
          ci = instance;
          applyMute();
        }
        if (event === 'emu-ready' || event === 'bnd-play') cover.hidden = true;
      },
    });
    restartButton.disabled = false;
  }

  // The manifest is never cached and names the current bundle; the bundle's file name carries a hash of its contents, so js-dos's
  // aggressive caching can never hand back an old build.
  function start() {
    playButton.disabled = true;
    playButton.textContent = 'Loading…';
    fetch('bundles.json', { cache: 'no-store' })
      .then(function (r) { return r.json(); })
      .then(function (m) { manifest = m; launch(); })
      .catch(function (error) {
        playButton.textContent = 'Could not load bundles.json: ' + error;
      });
  }

  // Leaving the game and typing `exit` at the DOS prompt stops the emulator. Restart loads the page again, back to the Play cover: a
  // fresh emulator every time, and the click on Play is the gesture that lets the sound start. (Tearing js-dos down and building a second
  // one on the same element sometimes left a black screen in a real browser.)
  function restart() {
    restartButton.disabled = true;
    window.location.reload();
  }

  muteButton.addEventListener('click', function () {
    muted = !muted;
    showMuted();
    applyMute();
  });
  fullButton.addEventListener('click', function () {
    if (stage.requestFullscreen) stage.requestFullscreen();
  });
  playButton.addEventListener('click', start);
  restartButton.addEventListener('click', restart);
  showMuted();
  if (params.has('autoplay')) start();
})();
