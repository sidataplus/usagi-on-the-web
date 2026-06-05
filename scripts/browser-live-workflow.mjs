#!/usr/bin/env node

import { pathToFileURL } from "node:url";

export const LIVE_BROWSER_WORKFLOW_TERMS = Object.freeze({
  drugSuccess: "tramadol hydrochloride 50 mg capsule",
  drugExpectedFailure: "query with no precomputed embedding",
  hybridSuccess: "tramadol 50 mg capsule",
});

const DEFAULTS = Object.freeze({
  baseUrl: "http://127.0.0.1:3220",
  email: "demo@usagi.test",
  password: "password123",
  pollAttempts: 20,
  pollIntervalMs: 1500,
  captureScreenshots: false,
});

export async function runLiveBrowserWorkflow(options = {}) {
  const config = { ...DEFAULTS, ...options };
  const tab = config.tab;
  if (!tab) throw new Error("runLiveBrowserWorkflow requires a Browser tab handle");

  const evidence = {
    startedAt: new Date().toISOString(),
    baseUrl: config.baseUrl,
    terms: LIVE_BROWSER_WORKFLOW_TERMS,
    projects: {},
    steps: [],
  };

  await step(evidence, "sign_in", async () => {
    await ensureSignedIn(tab, config);
    return snapshotCheck(tab, [
      "Dashboard",
      "Projects",
    ]);
  });

  const drugProject = await step(evidence, "drug_project", async () => {
    return createProject(tab, config, {
      name: `Browser Drug Workflow ${Date.now()}`,
      description: "Browser-rendered live E2E drug workflow",
      domain: "Drug",
      sourceVocabulary: "NDC",
      targetVocabularies: "RxNorm",
      vocabularyVersion: "browser-live",
    });
  });
  evidence.projects.drug = drugProject;

  await step(evidence, "drug_import", async () => {
    return importRows(tab, config, drugProject.id, [
      "source_code,source_name,source_frequency",
      `SRC_TRAMADOL_50_CAP,${LIVE_BROWSER_WORKFLOW_TERMS.drugSuccess},12`,
      `SRC_MISSING,${LIVE_BROWSER_WORKFLOW_TERMS.drugExpectedFailure},4`,
    ].join("\n"), [
      "Import confirmed. Source terms are ready for review.",
      "2 source terms became",
    ]);
  });

  await step(evidence, "manual_search", async () => {
    await openMappingBySourceName(tab, config, drugProject.id, LIVE_BROWSER_WORKFLOW_TERMS.drugSuccess);
    await clickUnique(tab, tab.playwright.getByRole("button", { name: "Search", exact: true }), "Search");
    await settle(tab);
    return snapshotCheck(tab, [
      "2 saved candidates",
      "Tramadol Hydrochloride 50 MG Oral Capsule",
      "#40162522",
    ]);
  });

  await step(evidence, "drug_auto_map_partial_failure", async () => {
    const jobUrl = await triggerSuggestionsAndOpenJob(tab, config, drugProject.id);
    const snapshot = await pollSnapshot(tab, config, [
      "Succeeded with errors",
      "Partial failures",
      "EMBEDDING_FAILED",
      "SRC_MISSING",
      "Retry",
    ]);
    return {
      url: jobUrl,
      checks: includesAll(snapshot, [
        "Succeeded with errors",
        "Partial failures",
        "EMBEDDING_FAILED",
        "SRC_MISSING",
        "Retry",
      ]),
      screenshot: await maybeScreenshot(tab, config, "drug-partial-failure"),
    };
  });

  await step(evidence, "drug_review_after_mapper", async () => {
    await goto(tab, config, `/projects/${drugProject.id}/mappings`);
    return snapshotCheck(tab, [
      "1 ready with candidates",
      "1 waiting on suggestions",
      "Succeeded with errors",
      "Tramadol Hydrochloride 50 MG Oral Capsule",
    ]);
  });

  const mixedProject = await step(evidence, "mixed_project", async () => {
    return createProject(tab, config, {
      name: `Browser Mixed Workflow ${Date.now()}`,
      description: "Browser-rendered live E2E hybrid workflow",
      domain: "Mixed",
      sourceVocabulary: "SOURCE",
      targetVocabularies: "RxNorm",
      vocabularyVersion: "browser-live",
    });
  });
  evidence.projects.mixed = mixedProject;

  await step(evidence, "mixed_import", async () => {
    return importRows(tab, config, mixedProject.id, [
      "source_code,source_name,source_frequency,source_domain_hint",
      `MIX_TRAMADOL_OK,${LIVE_BROWSER_WORKFLOW_TERMS.hybridSuccess},8,Drug`,
    ].join("\n"), [
      "Import confirmed. Source terms are ready for review.",
      "1 source term became",
    ]);
  });

  await step(evidence, "mixed_hybrid_success", async () => {
    const jobUrl = await triggerSuggestionsAndOpenJob(tab, config, mixedProject.id);
    const snapshot = await pollSnapshot(tab, config, [
      "Succeeded",
      "1/1",
      "Failed rows",
      "0",
    ]);
    return {
      url: jobUrl,
      checks: includesAll(snapshot, [
        "Succeeded",
        "1/1",
        "Failed rows",
      ]),
      screenshot: await maybeScreenshot(tab, config, "mixed-hybrid-job"),
    };
  });

  await step(evidence, "mixed_review_candidates", async () => {
    await goto(tab, config, `/projects/${mixedProject.id}/mappings`);
    const snapshot = await pollSnapshot(tab, config, [
      "1 ready with candidates",
      "Tramadol Hydrochloride 50 MG Oral Capsule",
    ]);
    return {
      checks: includesAll(snapshot, [
        "1 ready with candidates",
        "0 waiting on suggestions",
        "Tramadol Hydrochloride 50 MG Oral Capsule",
      ]),
      screenshot: await maybeScreenshot(tab, config, "mixed-review-candidates"),
    };
  });

  evidence.finishedAt = new Date().toISOString();
  evidence.ok = evidence.steps.every((item) => item.ok);
  return evidence;
}

async function step(evidence, name, callback) {
  const startedAt = new Date().toISOString();
  try {
    const result = await callback();
    evidence.steps.push({ name, startedAt, finishedAt: new Date().toISOString(), ok: true, result });
    return result;
  } catch (error) {
    evidence.steps.push({
      name,
      startedAt,
      finishedAt: new Date().toISOString(),
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
    throw error;
  }
}

async function ensureSignedIn(tab, config) {
  await goto(tab, config, "/dashboard");
  let snapshot = await tab.playwright.domSnapshot();
  if (!snapshot.includes("Sign in")) return;

  await fillUnique(tab, tab.playwright.getByLabel("Email", { exact: true }), config.email, "Email");
  await fillUnique(tab, tab.playwright.getByLabel("Password", { exact: true }), config.password, "Password");
  await clickUnique(tab, tab.playwright.getByRole("button", { name: "Sign in", exact: true }), "Sign in");
  await settle(tab);
  snapshot = await tab.playwright.domSnapshot();
  assertIncludes(snapshot, "Dashboard", "signed-in dashboard");
}

async function createProject(tab, config, project) {
  await goto(tab, config, "/projects/new");
  await fillUnique(tab, tab.playwright.getByLabel("Name", { exact: true }), project.name, "Name");
  await fillUnique(tab, tab.playwright.getByLabel("Description", { exact: true }), project.description, "Description");
  await selectUnique(tab, tab.playwright.getByLabel("Mapping domain", { exact: true }), project.domain, "Mapping domain");
  await fillUnique(tab, tab.playwright.getByLabel("Source vocabulary", { exact: true }), project.sourceVocabulary, "Source vocabulary");
  await fillUnique(tab, tab.playwright.getByLabel("Target vocabularies", { exact: true }), project.targetVocabularies, "Target vocabularies");
  await fillUnique(tab, tab.playwright.getByLabel("Vocabulary version", { exact: true }), project.vocabularyVersion, "Vocabulary version");
  await clickUnique(tab, tab.playwright.getByRole("button", { name: "Create project", exact: true }), "Create project");
  await settle(tab);

  const url = await tab.url();
  const id = parseProjectId(url);
  const snapshot = await tab.playwright.domSnapshot();
  assertIncludes(snapshot, project.name, "created project page");
  assertIncludes(snapshot, "Import source terms", "project next action");
  return { id, name: project.name, url };
}

async function importRows(tab, config, projectId, rowsText, expectedTexts) {
  await goto(tab, config, `/projects/${projectId}/imports/new`);
  await clickUnique(tab, tab.playwright.getByText("Or paste rows directly", { exact: true }), "Or paste rows directly");
  await settle(tab, 300);
  await fillUnique(tab, tab.playwright.getByLabel("CSV rows", { exact: true }), rowsText, "CSV rows");
  await clickUnique(tab, tab.playwright.getByRole("button", { name: "Preview import", exact: true }), "Preview import");
  await settle(tab);

  let snapshot = await tab.playwright.domSnapshot();
  assertIncludes(snapshot, "Import preview", "import preview");
  assertIncludes(snapshot, "pasted-source-terms.csv", "pasted import filename");
  await clickUnique(tab, tab.playwright.getByRole("button", { name: "Confirm import", exact: true }), "Confirm import");
  await settle(tab);

  snapshot = await tab.playwright.domSnapshot();
  for (const text of expectedTexts) assertIncludes(snapshot, text, "confirmed import");
  return { url: await tab.url(), checks: includesAll(snapshot, expectedTexts) };
}

async function openMappingBySourceName(tab, config, projectId, sourceName) {
  await goto(tab, config, `/projects/${projectId}/mappings?status=unchecked`);
  const href = await tab.playwright.evaluate((name) => {
    const link = Array.from(document.querySelectorAll("a")).find((candidate) => candidate.textContent.trim() === name);
    return link?.getAttribute("href") || null;
  }, sourceName, { timeoutMs: 5000 });
  if (!href) throw new Error(`Could not find mapping link for ${sourceName}`);
  await tab.goto(new URL(href, config.baseUrl).toString());
  await settle(tab);
  const snapshot = await tab.playwright.domSnapshot();
  assertIncludes(snapshot, sourceName, "mapping detail");
}

async function triggerSuggestionsAndOpenJob(tab, config, projectId) {
  await goto(tab, config, `/projects/${projectId}/mappings`);
  await clickUnique(tab, tab.playwright.getByRole("button", { name: "Suggest candidates", exact: true }), "Suggest candidates");
  await settle(tab);
  const snapshot = await tab.playwright.domSnapshot();
  assertIncludes(snapshot, "Auto-suggest started.", "suggestion flash");
  const href = await tab.playwright.evaluate(() => {
    const link = Array.from(document.querySelectorAll("a")).find((candidate) => candidate.textContent.trim() === "Watch progress");
    return link?.getAttribute("href") || null;
  }, null, { timeoutMs: 5000 });
  if (!href) throw new Error("Could not find Watch progress link");
  const jobUrl = new URL(href, config.baseUrl).toString();
  await tab.goto(jobUrl);
  await settle(tab);
  return jobUrl;
}

async function pollSnapshot(tab, config, requiredTexts) {
  let snapshot = "";
  for (let index = 0; index < config.pollAttempts; index += 1) {
    snapshot = await tab.playwright.domSnapshot();
    if (requiredTexts.every((text) => snapshot.includes(text))) return snapshot;
    await tab.reload();
    await settle(tab, config.pollIntervalMs);
  }
  for (const text of requiredTexts) assertIncludes(snapshot, text, "polled page");
  return snapshot;
}

async function snapshotCheck(tab, expectedTexts) {
  const snapshot = await tab.playwright.domSnapshot();
  for (const text of expectedTexts) assertIncludes(snapshot, text, "page snapshot");
  return { url: await tab.url(), checks: includesAll(snapshot, expectedTexts) };
}

async function goto(tab, config, path) {
  const nextUrl = new URL(path, config.baseUrl).toString();
  if ((await tab.url()) !== nextUrl) await tab.goto(nextUrl);
  await settle(tab);
}

async function fillUnique(tab, locator, value, label) {
  await requireUnique(locator, label);
  try {
    await locator.fill(value, { timeoutMs: 5000 });
  } catch (error) {
    await fillWithTypingFallback(tab, locator, value, error);
  }
  await settle(tab, 100);
}

async function fillWithTypingFallback(tab, locator, value, originalError) {
  await locator.click({ timeoutMs: 5000 }).catch(() => {});
  await locator.press("ControlOrMeta+A", { timeoutMs: 5000 }).catch(() => {});
  await locator.press("Backspace", { timeoutMs: 5000 }).catch(() => {});
  try {
    await locator.type(value, { timeoutMs: 10000 });
  } catch (fallbackError) {
    try {
      await typeByKeyPress(locator, value);
    } catch (keypressError) {
      const originalMessage = originalError instanceof Error ? originalError.message : String(originalError);
      const fallbackMessage = fallbackError instanceof Error ? fallbackError.message : String(fallbackError);
      const keypressMessage = keypressError instanceof Error ? keypressError.message : String(keypressError);
      throw new Error(
        `Could not fill field. fill failed with ${originalMessage}; locator typing failed with ${fallbackMessage}; keypress typing failed with ${keypressMessage}`
      );
    }
  }
  await settle(tab, 100);
}

async function typeByKeyPress(locator, value) {
  for (const character of value) {
    await locator.press(keyForCharacter(character), { timeoutMs: 5000 });
  }
}

function keyForCharacter(character) {
  if (character === " ") return "Space";
  if (character === "\n") return "Enter";
  if (character === "\t") return "Tab";
  return character;
}

async function selectUnique(tab, locator, value, label) {
  await requireUnique(locator, label);
  await locator.selectOption(value, { timeoutMs: 5000 });
  await settle(tab, 100);
}

async function clickUnique(tab, locator, label) {
  await requireUnique(locator, label);
  try {
    await locator.click({ timeoutMs: 5000 });
  } catch (error) {
    const clicked = await clickByTextCenter(tab, label);
    if (!clicked) {
      const message = error instanceof Error ? error.message : String(error);
      throw new Error(`Could not click ${label}: ${message}`);
    }
  }
  await settle(tab);
}

async function clickByTextCenter(tab, label) {
  const rect = await tab.playwright.evaluate((text) => {
    const targets = Array.from(document.querySelectorAll("button, a, summary, input[type='submit']"));
    const element = targets.find((candidate) => {
      const visible = candidate.getClientRects().length > 0;
      const labelText = candidate.textContent?.trim() || candidate.value || candidate.getAttribute("aria-label") || "";
      return visible && labelText === text;
    });
    if (!element) return null;

    const bounds = element.getBoundingClientRect();
    return {
      x: bounds.left + bounds.width / 2,
      y: bounds.top + bounds.height / 2,
    };
  }, label, { timeoutMs: 5000 });

  if (!rect) return false;
  await tab.cua.click({ x: rect.x, y: rect.y });
  return true;
}

async function requireUnique(locator, label) {
  const count = await locator.count();
  if (count !== 1) throw new Error(`Expected one ${label}, found ${count}`);
}

async function settle(tab, timeoutMs = 700) {
  await tab.playwright.waitForLoadState({ state: "load", timeoutMs: 10000 }).catch(() => {});
  await tab.playwright.waitForTimeout(timeoutMs);
}

function parseProjectId(url) {
  const match = url.match(/\/projects\/(proj_[^/?#]+)/);
  if (!match) throw new Error(`Could not parse project id from ${url}`);
  return match[1];
}

function assertIncludes(snapshot, text, label) {
  if (!snapshot.includes(text)) throw new Error(`Expected ${label} to include ${JSON.stringify(text)}`);
}

function includesAll(snapshot, texts) {
  return Object.fromEntries(texts.map((text) => [text, snapshot.includes(text)]));
}

async function maybeScreenshot(tab, config, label) {
  if (!config.captureScreenshots) return { attempted: false };
  try {
    const bytes = await tab.screenshot({ fullPage: false });
    return { attempted: true, label, bytes: bytes.length };
  } catch (error) {
    return {
      attempted: true,
      label,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

function printSelfCheck() {
  const payload = {
    ok: true,
    exports: ["LIVE_BROWSER_WORKFLOW_TERMS", "runLiveBrowserWorkflow"],
    terms: LIVE_BROWSER_WORKFLOW_TERMS,
  };
  console.log(JSON.stringify(payload, null, 2));
}

const runtimeProcess = globalThis.process;
if (runtimeProcess?.argv?.[1] && import.meta.url === pathToFileURL(runtimeProcess.argv[1]).href) {
  if (runtimeProcess.argv.includes("--self-check")) {
    printSelfCheck();
  } else {
    console.log("Import this module from the in-app Browser runtime and call runLiveBrowserWorkflow({ browser, tab }).");
    console.log("Run node scripts/browser-live-workflow.mjs --self-check to inspect its static contract.");
  }
}
