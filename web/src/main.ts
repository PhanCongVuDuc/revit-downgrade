import '@/shared/styles/tokens.css';
import '@/shared/styles/base.css';

import { createRevitDowngradeApp } from '@/app/createApp';

const { app, router } = createRevitDowngradeApp();
// The prerendered page stays on screen until the route's view is loaded, so mounting never shows an empty page.
void router.isReady().then(() => {
  app.mount('#app');
});
