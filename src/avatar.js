// Canvas avatar with three states: idle, paused, capture. Each state change
// is a single redraw triggered by an event - there is no animation loop
// running in the background, only a one-shot CSS bounce for "capture".
(function () {
  const canvas = document.getElementById("avatar");
  const ctx = canvas.getContext("2d");
  const SIZE = canvas.width;

  function prefersReducedMotion() {
    return (
      window.matchMedia &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    );
  }

  function drawFace(eyesClosed) {
    ctx.clearRect(0, 0, SIZE, SIZE);

    const cx = SIZE / 2;
    const cy = SIZE / 2;
    const r = SIZE / 2 - 1;

    ctx.fillStyle = "#4a6fe0";
    ctx.beginPath();
    ctx.arc(cx, cy, r, 0, Math.PI * 2);
    ctx.fill();

    ctx.fillStyle = "#ffffff";
    ctx.strokeStyle = "#ffffff";
    ctx.lineWidth = 2;

    const eyeOffsetX = SIZE * 0.16;
    const eyeY = cy - SIZE * 0.05;
    const eyeR = SIZE * 0.09;

    for (const side of [-1, 1]) {
      const ex = cx + side * eyeOffsetX;
      if (eyesClosed) {
        ctx.beginPath();
        ctx.moveTo(ex - eyeR, eyeY);
        ctx.lineTo(ex + eyeR, eyeY);
        ctx.stroke();
      } else {
        ctx.beginPath();
        ctx.arc(ex, eyeY, eyeR, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    ctx.beginPath();
    ctx.arc(cx, cy + SIZE * 0.06, SIZE * 0.22, 0.15 * Math.PI, 0.85 * Math.PI);
    ctx.stroke();
  }

  canvas.addEventListener("animationend", () => {
    canvas.classList.remove("avatar-capture");
  });

  function setState(state) {
    canvas.classList.remove("avatar-capture");

    if (state === "paused") {
      drawFace(true);
      return;
    }

    drawFace(false);

    if (state === "capture" && !prefersReducedMotion()) {
      // Force reflow so re-adding the class restarts the animation even if
      // a previous capture bounce is still settling.
      void canvas.offsetWidth;
      canvas.classList.add("avatar-capture");
    }
  }

  drawFace(false);

  window.RecallPetAvatar = { setState };
})();
