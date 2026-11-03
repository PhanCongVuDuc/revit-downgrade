<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { RouterLink } from 'vue-router';

import { GUIDES, guidePath } from '@/shared/catalog/guides';
import { useLocalePath } from '@/shared/locale/localePath';

const { t } = useI18n();
const lp = useLocalePath();
</script>

<template>
  <nav class="guide-links" aria-labelledby="guide-links-title">
    <h2 id="guide-links-title">{{ t('convert.guideLinks.title') }}</h2>
    <ul>
      <li v-for="guide in GUIDES" :key="guide.slug">
        <RouterLink :to="lp(guidePath(guide))">{{ t(`guides.articles.${guide.key}.title`) }}</RouterLink>
      </li>
    </ul>
  </nav>
</template>

<style scoped>
.guide-links {
  width: min(40rem, 100%);
  margin-top: calc(var(--space-6) * 2);
}

h2 {
  font-size: 1.125rem;
  margin: 0 0 var(--space-2);
}

ul {
  list-style: none;
  margin: 0;
  padding: 0;
}

li a {
  display: inline-block;
  padding: var(--space-2) 0;
  min-height: 2.75rem;
  box-sizing: border-box;
}
</style>
