import { chromium } from "playwright-core";
import { mkdirSync } from "node:fs";

const shots = "shots";
mkdirSync(shots, { recursive: true });

const browser = await chromium.launch({ channel: "msedge" });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });

page.on("console", (m) => {
  if (m.type() === "error") console.log("[console.error]", m.text().slice(0, 300));
});
page.on("pageerror", (e) => console.log("[pageerror]", String(e).slice(0, 500)));

await page.goto("http://localhost:4173/", { waitUntil: "networkidle" });
await page.waitForTimeout(600);
await page.screenshot({ path: `${shots}/week-light.png` });

await page.goto("http://localhost:4173/#/dark", { waitUntil: "networkidle" });
await page.waitForTimeout(600);
await page.screenshot({ path: `${shots}/week-dark.png` });

await page.goto("http://localhost:4173/#/widget", { waitUntil: "networkidle" });
await page.waitForTimeout(600);
await page.screenshot({ path: `${shots}/widget.png` });

await page.goto("http://localhost:4173/", { waitUntil: "networkidle" });
await page.waitForTimeout(300);
await page.locator("input").fill("Mañana entregar informe de física urgente");
await page.keyboard.press("Enter");
await page.waitForTimeout(400);
await page.screenshot({ path: `${shots}/quickadd-preview.png` });

await browser.close();
console.log("screenshots OK");
