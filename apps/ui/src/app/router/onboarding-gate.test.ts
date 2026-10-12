import { describe, expect, it } from "vitest";
import type { LegacyState } from "@/api/generated/bindings";
import { legacyMigrationOffered, onboardingRedirect, welcomeBackPath, welcomePath } from "./onboarding-gate";

const ready: LegacyState = { type: "ready", schema_version: 96 };

describe("legacyMigrationOffered", () => {
  it("offers the move for a ready or outdated legacy database", () => {
    expect(legacyMigrationOffered({ legacy_state: ready, ui_state: {} })).toBe(true);
    expect(
      legacyMigrationOffered({
        legacy_state: { type: "requires_legacy_upgrade", reason: { type: "bin_storage" }, required_app_version: "2.2.5" },
        ui_state: {},
      }),
    ).toBe(true);
  });

  it("does not offer it once declined or moved, or without legacy data", () => {
    expect(legacyMigrationOffered({ legacy_state: ready, ui_state: { legacyMigration: "declined" } })).toBe(false);
    expect(legacyMigrationOffered({ legacy_state: ready, ui_state: { legacyMigration: "migrated" } })).toBe(false);
    expect(legacyMigrationOffered({ legacy_state: { type: "absent" }, ui_state: {} })).toBe(false);
  });
});

describe("onboardingRedirect", () => {
  it("sends a new install to the welcome flow and a legacy install to welcome back", () => {
    expect(onboardingRedirect({ completed: false, legacy: false }, "/")).toBe(welcomePath);
    expect(onboardingRedirect({ completed: false, legacy: true }, "/")).toBe(welcomeBackPath);
    expect(onboardingRedirect({ completed: false, legacy: true }, welcomePath)).toBe(welcomeBackPath);
    expect(onboardingRedirect({ completed: false, legacy: false }, welcomeBackPath)).toBe(welcomePath);
  });

  it("keeps welcome back open after the move brings a completed onboarding along", () => {
    expect(onboardingRedirect({ completed: true, legacy: true }, welcomeBackPath)).toBeNull();
    expect(onboardingRedirect({ completed: true, legacy: false }, welcomeBackPath)).toBeNull();
  });

  it("leaves a finished install in the app", () => {
    expect(onboardingRedirect({ completed: true, legacy: true }, "/")).toBeNull();
    expect(onboardingRedirect({ completed: true, legacy: false }, welcomePath)).toBe("/");
  });
});
