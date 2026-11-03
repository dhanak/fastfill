// Use normal browser download handling, without an application modal.
export function downloadFile(name, bytes, mimeType) {
  const link = document.createElement("a");
  const url = URL.createObjectURL(new Blob([bytes], { type: mimeType }));
  try {
    link.href = url;
    link.download = name;
    link.hidden = true;
    document.body.appendChild(link);
    link.click();
  } finally {
    link.remove();
    // Navigation consumes the blob asynchronously; do not revoke it yet.
    setTimeout(() => URL.revokeObjectURL(url), 10_000);
  }
}
