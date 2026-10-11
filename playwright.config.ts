import {defineConfig} from '@playwright/test';
export default defineConfig({
  testDir:'./tests/ui',fullyParallel:false,workers:1,
  use:{baseURL:'http://127.0.0.1:1420',channel:'msedge',viewport:{width:1366,height:768},trace:'retain-on-failure'},
  webServer:{command:'npm run dev',url:'http://127.0.0.1:1420',reuseExistingServer:true},
});
