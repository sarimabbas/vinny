import "./style.css";

const CELL_WIDTH = 192;
const CELL_HEIGHT = 208;
// The atlas leaves six transparent source pixels below Vinny's feet.
// This offset accounts for that padding so the visible feet meet the platform.
const GROUND_OFFSET = 12;

const states = {
  idle: { row: 0, durations: [4200, 100, 90, 90, 100, 130] },
  dance: { row: 0, durations: [120, 100, 90, 90, 100, 180] },
  "running-right": { row: 1, durations: [120, 120, 120, 120, 120, 120, 120, 220] },
  "running-left": { row: 1, flipX: true, durations: [120, 120, 120, 120, 120, 120, 120, 220] },
  waving: { row: 3, durations: [140, 140, 140, 280] },
  jumping: { row: 4, durations: [140, 140, 140, 140, 280] },
  failed: { row: 5, durations: [140, 140, 140, 140, 140, 140, 140, 240] },
  waiting: { row: 6, durations: [150, 150, 150, 150, 150, 260] },
  running: { row: 7, durations: [120, 120, 120, 120, 120, 220] },
  review: { row: 8, durations: [150, 150, 150, 150, 150, 280] },
};

class PetRuntime {
  constructor(canvas, stage) {
    this.canvas = canvas;
    this.stage = stage;
    this.context = canvas.getContext("2d", { alpha: true });
    this.image = new Image();
    this.outlineImage = document.createElement("canvas");
    this.pixelRatio = 1;
    this.state = "idle";
    this.frame = 0;
    this.frameStarted = performance.now();
    this.lastTick = performance.now();
    this.locked = false;
    this.position = { x: 0.5, y: 0.62 };
    this.target = { ...this.position };
    this.pointerHistory = [];
    this.lastPointerAt = performance.now();
    this.lastAutomaticAt = performance.now();
    this.automaticIndex = 0;
    this.antennaHovered = false;
    this.transmission = { active: false, startedAt: 0, duration: 4200 };
    this.nextTransmissionAt = performance.now() + 9000;
    this.reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
    this.mobileLayout = window.matchMedia("(max-width: 760px)");
    this.image.addEventListener("load", () => {
      this.outlineImage.width = this.image.naturalWidth;
      this.outlineImage.height = this.image.naturalHeight;
      const outlineContext = this.outlineImage.getContext("2d");
      outlineContext.drawImage(this.image, 0, 0);
      outlineContext.globalCompositeOperation = "source-in";
      outlineContext.fillStyle = "#171719";
      outlineContext.fillRect(0, 0, this.outlineImage.width, this.outlineImage.height);
      this.stage.classList.add("is-ready");
      this.resize();
      requestAnimationFrame((time) => this.tick(time));
    });
    this.image.src = "/assets/vinny-spritesheet.webp?v=2";
  }

  resize() {
    const bounds = this.stage.getBoundingClientRect();
    const ratio = Math.min(window.devicePixelRatio || 1, 3);
    this.pixelRatio = ratio;
    this.canvas.width = Math.max(1, Math.round(bounds.width * ratio));
    this.canvas.height = Math.max(1, Math.round(bounds.height * ratio));
    this.canvas.style.width = `${bounds.width}px`;
    this.canvas.style.height = `${bounds.height}px`;
    this.context.setTransform(ratio, 0, 0, ratio, 0, 0);
    this.context.imageSmoothingEnabled = true;
    this.context.imageSmoothingQuality = "high";
    this.target = this.clampPosition(this.target, bounds);
    this.position = this.clampPosition(this.position, bounds);
  }

  spriteSize(bounds) {
    const scale = this.mobileLayout.matches ? 0.88 : 0.7;
    const height = Math.min(bounds.height * 0.72, bounds.width * 0.42, 168) * scale;
    return { height, width: height * (CELL_WIDTH / CELL_HEIGHT) };
  }

  groundPosition(bounds) {
    const { height } = this.spriteSize(bounds);
    return (bounds.height - GROUND_OFFSET - height / 2) / bounds.height;
  }

  clampPosition(position, bounds) {
    const size = this.spriteSize(bounds);
    const padding = 0;
    const minX = Math.min(0.5, (size.width / 2 + padding) / bounds.width);
    const maxX = Math.max(0.5, 1 - minX);
    return {
      x: Math.max(minX, Math.min(maxX, position.x)),
      y: this.groundPosition(bounds),
    };
  }

  play(name, once = false) {
    if (!states[name] || this.reducedMotion.matches) return;
    this.state = name;
    this.frame = 0;
    this.frameStarted = performance.now();
    this.locked = once;
  }

  point(event) {
    if (this.reducedMotion.matches) return;
    const bounds = this.stage.getBoundingClientRect();
    const nextTarget = this.clampPosition({
      x: (event.clientX - bounds.left) / bounds.width,
      y: this.groundPosition(bounds),
    }, bounds);
    this.pointerHistory.push({ ...nextTarget, at: performance.now() });
    if (this.pointerHistory.length > 90) this.pointerHistory.shift();
    const overAntenna = this.isOverAntenna(event, bounds);
    if (overAntenna && !this.antennaHovered) this.play("waving", true);
    this.antennaHovered = overAntenna;
    this.lastPointerAt = performance.now();
  }

  scheduleTransmission(time) {
    this.nextTransmissionAt = time + 26000 + Math.random() * 14000;
  }

  startTransmission(time) {
    this.transmission.active = true;
    this.transmission.startedAt = time;
    this.state = "idle";
    this.frame = 0;
    this.frameStarted = time;
    this.locked = true;
    this.pointerHistory.length = 0;
    this.target = { ...this.position };
  }

  endTransmission(time) {
    this.transmission.active = false;
    this.locked = false;
    this.state = "idle";
    this.frame = 0;
    this.frameStarted = time;
    this.scheduleTransmission(time);
  }

  updateTransmission(time) {
    if (this.transmission.active) {
      if (time - this.transmission.startedAt >= this.transmission.duration) {
        this.endTransmission(time);
      }
      return;
    }

    if (
      time >= this.nextTransmissionAt &&
      time - this.lastPointerAt > 3500 &&
      this.state === "idle" &&
      !this.locked
    ) {
      this.startTransmission(time);
    }
  }

  updateDelayedTarget(time) {
    const cutoff = time - 320;
    let delayedTarget;
    while (this.pointerHistory.length && this.pointerHistory[0].at <= cutoff) {
      delayedTarget = this.pointerHistory.shift();
    }
    if (delayedTarget) this.target = { x: delayedTarget.x, y: delayedTarget.y };
  }

  moveTowardTarget(time) {
    const bounds = this.stage.getBoundingClientRect();
    const elapsed = Math.min((time - this.lastTick) / 1000, 0.05);
    this.lastTick = time;
    const dx = (this.target.x - this.position.x) * bounds.width;
    const dy = (this.target.y - this.position.y) * bounds.height;
    const distance = Math.hypot(dx, dy);
    if (distance < 5) return;
    const step = Math.min(distance, 115 * elapsed);
    this.position.x += (dx / distance) * step / bounds.width;
    this.position.y += (dy / distance) * step / bounds.height;
  }

  isOverAntenna(event, bounds) {
    const { height, width } = this.spriteSize(bounds);
    const left = this.position.x * bounds.width - width / 2;
    const top = this.position.y * bounds.height - height / 2;
    const u = (event.clientX - bounds.left - left) / width;
    const v = (event.clientY - bounds.top - top) / height;
    const dx = (u - 0.5) / 0.18;
    const dy = (v - 0.17) / 0.2;
    return dx * dx + dy * dy <= 1;
  }

  advance(time) {
    const definition = states[this.state];
    if (time - this.frameStarted < definition.durations[this.frame]) return;
    this.frameStarted = time;
    this.frame += 1;
    if (this.frame < definition.durations.length) return;
    this.frame = 0;
    if (this.locked) {
      this.locked = false;
      this.state = "idle";
    }
  }

  chooseMovementState(time) {
    if (this.locked || this.reducedMotion.matches) return;
    const bounds = this.stage.getBoundingClientRect();
    const horizontalDistance = (this.target.x - this.position.x) * bounds.width;
    if (Math.abs(horizontalDistance) > 18) {
      const next = horizontalDistance > 0 ? "running-right" : "running-left";
      if (this.state !== next) this.play(next);
    } else if (this.state === "running-right" || this.state === "running-left") {
      this.play("idle");
    }

    if (time - this.lastPointerAt > 5000 && time - this.lastAutomaticAt > 6500) {
      const automatic = ["review", "running", "waiting", "waving"];
      this.play(automatic[this.automaticIndex], true);
      this.automaticIndex = (this.automaticIndex + 1) % automatic.length;
      this.lastAutomaticAt = time;
    }
  }

  drawTransmission(time, destinationWidth, destinationHeight) {
    const elapsed = time - this.transmission.startedAt;
    const duration = this.transmission.duration;
    const appearing = Math.min(1, elapsed / 420);
    const disappearing = Math.min(1, (duration - elapsed) / 620);
    const cover = Math.max(0, Math.min(appearing, disappearing));
    const desktop = Math.max(0, Math.min(1, (elapsed - 620) / 420, (3500 - elapsed) / 420));
    const sx = destinationWidth / CELL_WIDTH;
    const sy = destinationHeight / CELL_HEIGHT;
    const context = this.context;
    const screen = { x: 39, y: 75, width: 114, height: 86, radius: 23 };

    context.save();
    context.scale(sx, sy);
    context.beginPath();
    context.roundRect(screen.x, screen.y, screen.width, screen.height, screen.radius);
    context.clip();
    context.globalAlpha = cover;
    context.fillStyle = "#11171d";
    context.fillRect(screen.x, screen.y, screen.width, screen.height);

    if (desktop > 0) {
      context.save();
      context.globalAlpha = desktop * cover;
      const wallpaper = context.createLinearGradient(screen.x, screen.y, screen.x + screen.width, screen.y + screen.height);
      wallpaper.addColorStop(0, "#2754a5");
      wallpaper.addColorStop(0.52, "#833fd0");
      wallpaper.addColorStop(1, "#f16d73");
      context.fillStyle = wallpaper;
      context.fillRect(screen.x, screen.y, screen.width, screen.height);

      context.fillStyle = "rgba(245,248,255,.76)";
      context.fillRect(screen.x, screen.y, screen.width, 9);
      context.fillStyle = "#ff5c57";
      context.beginPath();
      context.arc(45, 80, 1.2, 0, Math.PI * 2);
      context.fill();
      context.fillStyle = "rgba(20,26,38,.55)";
      context.fillRect(134, 78.5, 12, 2);

      const drift = Math.sin(elapsed / 360) * 1.2;
      context.fillStyle = "rgba(247,249,255,.93)";
      context.beginPath();
      context.roundRect(56 + drift, 93, 80, 52, 4);
      context.fill();
      context.fillStyle = "#e7eaf1";
      context.fillRect(56 + drift, 93, 80, 8);
      for (const [color, cx] of [["#ff5f57", 61], ["#febc2e", 65], ["#28c840", 69]]) {
        context.fillStyle = color;
        context.beginPath();
        context.arc(cx + drift, 97, 1.25, 0, Math.PI * 2);
        context.fill();
      }
      context.fillStyle = "#d9deea";
      context.fillRect(61 + drift, 106, 17, 34);
      context.fillStyle = "#557ad8";
      context.fillRect(82 + drift, 107, 44, 5);
      context.fillStyle = "#d7ddea";
      context.fillRect(82 + drift, 116, 34, 3);
      context.fillRect(82 + drift, 123, 40, 3);
      context.fillRect(82 + drift, 130, 26, 3);

      const cursorX = 92 + Math.sin(elapsed / 520) * 18;
      const cursorY = 119 + Math.cos(elapsed / 430) * 10;
      context.fillStyle = "#ffffff";
      context.strokeStyle = "#111820";
      context.lineWidth = 1;
      context.beginPath();
      context.moveTo(cursorX, cursorY);
      context.lineTo(cursorX, cursorY + 10);
      context.lineTo(cursorX + 3, cursorY + 7);
      context.lineTo(cursorX + 6, cursorY + 7);
      context.closePath();
      context.fill();
      context.stroke();

      context.fillStyle = "#ff5f57";
      context.beginPath();
      context.arc(145, 151, 2, 0, Math.PI * 2);
      context.fill();

      const vignette = context.createRadialGradient(96, 117, 24, 96, 117, 72);
      vignette.addColorStop(0, "rgba(5,8,14,0)");
      vignette.addColorStop(0.72, "rgba(5,8,14,.05)");
      vignette.addColorStop(1, "rgba(5,8,14,.32)");
      context.fillStyle = vignette;
      context.fillRect(screen.x, screen.y, screen.width, screen.height);
      context.restore();
    }

    context.globalAlpha = cover * (desktop > 0 ? 0.13 : 0.34);
    context.strokeStyle = "#b8efff";
    context.lineWidth = 0.75;
    for (let line = 77; line < 161; line += 4) {
      const edgePull = Math.abs(line - 118) / 43;
      const bow = 1.2 + edgePull * 1.5;
      context.beginPath();
      context.moveTo(screen.x - 1, line);
      context.quadraticCurveTo(96, line + bow, screen.x + screen.width + 1, line);
      context.stroke();
    }

    context.globalAlpha = cover * 0.16;
    context.strokeStyle = "#ffffff";
    context.lineWidth = 1;
    context.beginPath();
    context.moveTo(53, 79);
    context.quadraticCurveTo(96, 75.5, 139, 79);
    context.stroke();

    const transition = elapsed < 900 || elapsed > 3350;
    if (transition) {
      const sweep = 79 + ((elapsed * 0.24) % 78);
      context.globalAlpha = cover * 0.7;
      context.strokeStyle = "#65ddff";
      context.lineWidth = 2;
      context.beginPath();
      context.moveTo(screen.x, sweep);
      context.quadraticCurveTo(96, sweep + 2.4, screen.x + screen.width, sweep);
      context.stroke();
      context.fillStyle = "rgba(255,92,87,.7)";
      context.fillRect(49, sweep + 5, 34, 2);
      context.fillStyle = "rgba(105,236,180,.75)";
      context.fillRect(102, sweep - 6, 41, 2);
    }
    context.restore();
  }

  draw(time) {
    const bounds = this.stage.getBoundingClientRect();
    this.context.clearRect(0, 0, bounds.width, bounds.height);
    const { height, width } = this.spriteSize(bounds);
    const x = this.position.x * bounds.width - width / 2;
    const y = this.position.y * bounds.height - height / 2;
    const definition = states[this.state];

    const destinationWidth = Math.round(width * this.pixelRatio) / this.pixelRatio;
    const destinationHeight = Math.round(height * this.pixelRatio) / this.pixelRatio;
    const alignedX = Math.round(x * this.pixelRatio) / this.pixelRatio;
    const alignedY = Math.round(y * this.pixelRatio) / this.pixelRatio;
    const drawFrame = (source, offsetX = 0, offsetY = 0) => {
      this.context.drawImage(
        source,
        this.frame * CELL_WIDTH,
        definition.row * CELL_HEIGHT,
        CELL_WIDTH,
        CELL_HEIGHT,
        offsetX,
        offsetY,
        destinationWidth,
        destinationHeight,
      );
    };

    this.context.save();
    this.context.translate(
      alignedX + (definition.flipX ? destinationWidth : 0),
      alignedY,
    );
    if (definition.flipX) this.context.scale(-1, 1);

    const outlineWidth = 2 / this.pixelRatio;
    for (const [offsetX, offsetY] of [
      [-outlineWidth, 0], [outlineWidth, 0], [0, -outlineWidth], [0, outlineWidth],
      [-outlineWidth, -outlineWidth], [outlineWidth, -outlineWidth],
      [-outlineWidth, outlineWidth], [outlineWidth, outlineWidth],
    ]) drawFrame(this.outlineImage, offsetX, offsetY);

    drawFrame(this.image);
    if (this.transmission.active) {
      this.drawTransmission(time, destinationWidth, destinationHeight);
    }
    this.context.restore();
  }

  tick(time) {
    if (!this.reducedMotion.matches) {
      this.updateDelayedTarget(time);
      this.updateTransmission(time);
      if (this.transmission.active) {
        this.lastTick = time;
      } else {
        this.chooseMovementState(time);
        if (this.state === "running-right" || this.state === "running-left") {
          this.moveTowardTarget(time);
        } else {
          this.lastTick = time;
        }
        this.advance(time);
      }
    } else {
      this.state = "idle";
      this.frame = 0;
    }
    this.draw(time);
    requestAnimationFrame((nextTime) => this.tick(nextTime));
  }
}

const playground = document.querySelector("#playground");
const canvas = document.querySelector("#vinny-canvas");
const pet = new PetRuntime(canvas, playground);
const resizeObserver = new ResizeObserver(() => pet.resize());
resizeObserver.observe(playground);

playground.addEventListener("pointermove", (event) => pet.point(event));
playground.addEventListener("pointerleave", () => { pet.antennaHovered = false; });
playground.addEventListener("pointerup", () => pet.play("dance", true));
playground.addEventListener("dblclick", () => pet.play("failed", true));
playground.addEventListener("keydown", (event) => {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    pet.play("dance", true);
  }
});

window.addEventListener("pagehide", () => resizeObserver.disconnect());

const serverCard = document.querySelector("#server-card");
const serverToggle = document.querySelector("#server-toggle");
const serverStatus = document.querySelector(".server-status");
const applyServer = document.querySelector("#apply-server");
const pipPreview = document.querySelector("#pip-preview");
const pipAddress = document.querySelector("#pip-address");
const listenAddress = document.querySelector("#listen-address");
const listenPort = document.querySelector("#listen-port");
const secureServer = document.querySelector("#secure-server");
const passwordRow = document.querySelector("#password-row");
const passwordInput = document.querySelector("#server-password");
const legacyAuth = document.querySelector("#legacy-auth");
const legacyAuthRow = document.querySelector("#legacy-auth-row");
const listenerWarning = document.querySelector("#listener-warning");
const viewerPolicy = document.querySelector("#viewer-policy");
const viewerHelp = document.querySelector("#viewer-help");
const viewerExplanations = {
  "follow-client": "A viewer can share the session or request exclusive access.",
  "always-shared": "New viewers join without disconnecting anyone.",
  "single-client": "New connections are rejected while a viewer is connected.",
};
const fpsOutput = document.querySelector("#fps-output");
let fps = 20;
let serverTimer;

function updatePreviewAddress() {
  const address = listenAddress.value.trim() || "127.0.0.1";
  const port = listenPort.value || "5900";
  pipAddress.textContent = `${address}:${port}`;
  const legacyEnabled = secureServer.checked && legacyAuth.checked;
  listenerWarning.hidden = (!legacyEnabled && secureServer.checked) || address === "127.0.0.1" || address === "::1";
  listenerWarning.textContent = legacyEnabled
    ? "Legacy VNC connections are password-protected but plaintext. Use only trusted networks or a secure tunnel."
    : "This listener is unauthenticated and plaintext. Use only trusted networks or a secure tunnel.";
}

function updateSecurityFields() {
  if (!secureServer.checked) legacyAuth.checked = false;
  passwordRow.hidden = !secureServer.checked;
  passwordInput.placeholder = legacyAuth.checked ? "1–8 bytes" : "Required";
  legacyAuthRow.hidden = !secureServer.checked;
  updatePreviewAddress();
}

function stopServerPreview() {
  window.clearTimeout(serverTimer);
  serverCard.classList.remove("is-starting", "is-live");
  pipPreview.classList.remove("is-visible");
  pipPreview.setAttribute("aria-hidden", "true");
  serverStatus.textContent = "stopped";
  applyServer.textContent = "Apply & restart";
}

function startServerPreview() {
  window.clearTimeout(serverTimer);
  updatePreviewAddress();
  serverCard.classList.remove("is-live");
  serverCard.classList.add("is-starting");
  pipPreview.classList.remove("is-visible");
  pipPreview.setAttribute("aria-hidden", "true");
  serverStatus.textContent = "starting";
  applyServer.textContent = "Starting…";

  serverTimer = window.setTimeout(() => {
    serverCard.classList.remove("is-starting");
    serverCard.classList.add("is-live");
    pipPreview.classList.add("is-visible");
    pipPreview.setAttribute("aria-hidden", "false");
    serverStatus.textContent = "listening";
    applyServer.textContent = "Apply & restart";
  }, 520);
}

serverToggle.addEventListener("change", () => {
  if (serverToggle.checked) startServerPreview();
  else stopServerPreview();
});

applyServer.addEventListener("click", () => {
  if (!serverToggle.checked) {
    serverToggle.checked = true;
  }
  startServerPreview();
});

document.querySelectorAll(".stepper button").forEach((button) => {
  button.addEventListener("click", () => {
    fps = Math.max(1, Math.min(60, fps + Number(button.dataset.step)));
    fpsOutput.textContent = `${fps} FPS`;
  });
});

viewerPolicy.addEventListener("change", () => {
  viewerHelp.textContent = viewerExplanations[viewerPolicy.value];
});
secureServer.addEventListener("change", updateSecurityFields);
legacyAuth.addEventListener("change", updateSecurityFields);
listenAddress.addEventListener("input", updatePreviewAddress);
listenPort.addEventListener("input", updatePreviewAddress);
serverCard.addEventListener("submit", (event) => event.preventDefault());
serverToggle.checked = false;
stopServerPreview();
updatePreviewAddress();
window.addEventListener("pagehide", () => window.clearTimeout(serverTimer));


const copyInstall = document.querySelector("#copy-install");
const copyStatus = document.querySelector("#copy-status");
copyInstall.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText("brew install --cask sarimabbas/tap/vinny");
    copyStatus.textContent = "Install command copied.";
  } catch {
    copyStatus.textContent = "Select and copy the command above.";
  }
});
