import test from "node:test";
import assert from "node:assert/strict";

import { overrideBytesForKey } from "../src/input.js";

test("plain Backspace maps to DEL exactly once", () => {
  assert.deepEqual(
    overrideBytesForKey(
      { type: "keydown", key: "Backspace", ctrlKey: false, altKey: false, metaKey: false },
      new Uint8Array([0x17])
    ),
    new Uint8Array([0x7f])
  );
});

test("Ctrl+Backspace maps to configured word erase exactly once", () => {
  assert.deepEqual(
    overrideBytesForKey(
      { type: "keydown", key: "Backspace", ctrlKey: true, altKey: false, metaKey: false },
      new Uint8Array([0x1b, 0x7f])
    ),
    new Uint8Array([0x1b, 0x7f])
  );
});

test("keyup is consumed without sending another erase sequence", () => {
  assert.deepEqual(
    overrideBytesForKey(
      { type: "keyup", key: "Backspace", ctrlKey: true, altKey: false, metaKey: false },
      new Uint8Array([0x17])
    ),
    new Uint8Array()
  );
});

test("modified and unrelated keys remain under xterm control", () => {
  assert.equal(
    overrideBytesForKey(
      { type: "keydown", key: "Backspace", ctrlKey: false, altKey: true, metaKey: false },
      new Uint8Array([0x17])
    ),
    undefined
  );
  assert.equal(
    overrideBytesForKey(
      { type: "keydown", key: "Enter", ctrlKey: false, altKey: false, metaKey: false },
      new Uint8Array([0x17])
    ),
    undefined
  );
});
