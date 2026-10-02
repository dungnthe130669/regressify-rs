import { describe, expect, test } from "bun:test";
import { formatLogRecord, logRecordSchema, matchesLog } from "./logs";
import {
  artifactPath,
  captureSchema,
  encodeArtifactPath,
  filterCaptures,
  imagePath,
  manifestSchema,
  passRate,
  safePageUrl,
  suiteLabel,
  suiteReportSchema,
} from "./report";

const image = `_snapshot/images/shop/test/home.png?v=${"a".repeat(20)}`;
export const capture = captureSchema.parse({
  id: "home__FINAL.png",
  label: "Home, desktop",
  scenario: "Home",
  selector: "document",
  status: "fail",
  threshold: 0.1,
  mismatch: 1.25,
  error: null,
  reference: image,
  test: image,
  diff: image,
  referenceUrl: "https://example.com/",
  testUrl: null,
  dimensions: { same: false, width: -20, height: 30 },
});

describe("snapshot contract", () => {
  test("capture diagnostic schema is typed and excludes provider detail", () => {
    const record = {
      schemaVersion: 1,
      operationId: "11111111-1111-4111-8111-111111111111",
      operationKind: "capture",
      captureRunId: null,
      execution: null,
      sequence: 0,
      timestampUtcMs: 1790812800000,
      elapsedMs: 10,
      severity: "debug",
      stage: "navigation",
      eventName: "decision",
      message: "<img src=x onerror=alert(1)>",
      fields: { statusCode: { type: "number", value: 200 } },
      spanId: "22222222-2222-4222-8222-222222222222",
      parentSpan: null,
      durationMs: 10,
      outcome: "success",
      errorCause: null,
    };
    const parsed = logRecordSchema.parse(record);
    expect(formatLogRecord(parsed)).toContain("statusCode=200");
    expect(matchesLog(parsed, "error", "all", "")).toBe(false);
    expect(matchesLog(parsed, "debug", "navigation", "STATUSCODE")).toBe(true);
    for (const timestampUtcMs of [-8640000000000001, 8640000000000001, Number.MAX_SAFE_INTEGER]) {
      expect(logRecordSchema.safeParse({ ...record, timestampUtcMs }).success).toBe(false);
    }
    for (const timestampUtcMs of [-8640000000000000, 0, 8640000000000000]) {
      const valid = logRecordSchema.parse({ ...record, timestampUtcMs });
      expect(() => new Date(valid.timestampUtcMs).toISOString()).not.toThrow();
    }
    expect(
      logRecordSchema.safeParse({
        ...record,
        fields: { password: { type: "kind", value: "secret" } },
      }).success,
    ).toBe(false);
    expect(logRecordSchema.safeParse({ ...record, operationKind: "deployment" }).success).toBe(
      false,
    );
    for (const operationKind of ["command", "application"]) {
      expect(logRecordSchema.safeParse({ ...record, operationKind }).success).toBe(false);
    }
    expect(
      logRecordSchema.safeParse({
        ...record,
        errorCause: { category: "provider", causes: [], localDetail: "private" },
      }).success,
    ).toBe(false);
  });
  test("manifest_round_trips_optional_execution_log_fields", () => {
    expect(capture.logs).toBeUndefined();
    const states = ["complete", "partial", "missing", "legacyUnavailable", "loadingError"] as const;
    const parsed = states.map((state) => {
      const log = {
        state,
        operationId: null,
        executionId: null,
        chunks: [],
        records: 0,
        missingRecords: null,
        build: null,
      };
      const value = captureSchema.parse({ ...capture, logs: { reference: null, test: log } });
      expect(value.logs?.test).toEqual(log);
      expect(captureSchema.parse(JSON.parse(JSON.stringify(value)))).toEqual(value);
      return value.logs?.test?.state;
    });
    expect(new Set(parsed).size).toBe(5);
    const log = {
      state: "complete",
      operationId: null,
      executionId: null,
      chunks: ["https://remote.example.test/private.json"],
      records: 0,
      missingRecords: 0,
      build: null,
    };
    expect(
      captureSchema.safeParse({ ...capture, logs: { reference: null, test: log } }).success,
    ).toBe(false);
  });
  test("previews are optional and every preview URL uses the validated image contract", () => {
    expect(capture.previews).toBeUndefined();
    const previews = { reference: image, test: image, diff: null, grid: image };
    expect(captureSchema.parse({ ...capture, previews }).previews).toEqual(previews);
    for (const role of ["reference", "test", "diff", "grid"]) {
      for (const path of [
        "_snapshot/images/thumb.png",
        "https://example.com/thumb.png",
        `../thumb.png?v=${"a".repeat(20)}`,
      ]) {
        expect(
          captureSchema.safeParse({ ...capture, previews: { ...previews, [role]: path } }).success,
        ).toBe(false);
      }
    }
  });
  test("suite display names are optional presentation data, not route identity", () => {
    for (const displayName of [undefined, "", "  ", "Customer checkout", "<img src=x>"]) {
      const suite = suiteReportSchema.parse({ id: "shop", displayName, runId: "run", tests: [] });
      expect(suiteLabel(suite)).toBe(displayName?.trim() ? displayName : "shop");
      expect(imagePath(suite.id, "home.png")).toBe("/suites/shop/images/home.png");
    }
    expect(
      suiteReportSchema.safeParse({ id: "shop", displayName: 10, runId: "run", tests: [] }).success,
    ).toBe(false);
  });
  test("accepts signed dimension differences and hostile labels as plain data", () => {
    expect(
      captureSchema.parse({ ...capture, label: '<img src=x onerror="alert(1)">' }).label,
    ).toContain("<img");
    expect(capture.dimensions?.width).toBe(-20);
  });
  test("rejects unversioned, remote and traversal images", () => {
    for (const path of [
      "../../state.json",
      "https://example.com/image.png",
      "/images/x.png",
      "_snapshot/images/x.png",
      `${image}?x=1`,
      `_snapshot/images/../private.png?v=${"a".repeat(20)}`,
      `_snapshot/images/%2E%2E/private.png?v=${"a".repeat(20)}`,
      `_snapshot/images/shop/%2Fprivate.png?v=${"a".repeat(20)}`,
      `_snapshot/images/shop/CON.png?v=${"a".repeat(20)}`,
      `_snapshot/images/shop/home.png?v=${"a".repeat(19)}`,
    ]) {
      expect(captureSchema.safeParse({ ...capture, test: path }).success).toBe(false);
    }
  });
  test("decodes canonical paths once and requires a version on data and every image role", () => {
    const path = "_snapshot/images/caf\u00e9 & shop/reference/home #1 %2F.png";
    const url = `${encodeArtifactPath(path)}?v=${"A_-0".repeat(5)}`;
    expect(artifactPath(url, "images")).toBe(path);
    expect(artifactPath(url, "data")).toBeUndefined();
    for (const role of ["reference", "test", "diff"]) {
      expect(
        captureSchema.safeParse({ ...capture, [role]: "_snapshot/images/shop/test/home.png" })
          .success,
      ).toBe(false);
    }
    expect(artifactPath(`_snapshot/data/manifest.json?v=${"a".repeat(20)}`, "data")).toBe(
      "_snapshot/data/manifest.json",
    );
    expect(artifactPath("_snapshot/data/manifest.json", "data")).toBeUndefined();
  });
  test("rejects unknown statuses, invalid percentages, duplicate image IDs", () => {
    expect(captureSchema.safeParse({ ...capture, status: "unknown" }).success).toBe(false);
    expect(captureSchema.safeParse({ ...capture, threshold: 101 }).success).toBe(false);
    expect(captureSchema.safeParse({ ...capture, mismatch: Number.NaN }).success).toBe(false);
    expect(
      suiteReportSchema.safeParse({ id: "shop", runId: "run", tests: [capture, capture] }).success,
    ).toBe(false);
  });
  test("validates empty manifests and rejects inconsistent totals and versions", () => {
    const manifest = {
      version: 2,
      title: "",
      heading: "",
      subheading: "",
      generatedAt: "2026-09-23T00:00:00Z",
      suites: [],
    };
    expect(manifestSchema.parse(manifest).suites).toEqual([]);
    expect(manifestSchema.safeParse({ ...manifest, version: 1 }).success).toBe(false);
    const suite = {
      id: "shop",
      total: 2,
      passed: 1,
      failed: 0,
      errors: 0,
      report: `_snapshot/data/suites/shop.json?v=${"a".repeat(20)}`,
    };
    expect(manifestSchema.safeParse({ ...manifest, suites: [suite] }).success).toBe(false);
  });
  test("encodes route segments without interpreting names as routes", () => {
    expect(imagePath("caf\u00e9 & shop", "home #1.png")).toBe(
      "/suites/caf%C3%A9%20%26%20shop/images/home%20%231.png",
    );
  });
  test("allows only uncredentialed http(s) page links", () => {
    for (const url of [
      "javascript:alert(1)",
      "data:text/html,x",
      "file:///etc/passwd",
      "//example.com",
      "https://user:secret@example.com",
      "bad",
      null,
    ]) {
      expect(safePageUrl(url)).toBeUndefined();
    }
    expect(safePageUrl("https://example.com/a?q=b")).toBe("https://example.com/a?q=b");
  });
  test("combines search and status including engine errors and zero totals", () => {
    const tests = [
      capture,
      { ...capture, id: "error", error: "Timeout" },
      { ...capture, id: "pass", status: "pass" as const, label: "Account" },
    ];
    expect(filterCaptures(tests, "home", "fail")).toHaveLength(2);
    expect(filterCaptures(tests, " HOME ", "error")).toHaveLength(1);
    expect(filterCaptures(tests, "account", "pass")).toHaveLength(1);
    expect(filterCaptures(tests, "missing", "all")).toHaveLength(0);
    expect(passRate(0, 0)).toBe("0");
    expect(passRate(2, 3)).toBe("66.7");
  });
});
