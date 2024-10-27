// console.log("Hello via Bun!");

const puppeteer = require('puppeteer');

(async () => {
    const browser = await puppeteer.connect({
        browserURL: 'http://localhost:9222'
    });
    const page = await browser.newPage();
    await page.goto('https://slashdot.org');
    console.log(await page.title());
    await browser.disconnect();
})();
