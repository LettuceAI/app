(async () => {
  let ok = false;
  try {
    localStorage.clear();
    sessionStorage.clear();
    ok = localStorage.length === 0 && sessionStorage.length === 0;
  } catch {}
  await window.__TAURI_INTERNALS__.invoke("plugin:event|emit", {
    event: "lettuce-reset-storage",
    payload: { nonce: resetNonce, ok },
  });
})();
