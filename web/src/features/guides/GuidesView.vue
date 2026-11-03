<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { RouterLink } from 'vue-router';

import { GUIDES, guidePath } from '@/shared/catalog/guides';
import { useLocalePath } from '@/shared/locale/localePath';
import { SITE_NAME, usePageMeta } from '@/shared/seo/pageMeta';
import { authorNode, breadcrumbNode, graph } from '@/shared/seo/structuredData';

const { t } = useI18n();
const lp = useLocalePath();

usePageMeta(() => ({
  title: t('guides.pageTitle'),
  description: t('guides.intro'),
  path: lp('/guides'),
  jsonLd: [
    graph(
      authorNode(),
      breadcrumbNode([
        { name: SITE_NAME, path: lp('/') },
        { name: t('guides.title'), path: lp('/guides') },
      ]),
    ),
  ],
}));
</script>

<template>
  <article class="guides">
    <h1>{{ t('guides.title') }}</h1>
    <p class="intro">{{ t('guides.intro') }}</p>
    <ul>
      <li v-for="guide in GUIDES" :key="guide.slug">
        <RouterLink :to="lp(guidePath(guide))">{{ t(`guides.articles.${guide.key}.title`) }}</RouterLink>
        <p>{{ t(`guides.articles.${guide.key}.description`) }}</p>
      </li>
    </ul>
  </article>
</template>

<style scoped>
.guides {
  max-width: 40rem;
  margin: 0 auto;
}

h1 {
  font-size: 1.75rem;
}

.intro {
  color: var(--color-text-muted);
}

ul {
  list-style: none;
  margin: var(--space-6) 0 0;
  padding: 0;
  display: grid;
  gap: var(--space-4);
}

li a {
  display: inline-block;
  padding: var(--space-2) 0;
  font-size: 1.0625rem;
  font-weight: 600;
}

li p {
  margin: var(--space-1) 0 0;
  color: var(--color-text-muted);
}
</style>
