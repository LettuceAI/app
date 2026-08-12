const STORAGE_KEY = "asr.inputDeviceId";

export function getStoredMicDeviceId(): string | null {
  if (typeof localStorage === "undefined") return null;
  const raw = localStorage.getItem(STORAGE_KEY);
  return raw && raw.trim() ? raw : null;
}

export function setStoredMicDeviceId(id: string | null): void {
  if (typeof localStorage === "undefined") return;
  if (!id) {
    localStorage.removeItem(STORAGE_KEY);
  } else {
    localStorage.setItem(STORAGE_KEY, id);
  }
}

export function micConstraintsWithStoredDevice(
  base: MediaTrackConstraints = {},
): MediaTrackConstraints {
  const id = getStoredMicDeviceId();
  return id ? { ...base, deviceId: { exact: id } } : base;
}
