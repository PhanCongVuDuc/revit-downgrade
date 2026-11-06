<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';

import { CONTACT_EMAIL } from '@/shared/contact/contact';
import { useLocalePath } from '@/shared/locale/localePath';
import { SITE_NAME, usePageMeta } from '@/shared/seo/pageMeta';
import { authorNode, breadcrumbNode, graph, webPageNode, websiteNode } from '@/shared/seo/structuredData';

const props = defineProps<{ page: 'privacy' | 'terms' }>();

/** Date the two legal pages last changed in substance, shown on them. */
const UPDATED = '2026-11-06';

const i18n = useI18n();
const { t, locale } = i18n;
const lp = useLocalePath();

const path = computed(() => lp(`/${props.page}`));

usePageMeta(() => ({
  title: t(`legal.${props.page}.pageTitle`),
  description: t(`legal.${props.page}.description`),
  path: path.value,
  jsonLd: [
    graph(
      authorNode(),
      websiteNode(t('convert.description'), locale.value),
      webPageNode({
        type: 'WebPage',
        name: t(`legal.${props.page}.title`),
        description: t(`legal.${props.page}.description`),
        path: path.value,
        inLanguage: locale.value,
      }),
      breadcrumbNode([
        { name: SITE_NAME, path: lp('/') },
        { name: t(`legal.${props.page}.title`), path: path.value },
      ]),
    ),
  ],
}));

interface RawSection {
  heading: string;
  paragraphs: string[];
}

const sections = computed(() => {
  const messages: unknown = i18n.tm(`legal.${props.page}.sections`);
  return (messages as RawSection[]).map((s) => ({
    heading: i18n.rt(s.heading),
    paragraphs: s.paragraphs.map((p) => i18n.rt(p, { email: CONTACT_EMAIL })),
  }));
});

const updated = computed(() =>
  new Intl.DateTimeFormat(locale.value, { timeZone: 'UTC', dateStyle: 'long' }).format(
    new Date(`${UPDATED}T00:00:00Z`),
  ),
);
</script>

<template>
  <article class="legal">
    <h1>{{ t(`legal.${page}.title`) }}</h1>
    <p class="updated">{{ t('legal.updated', { date: updated }) }}</p>
    <section v-for="section in sections" :key="section.heading">
      <h2>{{ section.heading }}</h2>
      <p v-for="paragraph in section.paragraphs" :key="paragraph">{{ paragraph }}</p>
    </section>
  </article>
</template>

<style scoped>
.legal {
  width: min(44rem, 100%);
  margin: 0 auto;
}

.updated {
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

section {
  margin-top: var(--space-5);
}
</style>
