// Iced's browser clipboard is a stub; use the platform API without a cache.
export async function readClipboard() {
  if (!globalThis.navigator?.clipboard?.readText) {
    throw 'Clipboard requires HTTPS or localhost.';
  }
  try {
    const text = await navigator.clipboard.readText();
    if (text.length > 10_000) {
      throw new Error('Text box limit is 10,000 characters.');
    }
    return text;
  } catch (error) {
    throw error?.message ?? String(error);
  }
}

export async function writeClipboard(text) {
  if (!globalThis.navigator?.clipboard?.writeText) {
    throw 'Clipboard requires HTTPS or localhost.';
  }
  try {
    await navigator.clipboard.writeText(text);
  } catch (error) {
    throw error?.message ?? String(error);
  }
}
