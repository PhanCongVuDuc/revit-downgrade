<script setup lang="ts">
import { useI18n } from 'vue-i18n';
import { RouterLink, useRoute } from 'vue-router';

import { useLocalePath } from '@/shared/locale/localePath';
import { usePageMeta } from '@/shared/seo/pageMeta';

const { t } = useI18n();
const route = useRoute();
const lp = useLocalePath();

usePageMeta(() => ({
  title: t('notFound.pageTitle'),
  description: t('notFound.body'),
  path: route.path,
  noindex: true,
}));
</script>

<template>
  <article class="not-found">
    <h1>{{ t('notFound.title') }}</h1>
    <p>{{ t('notFound.body') }}</p>
    <ul>
      <li>
        <RouterLink :to="lp('/')">{{ t('notFound.home') }}</RouterLink>
      </li>
      <li>
        <RouterLink :to="lp('/guides')">{{ t('notFound.guides') }}</RouterLink>
      </li>
    </ul>
  </article>
</template>

<style scoped>
.not-found {
  max-width: 40rem;
  margin: 0 auto;
}

h1 {
  font-size: 1.75rem;
}

li a {
  display: inline-block;
  padding: var(--space-2) 0;
}
</style>
