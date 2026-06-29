export function overrideBytesForKey(event, backspace, wordErase) {
  if (event.key !== "Backspace" || event.altKey || event.metaKey) {
    return undefined;
  }

  if (event.type !== "keydown") {
    return new Uint8Array();
  }

  return event.ctrlKey ? wordErase : backspace;
}
