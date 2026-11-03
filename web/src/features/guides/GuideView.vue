<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { RouterLink, useRoute } from 'vue-router';

import { GUIDES, guideBySlug, guidePath } from '@/shared/catalog/guides';
import { PAIRS, pairPath } from '@/shared/catalog/pairs';
import { AUTHOR_NAME, LINKEDIN_URL } from '@/shared/contact/contact';
import { useLocalePath } from '@/shared/locale/localePath';
import { SITE_NAME, usePageMeta } from '@/shared/seo/pageMeta';
import { articleNode, authorNode, breadcrumbNode, graph } from '@/shared/seo/structuredData';

/** One section of an article as the locale files hold it: a heading, then paragraphs, a list, steps or a table. */
interface RawSection {
  heading: string;
  paragraphs?: string[];
  list?: string[];
  ordered?: string[];
  table?: { head: string[]; rows: string[][] };
}

const i18n = useI18n();
const { t, locale } = i18n;
const route = useRoute();
const lp = useLocalePath();

// The router only opens this view for a known slug.
const guide = computed(() => guideBySlug(String(route.params.slug)) ?? GUIDES[0]);
const others = computed(() => GUIDES.filter((g) => g.slug !== guide.value.slug));
// One step down from each version: the conversions most people come for.
const pairs = PAIRS.filter((p) => p.from - p.to === 1);

const sections = computed(() => {
  const messages: unknown = i18n.tm(`guides.articles.${guide.value.key}.sections`);
  const raw = messages as RawSection[];
  const text = (s: string) => i18n.rt(s);
  return raw.map((s) => ({
    heading: text(s.heading),
    paragraphs: (s.paragraphs ?? []).map(text),
    list: (s.list ?? []).map(text),
    ordered: (s.ordered ?? []).map(text),
    table: s.table ? { head: s.table.head.map(text), rows: s.table.rows.map((row) => row.map(text)) } : null,
  }));
});

function formatDate(iso: string): string {
  return new Intl.DateTimeFormat(locale.value, { timeZone: 'UTC', dateStyle: 'long' }).format(
    new Date(`${iso}T00:00:00Z`),
  );
}

usePageMeta(() => {
  const g = guide.value;
  const title = t(`guides.articles.${g.key}.title`);
  const description = t(`guides.articles.${g.key}.description`);
  const path = lp(guidePath(g));
  return {
    title: `${title} | ${SITE_NAME}`,
    description,
    path,
    jsonLd: [
      graph(
        authorNode(),
        articleNode({
          headline: title,
          description,
          path,
          published: g.published,
          modified: g.updated,
          inLanguage: locale.value,
        }),
        breadcrumbNode([
          { name: SITE_NAME, path: lp('/') },
          { name: t('guides.title'), path: lp('/guides') },
          { name: title, path },
        ]),
      ),
    ],
  };
});
</script>

<template>
  <article class="guide">
    <nav class="crumb" :aria-label="t('guides.all')">
      <RouterLink :to="lp('/')">{{ SITE_NAME }}</RouterLink>
      <span aria-hidden="true"> › </span>
      <RouterLink :to="lp('/guides')">{{ t('guides.title') }}</RouterLink>
    </nav>
    <h1>{{ t(`guides.articles.${guide.key}.title`) }}</h1>
    <p class="intro">{{ t(`guides.articles.${guide.key}.description`) }}</p>
    <p class="byline">
      <i18n-t keypath="guides.by" scope="global">
        <template #name>
          <a :href="LINKEDIN_URL" rel="noopener author">{{ AUTHOR_NAME }}</a>
        </template>
      </i18n-t>
      ·
      <time :datetime="guide.published">{{ t('guides.published', { date: formatDate(guide.published) }) }}</time>
      <template v-if="guide.updated !== guide.published">
        ·
        <time :datetime="guide.updated">{{ t('guides.updated', { date: formatDate(guide.updated) }) }}</time>
      </template>
    </p>
    <section v-for="section in sections" :key="section.heading">
      <h2>{{ section.heading }}</h2>
      <p v-for="paragraph in section.paragraphs" :key="paragraph">{{ paragraph }}</p>
      <ul v-if="section.list.length">
        <li v-for="item in section.list" :key="item">{{ item }}</li>
      </ul>
      <ol v-if="section.ordered.length">
        <li v-for="item in section.ordered" :key="item">{{ item }}</li>
      </ol>
      <div v-if="section.table" class="table-wrap">
        <table>
          <thead>
            <tr>
              <th v-for="cell in section.table.head" :key="cell" scope="col">{{ cell }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in section.table.rows" :key="row[0]">
              <th scope="row">{{ row[0] }}</th>
              <td v-for="(cell, i) in row.slice(1)" :key="i">{{ cell }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
    <RouterLink class="cta" :to="lp('/')">{{ t('guides.cta') }}</RouterLink>
    <nav class="pairs" :aria-label="t('guides.pairsTitle')">
      <h2>{{ t('guides.pairsTitle') }}</h2>
      <ul>
        <li v-for="pair in pairs" :key="pairPath(pair)">
          <RouterLink :to="lp(pairPath(pair))">{{ t('convert.pairs.link', { ...pair }) }}</RouterLink>
        </li>
      </ul>
    </nav>
    <nav class="others" :aria-label="t('guides.others')">
      <h2>{{ t('guides.others') }}</h2>
      <ul>
        <li v-for="other in others" :key="other.slug">
          <RouterLink :to="lp(guidePath(other))">{{ t(`guides.articles.${other.key}.title`) }}</RouterLink>
        </li>
      </ul>
    </nav>
  </article>
</template>

<style scoped>
.guide {
  max-width: 40rem;
  margin: 0 auto;
}

.crumb {
  margin: 0;
  font-size: 0.875rem;
}

.crumb a {
  display: inline-block;
  padding: var(--space-2) 0;
}

h1 {
  font-size: 1.75rem;
}

.intro {
  color: var(--color-text-muted);
  font-size: 1.0625rem;
}

.byline {
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

h2 {
  font-size: 1.125rem;
  margin-top: var(--space-6);
}

li + li {
  margin-top: var(--space-1);
}

.table-wrap {
  overflow-x: auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9375rem;
}

th,
td {
  text-align: left;
  vertical-align: top;
  padding: var(--space-2);
  border-bottom: 1px solid var(--color-border);
}

thead th {
  font-weight: 600;
}

tbody th {
  font-weight: 600;
}

.cta {
  display: inline-block;
  margin-top: var(--space-6);
  padding: var(--space-3) var(--space-6);
  border-radius: var(--radius);
  background: var(--color-primary);
  color: var(--color-primary-text);
  font-weight: 600;
  text-decoration: none;
}

.pairs ul,
.others ul {
  list-style: none;
  padding: 0;
}

.pairs ul {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
  gap: 0 var(--space-4);
}

.pairs a,
.others a {
  display: inline-flex;
  align-items: center;
  min-height: 2.75rem;
}

.pairs li + li,
.others li + li {
  margin-top: 0;
}
</style>
