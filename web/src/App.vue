<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { RouterLink, RouterView } from 'vue-router';

import LanguageMenu from '@/app/LanguageMenu.vue';
import {
  AUTHOR_NAME,
  CONTACT_EMAIL,
  GITHUB_PROFILE_URL,
  LINKEDIN_URL,
  REPO_LABEL,
  REPO_URL,
  THANKS_NAME,
  THANKS_URL,
} from '@/shared/contact/contact';
import { useLocalePath } from '@/shared/locale/localePath';

const { t } = useI18n();
const lp = useLocalePath();
</script>

<template>
  <div class="shell">
    <header class="app-header">
      <RouterLink class="wordmark" :to="lp('/')" :aria-label="t('nav.home')">
        Revit <span class="light">Downgrade</span>
      </RouterLink>
      <nav class="links">
        <RouterLink :to="lp('/guides')">{{ t('nav.guides') }}</RouterLink>
        <RouterLink :to="lp('/revit-version-checker')">{{ t('nav.checker') }}</RouterLink>
        <RouterLink :to="lp('/how-it-works')">{{ t('nav.about') }}</RouterLink>
        <a class="source" :href="REPO_URL" rel="noopener">{{ t('nav.source') }}</a>
      </nav>
      <LanguageMenu class="language" />
    </header>
    <main>
      <RouterView />
    </main>
    <footer class="app-footer">
      <i18n-t keypath="footer.openSource" tag="p" scope="global">
        <template #repo>
          <a :href="REPO_URL" rel="noopener">{{ REPO_LABEL }}</a>
        </template>
      </i18n-t>
      <p class="author">
        {{ t('footer.author') }}: {{ AUTHOR_NAME }}, {{ t('footer.authorAbout') }} ·
        <a :href="`mailto:${CONTACT_EMAIL}`">{{ CONTACT_EMAIL }}</a> ·
        <a :href="LINKEDIN_URL" target="_blank" rel="noopener">LinkedIn</a> ·
        <a :href="GITHUB_PROFILE_URL" target="_blank" rel="noopener">GitHub</a>
      </p>
      <i18n-t keypath="footer.thanks" tag="p" scope="global">
        <template #name>
          <a :href="THANKS_URL" target="_blank" rel="noopener">{{ THANKS_NAME }}</a>
        </template>
      </i18n-t>
      <p>
        <RouterLink :to="lp('/privacy')">{{ t('footer.privacy') }}</RouterLink> ·
        <RouterLink :to="lp('/terms')">{{ t('footer.terms') }}</RouterLink>
      </p>
      <p>{{ t('footer.trademark') }}</p>
    </footer>
  </div>
</template>

<style scoped>
.shell {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: var(--space-2) var(--space-4);
  padding: var(--space-3) var(--space-4);
  font-size: 0.875rem;
}

.wordmark {
  display: inline-flex;
  align-items: center;
  min-height: 2.75rem;
  color: var(--color-text);
  font-size: 1.375rem;
  font-weight: 700;
  text-decoration: none;
}

.light {
  font-weight: 400;
  color: var(--color-text-muted);
}

.links {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2) var(--space-4);
  margin-left: auto;
}

.links a {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 2.75rem;
  min-width: 2.75rem;
  color: var(--color-text-muted);
}

/* On a phone the source link stays in the footer, so the header fits on two lines. */
@media (width < 26rem) {
  .links .source {
    display: none;
  }
}

/* On a phone the language stays beside the name, and the links take the next line. */
@media (width < 48rem) {
  .language {
    order: 1;
  }

  .links {
    order: 2;
    width: 100%;
    margin-left: 0;
  }
}

main {
  flex: 1;
  width: 100%;
  max-width: 48rem;
  margin: 0 auto;
  padding: var(--space-4);
}

.app-footer {
  text-align: center;
  color: var(--color-text-muted);
  font-size: 0.8125rem;
  padding: var(--space-3) var(--space-4) var(--space-4);
}

.app-footer p {
  margin: 0 0 var(--space-2);
  line-height: 1.8;
}

/* Each link gets a full-size touch target; its line grows to fit. */
.app-footer a {
  display: inline-block;
  min-width: 2.75rem;
  min-height: 2.75rem;
  line-height: 2.75rem;
  color: inherit;
}
</style>
