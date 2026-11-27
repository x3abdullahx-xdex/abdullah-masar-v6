// Abdullah Masar V6 — 4-Layer Web Audio API Escalating Ring (1–20+ Tasks) & Procedural Ambient Noise
// Replaces V5's "نغمة الإنجاز اليوم" with a serene, zero-asset procedural acoustic engine.
(function () {
  let ctx = null;
  let muted = false;

  // Two-octave pentatonic scale (C4 -> C6) for clean, non-fatiguing escalation across 1..20+ tasks
  const SCALE_HZ = [
    261.63, 293.66, 329.63, 392.00, 440.00,
    523.25, 587.33, 659.25, 783.99, 880.00,
    1046.50
  ];

  function ensureCtx() {
    if (!ctx) {
      const AC = window.AudioContext || window.webkitAudioContext;
      if (!AC) return null;
      ctx = new AC();
    }
    if (ctx.state === 'suspended') ctx.resume().catch(() => {});
    return ctx;
  }

  function createMasterBus(ac) {
    const comp = ac.createDynamicsCompressor();
    comp.threshold.setValueAtTime(-16, ac.currentTime);
    comp.knee.setValueAtTime(18, ac.currentTime);
    comp.ratio.setValueAtTime(4, ac.currentTime);
    comp.connect(ac.destination);
    return comp;
  }

  /**
   * Plays the 4-layer escalating completion ring based on daily completed task count (n = 1..20+).
   * Layer 1 (n >= 1): Pure sine + subtle triangle fundamental bell.
   * Layer 2 (n >= 4): High-passed 2nd harmonic shimmer.
   * Layer 3 (n >= 8): Warm fifth + octave low-passed chord body.
   * Layer 4 (n >= 12, peaks 20+): Gentle 3-note arpeggio tail + sub-octave bloom.
   */
  function playCompletionRing(completedCount) {
    if (muted) return;
    const ac = ensureCtx();
    if (!ac) return;

    const n = Math.max(1, Math.min(24, Number(completedCount) || 1));
    const now = ac.currentTime;
    const bus = createMasterBus(ac);

    // Map 1..20+ smoothly across the pentatonic scale
    const idx = Math.min(SCALE_HZ.length - 1, Math.floor(((n - 1) / 20) * (SCALE_HZ.length - 1)));
    const f0 = SCALE_HZ[idx];

    // Layer 1: Fundamental Bell (always on)
    const osc1 = ac.createOscillator();
    const g1 = ac.createGain();
    osc1.type = 'sine';
    osc1.frequency.setValueAtTime(f0, now);
    g1.gain.setValueAtTime(0.0001, now);
    g1.gain.linearRampToValueAtTime(0.22, now + 0.008);
    g1.gain.exponentialRampToValueAtTime(0.0001, now + 0.36);
    osc1.connect(g1).connect(bus);
    osc1.start(now);
    osc1.stop(now + 0.38);

    // Layer 2: Harmonic Shimmer (n >= 4)
    if (n >= 4) {
      const osc2 = ac.createOscillator();
      const hp = ac.createBiquadFilter();
      const g2 = ac.createGain();
      osc2.type = 'triangle';
      osc2.frequency.setValueAtTime(f0 * 2.0, now);
      hp.type = 'highpass';
      hp.frequency.setValueAtTime(1600, now);
      const shimmerGain = Math.min(0.14, 0.06 + (n - 4) * 0.006);
      g2.gain.setValueAtTime(0.0001, now);
      g2.gain.linearRampToValueAtTime(shimmerGain, now + 0.012);
      g2.gain.exponentialRampToValueAtTime(0.0001, now + 0.42);
      osc2.connect(hp).connect(g2).connect(bus);
      osc2.start(now);
      osc2.stop(now + 0.44);
    }

    // Layer 3: Warm Chord Body (n >= 8)
    if (n >= 8) {
      [f0 * 1.5, f0 * 2.0].forEach((freq, i) => {
        const osc3 = ac.createOscillator();
        const lp = ac.createBiquadFilter();
        const g3 = ac.createGain();
        osc3.type = 'triangle';
        osc3.frequency.setValueAtTime(freq, now + 0.02);
        lp.type = 'lowpass';
        lp.frequency.setValueAtTime(1400, now + 0.02);
        g3.gain.setValueAtTime(0.0001, now + 0.02);
        g3.gain.linearRampToValueAtTime(i === 0 ? 0.09 : 0.06, now + 0.045);
        g3.gain.exponentialRampToValueAtTime(0.0001, now + 0.52);
        osc3.connect(lp).connect(g3).connect(bus);
        osc3.start(now + 0.02);
        osc3.stop(now + 0.55);
      });
    }

    // Layer 4: Milestone Resonance & Sub-Octave Bloom (n >= 12, peaks at 20+)
    if (n >= 12) {
      const sub = ac.createOscillator();
      const gSub = ac.createGain();
      sub.type = 'sine';
      sub.frequency.setValueAtTime(f0 * 0.5, now);
      gSub.gain.setValueAtTime(0.0001, now);
      gSub.gain.linearRampToValueAtTime(0.11, now + 0.03);
      gSub.gain.exponentialRampToValueAtTime(0.0001, now + 0.65);
      sub.connect(gSub).connect(bus);
      sub.start(now);
      sub.stop(now + 0.68);

      const arpNotes = n >= 20 ? [f0 * 1.25, f0 * 1.5, f0 * 2.0] : [f0 * 1.5, f0 * 2.0];
      arpNotes.forEach((freq, i) => {
        const t = now + 0.075 * (i + 1);
        const oa = ac.createOscillator();
        const ga = ac.createGain();
        oa.type = 'sine';
        oa.frequency.setValueAtTime(freq, t);
        ga.gain.setValueAtTime(0.0001, t);
        ga.gain.linearRampToValueAtTime(0.09, t + 0.01);
        ga.gain.exponentialRampToValueAtTime(0.0001, t + 0.42);
        oa.connect(ga).connect(bus);
        oa.start(t);
        oa.stop(t + 0.45);
      });
    }
  }

  function playPrayerTone() {
    if (muted) return;
    const ac = ensureCtx();
    if (!ac) return;
    const now = ac.currentTime;
    [392.0, 523.25].forEach((freq, idx) => {
      const o = ac.createOscillator();
      const g = ac.createGain();
      o.type = 'sine';
      o.frequency.setValueAtTime(freq, now + idx * 0.09);
      g.gain.setValueAtTime(0.0001, now + idx * 0.09);
      g.gain.linearRampToValueAtTime(0.14, now + idx * 0.09 + 0.015);
      g.gain.exponentialRampToValueAtTime(0.0001, now + idx * 0.09 + 0.45);
      o.connect(g).connect(ac.destination);
      o.start(now + idx * 0.09);
      o.stop(now + idx * 0.09 + 0.48);
    });
  }

  // Procedural Offline Ambient Audio for Zen Focus (Brown / Rain / Wind)
  let ambientNode = null;
  let ambientGain = null;
  let ambientMode = 'silent';

  function stopAmbient() {
    if (ambientNode) {
      try { ambientNode.stop(); } catch (_) {}
      ambientNode.disconnect();
      ambientNode = null;
    }
    ambientMode = 'silent';
  }

  function setAmbient(mode, volumePct = 40) {
    stopAmbient();
    if (!mode || mode === 'silent') return;
    const ac = ensureCtx();
    if (!ac) return;

    const bufferSize = ac.sampleRate * 3;
    const buffer = ac.createBuffer(1, bufferSize, ac.sampleRate);
    const out = buffer.getChannelData(0);
    let lastOut = 0.0;
    for (let i = 0; i < bufferSize; i++) {
      const white = Math.random() * 2 - 1;
      if (mode === 'brown') {
        out[i] = (lastOut + 0.02 * white) / 1.02;
        lastOut = out[i];
        out[i] *= 3.2;
      } else {
        out[i] = white * 0.35;
      }
    }

    const src = ac.createBufferSource();
    src.buffer = buffer;
    src.loop = true;

    const filter = ac.createBiquadFilter();
    if (mode === 'rain') {
      filter.type = 'bandpass';
      filter.frequency.value = 1100;
      filter.Q.value = 0.6;
    } else if (mode === 'wind') {
      filter.type = 'lowpass';
      filter.frequency.value = 420;
    } else {
      filter.type = 'lowpass';
      filter.frequency.value = 260;
    }

    ambientGain = ac.createGain();
    ambientGain.gain.value = Math.max(0.01, Math.min(0.6, (volumePct / 100) * 0.35));

    src.connect(filter).connect(ambientGain).connect(ac.destination);
    src.start();
    ambientNode = src;
    ambientMode = mode;
  }

  function setAmbientVolume(volumePct) {
    if (ambientGain) {
      ambientGain.gain.value = Math.max(0.01, Math.min(0.6, (volumePct / 100) * 0.35));
    }
  }

  window.MasarAudio = {
    playCompletionRing,
    playPrayerTone,
    setAmbient,
    setAmbientVolume,
    stopAmbient,
    getAmbientMode: () => ambientMode,
    setMuted: (m) => { muted = Boolean(m); },
    isMuted: () => muted
  };
})();
