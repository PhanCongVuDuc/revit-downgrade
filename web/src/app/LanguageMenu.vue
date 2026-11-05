<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, useTemplateRef } from 'vue';
import { useI18n } from 'vue-i18n';
import { RouterLink, useRoute } from 'vue-router';

import { localizedPath } from '@/shared/locale/localePath';

import { type AppLocale, ensureLocale, LOCALES } from './i18n';

/** Each language by the name its own speakers use. */
const LANGUAGE_NAMES: Record<AppLocale, string> = { en: 'English', vi: 'Tiếng Việt', zh: '中文', hi: 'हिन्दी' };

const i18n = useI18n();
const { t, locale } = i18n;
const route = useRoute();
const menu = useTemplateRef<HTMLDetailsElement>('menu');

const current = computed(() => LANGUAGE_NAMES[locale.value as AppLocale]);

/** A page links to its address in each language, so crawlers can follow it too. */
function addressIn(code: string) {
  return { path: localizedPath(route.path, code), query: route.query, hash: route.hash };
}

function close() {
  if (menu.value) {
    menu.value.open = false;
  }
}

async function choose(code: AppLocale) {
  close();
  await ensureLocale(i18n, code);
  locale.value = code;
}

function onPointerDown(event: PointerEvent) {
  if (event.target instanceof Node && !menu.value?.contains(event.target)) {
    close();
  }
}

function onKeyDown(event: KeyboardEvent) {
  if (event.key === 'Escape' && menu.value?.open) {
    close();
    menu.value.querySelector('summary')?.focus();
  }
}

onMounted(() => {
  document.addEventListener('pointerdown', onPointerDown);
  document.addEventListener('keydown', onKeyDown);
});
onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onPointerDown);
  document.removeEventListener('keydown', onKeyDown);
});
</script>

<template>
  <!-- A details element: the links are in the page for crawlers, and it opens without any script. -->
  <details ref="menu" class="language-menu" data-testid="language-menu">
    <summary :aria-label="`${t('app.language')}: ${current}`">
      <svg class="globe" viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="12" cy="12" r="9" />
        <path d="M3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3z" />
      </svg>
      <span>{{ current }}</span>
      <svg class="chevron" viewBox="0 0 24 24" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
    </summary>
    <ul>
      <li v-for="code in LOCALES" :key="code">
        <!-- A job's page has one address for every language: there, the language just changes. -->
        <button
          v-if="route.name === 'job'"
          type="button"
          :lang="code"
          :aria-pressed="locale === code"
          :class="{ 'is-on': locale === code }"
          @click="choose(code)"
        >
          {{ LANGUAGE_NAMES[code] }}
        </button>
        <RouterLink
          v-else
          :to="addressIn(code)"
          :hreflang="code"
          :lang="code"
          :aria-current="locale === code ? 'true' : undefined"
          :class="{ 'is-on': locale === code }"
          @click="close"
        >
          {{ LANGUAGE_NAMES[code] }}
        </RouterLink>
      </li>
    </ul>
  </details>
</template>

<style scoped>
.language-menu {
  position: relative;
}

summary {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  min-height: 2.75rem;
  padding: 0 var(--space-2);
  border-radius: var(--radius);
  color: var(--color-text-muted);
  cursor: pointer;
  list-style: none;
  user-select: none;
}

summary::-webkit-details-marker {
  display: none;
}

summary:hover,
.language-menu[open] summary {
  background: var(--color-surface);
  color: var(--color-text);
}

svg {
  fill: none;
  stroke: currentcolor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 1.75;
}

.globe {
  width: 1.125rem;
  height: 1.125rem;
}

.chevron {
  width: 0.875rem;
  height: 0.875rem;
  transition: transform 0.15s;
}

.language-menu[open] .chevron {
  transform: rotate(180deg);
}

ul {
  position: absolute;
  z-index: 10;
  top: calc(100% + var(--space-1));
  right: 0;
  min-width: 10rem;
  margin: 0;
  padding: var(--space-1);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: var(--radius);
  box-shadow: var(--shadow-card);
  list-style: none;
}

li a,
li button {
  display: flex;
  align-items: center;
  box-sizing: border-box;
  width: 100%;
  min-height: 2.75rem;
  padding: 0 var(--space-2);
  background: none;
  border: 0;
  border-radius: calc(var(--radius) - 2px);
  color: var(--color-text);
  font: inherit;
  text-align: left;
  text-decoration: none;
  cursor: pointer;
}

li a:hover,
li button:hover {
  background: var(--color-primary-soft);
}

.is-on {
  font-weight: 700;
}

.is-on::after {
  margin-left: auto;
  padding-left: var(--space-3);
  color: var(--color-primary);
  content: '✓';
}
</style>
