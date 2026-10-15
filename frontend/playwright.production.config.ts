import {defineConfig} from '@playwright/test';
export default defineConfig({
  testDir:'./e2e',testMatch:'**/production-pages.production.ts',workers:1,
  timeout:45_000,
  use:{baseURL:'http://127.0.0.1:4174',locale:'zh-CN',trace:'retain-on-failure',
    launchOptions:process.env.SUNA_PLAYWRIGHT_EXECUTABLE?{executablePath:process.env.SUNA_PLAYWRIGHT_EXECUTABLE}:{}},
  webServer:{command:'npm run preview -- --host 127.0.0.1 --port 4174',url:'http://127.0.0.1:4174',reuseExistingServer:!process.env.CI,timeout:60_000},
});
