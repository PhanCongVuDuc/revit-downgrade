<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { RouterLink, RouterView, useRoute } from 'vue-router';

import { LOCALES } from '@/app/i18n';
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
import { localizedPath, useLocalePath } from '@/shared/locale/localePath';

const { t, locale } = useI18n();
const lp = useLocalePath();
const route = useRoute();

/** A page links to its address in the other language, so crawlers can follow it too. */
function otherLanguage(code: string) {
  return { path: localizedPath(route.path, code), query: route.query, hash: route.hash };
}
</script>

<template>
  <div class="shell">
    <header class="app-header">
      <RouterLink class="wordmark" :to="lp('/')" :aria-label="t('nav.home')">
        Revit <span class="light">Downgrade</span>
      </RouterLink>
      <nav class="links">
        <RouterLink :to="lp('/guides')">{{ t('nav.guides') }}</RouterLink>
        <RouterLink :to="lp('/how-it-works')">{{ t('nav.about') }}</RouterLink>
        <a class="source" :href="REPO_URL" rel="noopener">{{ t('nav.source') }}</a>
        <span class="languages" role="group" :aria-label="t('app.language')">
          <template v-for="code in LOCALES" :key="code">
            <!-- A job's page has one address for every language: there, the language just changes. -->
            <button
              v-if="route.name === 'job'"
              type="button"
              :aria-pressed="locale === code"
              :class="{ 'is-on': locale === code }"
              @click="locale = code"
            >
              {{ code.toUpperCase() }}
            </button>
            <RouterLink
              v-else
              :to="otherLanguage(code)"
              :hreflang="code"
              :lang="code"
              :aria-current="locale === code ? 'true' : undefined"
              :class="{ 'is-on': locale === code }"
            >
              {{ code.toUpperCase() }}
            </RouterLink>
          </template>
        </span>
      </nav>
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

.languages {
  display: inline-flex;
  border: 1px solid var(--color-border);
}

.languages button,
.languages a {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  text-decoration: none;
  font: inherit;
  font-size: 0.875rem;
  min-width: 2.75rem;
  min-height: 2.75rem;
  padding: var(--space-1) var(--space-2);
  background: transparent;
  color: var(--color-text-muted);
  border: 0;
  cursor: pointer;
}

.languages > * + * {
  border-left: 1px solid var(--color-border);
}

.languages > :first-child {
  border-radius: var(--radius) 0 0 var(--radius);
}

.languages > :last-child {
  border-radius: 0 var(--radius) var(--radius) 0;
}

.languages .is-on {
  background: var(--color-primary);
  color: var(--color-primary-text);
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
