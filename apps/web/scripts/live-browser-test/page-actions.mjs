import assert from "node:assert/strict";
import { BrowserPage, waitUntil } from "../browser-test.mjs";
import { chrome, origin, result } from "./context.mjs";
import { assertNoAlert } from "./support.mjs";

export async function assertPageIdentity(page) {
  assert.equal(await page.evaluate("location.origin"), origin);
  assert.equal(await page.evaluate("document.title"), "雾港调查台");
  await page.waitForText("进入调查台");
  assert.ok((await page.text()).length > 100);
  assert.equal(await page.hasText("Internal Server Error"), false);
  assert.equal(page.errors.length, 0, `browser console errors: ${page.errors.join(" | ")}`);
  result.checks.push("real product shell loaded without overlay or console errors");
}

export async function login(page, account) {
  await page.submit('form[data-form="login"]', {
    login: account.login,
    password: account.password,
  });
  await page.waitForText("选择一场调查", 20_000);
  await assertNoAlert(page);
}

export async function openCampaign(page, campaignId, expectedText) {
  await clickAndWait(
    page,
    `[data-action="open-campaign"][data-campaign-id="${campaignId}"]`,
    "战役已打开",
    20_000,
  );
  await page.waitForText(expectedText, 20_000);
}

export async function issueInvite(page, userId, role) {
  const previous = await page.evaluate(
    "Array.from(document.querySelectorAll('.secret-output code')).map((node) => node.innerText).join('\\n')",
  );
  await submitAndWait(page, 'form[data-form="issue-invite"]', { userId, role }, "邀请已签发", 20_000);
  await waitUntil(async () => {
    const current = await page.evaluate(
      "Array.from(document.querySelectorAll('.secret-output code')).map((node) => node.innerText).join('\\n')",
    );
    return current && current !== previous;
  }, 20_000, "invite token did not render");
  const [inviteId, rawToken] = await page.evaluate(
    "Array.from(document.querySelectorAll('.secret-output code')).map((node) => node.innerText)",
  );
  assert.ok(inviteId && rawToken);
  return { inviteId, rawToken };
}

export async function joinCampaign(account, campaignId, invite) {
  const page = await loginPage(account);
  await submitAndWait(page, 'form[data-form="accept-invite"]', {
    campaignId,
    inviteId: invite.inviteId,
    rawToken: invite.rawToken,
  }, "已加入战役", 20_000);
  return page;
}

export async function loginPage(account) {
  const page = await BrowserPage.open(chrome.debugOrigin, origin);
  await login(page, account);
  return page;
}

export async function submitAndWait(page, selector, values, expectedNotice, timeoutMs = 12_000) {
  const responsesBefore = page.responseBodies.length;
  await page.submit(selector, values);
  await waitUntil(async () => {
    const notice = await page.evaluate("document.querySelector('.feedback.notice')?.innerText || ''");
    const alert = await page.evaluate("document.querySelector('.feedback.error')?.innerText || ''");
    if (alert) throw new Error(alert);
    return page.responseBodies.length > responsesBefore && notice.includes(expectedNotice);
  }, timeoutMs, `operation did not complete: ${expectedNotice}`);
  await assertNoAlert(page);
}

export async function clickAndWait(page, selector, expectedNotice, timeoutMs = 12_000) {
  const responsesBefore = page.responseBodies.length;
  await page.click(selector);
  await waitUntil(async () => {
    const notice = await page.evaluate("document.querySelector('.feedback.notice')?.innerText || ''");
    const alert = await page.evaluate("document.querySelector('.feedback.error')?.innerText || ''");
    if (alert) throw new Error(alert);
    return page.responseBodies.length > responsesBefore && notice.includes(expectedNotice);
  }, timeoutMs, `operation did not complete: ${expectedNotice}`);
  await assertNoAlert(page);
}
