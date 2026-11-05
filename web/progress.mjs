// A single animation callback resolves before paint; wait one full frame.
export function yieldForPaint() {
  return new Promise(resolve => {
    let frame;
    const finish = () => {
      clearTimeout(timeout);
      cancelAnimationFrame(frame);
      resolve();
    };
    // Animation callbacks pause in hidden tabs. Keep background work moving.
    const timeout = setTimeout(finish, 50);
    frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(finish);
    });
  });
}
