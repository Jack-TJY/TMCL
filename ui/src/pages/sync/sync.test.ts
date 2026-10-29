import { describe, expect, it } from "vitest";
import type { InstanceSummary } from "../../bindings/InstanceSummary";
import {
  filterRows,
  MAX_OPTIONS,
  newRow,
  rowsToOptions,
  summarizePreview,
  toRows,
  usableInstances,
  validateRows,
} from "./sync";

describe("editor rows", () => {
  it("round-trips options and trims keys", () => {
    const rows = toRows([
      { key: "fov", value: "0.5" },
      { key: " key_key.jump ", value: "key.keyboard.space" },
    ]);
    expect(new Set(rows.map((r) => r.id)).size).toBe(2);
    expect(rowsToOptions(rows)).toEqual([
      { key: "fov", value: "0.5" },
      { key: "key_key.jump", value: "key.keyboard.space" },
    ]);
  });

  it("flags invalid rows like the backend", () => {
    const rows = [
      newRow("", "1"),
      newRow("a:b", "1"),
      newRow("lang", "en_us\nx"),
      newRow("fov", "1"),
      newRow("fov ", "2"),
      newRow("gamma", ""),
    ];
    expect(validateRows(rows)).toEqual(["empty", "colon", "newline", null, "duplicate", null]);
    expect(rowsToOptions(rows)).toBeNull();
  });

  it("rejects too many options", () => {
    const rows = Array.from({ length: MAX_OPTIONS + 1 }, (_, i) => newRow(`k${i}`, "v"));
    expect(rowsToOptions(rows)).toBeNull();
    expect(rowsToOptions(rows.slice(1))).toHaveLength(MAX_OPTIONS);
  });

  it("filters by key or value", () => {
    const rows = toRows([
      { key: "key_key.forward", value: "key.keyboard.w" },
      { key: "fov", value: "0.5" },
    ]);
    expect(filterRows(rows, " KEYBOARD ").map((r) => r.key)).toEqual(["key_key.forward"]);
    expect(filterRows(rows, "")).toHaveLength(2);
    expect(filterRows(rows, "视场", (k) => (k === "fov" ? "视场角" : null))).toHaveLength(1);
  });
});

describe("instances and preview", () => {
  it("excludes broken instances", () => {
    const base = {
      game_version: "1.20.1",
      loader: null,
      group: null,
      created_at: "",
      last_played_at: null,
    };
    const list: InstanceSummary[] = [
      { ...base, id: "a", name: "A", error: null },
      { ...base, id: "b", name: "B", error: "INSTANCE_CORRUPTED" },
    ];
    expect(usableInstances(list).map((i) => i.id)).toEqual(["a"]);
  });

  it("summarizes changes", () => {
    const change = { key: "fov", category: "video" as const, old: "0", new: "1" };
    expect(
      summarizePreview({
        targets: [
          {
            instance_id: "a",
            game_version: "1.20.1",
            changes: [change, change],
            has_backup: false,
          },
          { instance_id: "b", game_version: "1.12.2", changes: [], has_backup: true },
          { instance_id: "c", game_version: "1.8.9", changes: [change], has_backup: false },
        ],
      }),
    ).toEqual({ changes: 3, targets: 2 });
  });
});
