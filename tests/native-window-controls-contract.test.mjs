import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

test("Wayland native controls lower only tao's GtkHeaderBar event wrapper", () => {
  const rust = readFileSync(
    new URL("../src-tauri/src/lib.rs", import.meta.url),
    "utf8",
  );

  assert.match(
    rust,
    /fn repair_wayland_native_window_controls\(app: &tauri::App\)/,
  );
  assert.match(
    rust,
    /run_on_main_thread[\s\S]*titlebar\.downcast_ref::<gtk::EventBox>\(\)[\s\S]*header\.is::<gtk::HeaderBar>\(\)/,
  );
  assert.match(
    rust,
    /event_box\.is_above_child\(\)[\s\S]*event_box\.set_above_child\(false\)/,
  );
  assert.match(
    rust,
    /apply_main_window_icon\(app\);[\s\S]*repair_wayland_native_window_controls\(app\);[\s\S]*lock_main_webview_zoom\(app\);/,
  );
});
