<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, useTemplateRef } from 'vue';
import { useI18n } from 'vue-i18n';
import { RouterLink } from 'vue-router';

import { guidePath, GUIDES } from '@/shared/catalog/guides';
import { useLocalePath } from '@/shared/locale/localePath';
import { SITE_NAME, usePageMeta } from '@/shared/seo/pageMeta';
import { authorNode, breadcrumbNode, graph, webApplicationNode, websiteNode } from '@/shared/seo/structuredData';
import BaseButton from '@/shared/ui/BaseButton.vue';

import {
  checkFiles,
  type CheckedFile,
  csvOf,
  filesFromDrop,
  isRevitName,
  MAX_FILES,
  picked,
  type PickedFile,
} from './checkFiles';

const i18n = useI18n();
const { t, locale } = i18n;
const lp = useLocalePath();
const filesInput = useTemplateRef<HTMLInputElement>('files');
const folderInput = useTemplateRef<HTMLInputElement>('folder');

const rows = shallowRef<(CheckedFile | undefined)[]>([]);
const checking = ref(false);
/** How many Revit files were left out because the list was full. */
const leftOut = ref(0);
/** Nothing that was added was a Revit file. */
const nothingFound = ref(false);
const over = ref(false);
let depth = 0;
// A newer choice replaces a check still running.
let run = 0;

const path = computed(() => lp('/revit-version-checker'));

usePageMeta(() => ({
  title: t('checker.pageTitle'),
  description: t('checker.description'),
  path: path.value,
  jsonLd: [
    graph(
      authorNode(),
      websiteNode(t('convert.description'), locale.value),
      webApplicationNode({
        name: t('checker.title'),
        description: t('checker.description'),
        path: path.value,
        inLanguage: locale.value,
        operatingSystem: t('convert.schema.operatingSystem'),
        browserRequirements: t('convert.schema.browserRequirements'),
      }),
      breadcrumbNode([
        { name: SITE_NAME, path: lp('/') },
        { name: t('checker.title'), path: path.value },
      ]),
    ),
  ],
}));

interface RawSection {
  heading: string;
  body: string;
}

const sections = computed(() => {
  const messages: unknown = i18n.tm('checker.sections');
  return (messages as RawSection[]).map((s) => ({ heading: i18n.rt(s.heading), body: i18n.rt(s.body) }));
});

const shown = computed(() => rows.value.filter((row) => row !== undefined));
const compatibility = GUIDES.find((guide) => guide.key === 'compatibility');

function kindText(row: CheckedFile): string {
  return row.kind ? t(`convert.kind.${row.kind}`) : '';
}

function savedIn(row: CheckedFile): string {
  return row.year === null ? '' : `Revit ${String(row.year)}`;
}

function opensIn(row: CheckedFile): string {
  return row.year === null ? '' : t('checker.opensIn', { year: row.year });
}

function workshared(row: CheckedFile): string {
  if (row.workshared === null) {
    return '';
  }
  return row.workshared ? t('checker.yes') : t('checker.no');
}

function problem(row: CheckedFile): string {
  return row.problem ? t(`checker.problem.${row.problem}`) : '';
}

function sizeText(size: number): string {
  return `${new Intl.NumberFormat(locale.value, { maximumFractionDigits: 1 }).format(size / 1_048_576)} MB`;
}

async function add(chosen: PickedFile[], mine: number) {
  const revit = chosen.filter(({ name }) => isRevitName(name));
  const files = revit.slice(0, MAX_FILES);
  leftOut.value = revit.length - files.length;
  nothingFound.value = chosen.length > 0 && revit.length === 0;
  const results = Array.from<CheckedFile | undefined>({ length: files.length });
  rows.value = [];
  checking.value = files.length > 0;
  await checkFiles(files, (index, row) => {
    if (mine === run) {
      results[index] = row;
      rows.value = [...results];
    }
  });
  if (mine === run) {
    checking.value = false;
  }
}

function onPicked(event: Event) {
  const element = event.target as HTMLInputElement;
  run += 1;
  void add([...(element.files ?? [])].map(picked), run);
  // The same folder can then be chosen again.
  element.value = '';
}

function enter() {
  depth += 1;
  over.value = true;
}

function leave() {
  depth = Math.max(0, depth - 1);
  over.value = depth > 0;
}

function onDrop(event: DragEvent) {
  depth = 0;
  over.value = false;
  if (event.dataTransfer) {
    // The drop's contents are only readable during this event, so its folders are opened right away.
    run += 1;
    const mine = run;
    rows.value = [];
    checking.value = true;
    void filesFromDrop(event.dataTransfer).then((files) => {
      if (mine === run) {
        void add(files, mine);
      }
    });
  }
}

function downloadCsv() {
  const head = ['file', 'savedIn', 'opensIn', 'type', 'build', 'workshared', 'size', 'status'].map((key) =>
    t(`checker.column.${key}`),
  );
  const body = shown.value.map((row) => [
    row.name,
    savedIn(row),
    opensIn(row),
    kindText(row),
    row.build ?? '',
    workshared(row),
    String(row.size),
    problem(row),
  ]);
  const url = URL.createObjectURL(new Blob([csvOf([head, ...body])], { type: 'text/csv;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = 'revit-versions.csv';
  link.click();
  URL.revokeObjectURL(url);
}

// A file dropped just outside the zone would make the browser open it; only the zone takes drops.
function ignoreStray(event: DragEvent) {
  event.preventDefault();
}

onMounted(() => {
  if (folderInput.value) {
    folderInput.value.webkitdirectory = true;
  }
  window.addEventListener('dragover', ignoreStray);
  window.addEventListener('drop', ignoreStray);
});
onBeforeUnmount(() => {
  run += 1;
  window.removeEventListener('dragover', ignoreStray);
  window.removeEventListener('drop', ignoreStray);
});
</script>

<template>
  <article class="checker">
    <h1>{{ t('checker.title') }}</h1>
    <p class="intro">{{ t('checker.intro') }}</p>

    <div
      class="zone"
      :class="{ 'is-over': over }"
      data-testid="checker-zone"
      @dragenter.prevent="enter"
      @dragover.prevent
      @dragleave="leave"
      @drop.prevent="onDrop"
    >
      <div class="actions">
        <BaseButton @click="filesInput?.click()">{{ t('checker.chooseFiles') }}</BaseButton>
        <BaseButton variant="ghost" @click="folderInput?.click()">{{ t('checker.chooseFolder') }}</BaseButton>
      </div>
      <p class="hint">{{ t('checker.drop', { max: MAX_FILES }) }}</p>
      <p class="hint">{{ t('checker.privacy') }}</p>
      <input
        ref="files"
        type="file"
        hidden
        multiple
        accept=".rvt,.rte,.rfa,.rft"
        :aria-label="t('checker.chooseFiles')"
        @change="onPicked"
      />
      <input ref="folder" type="file" hidden :aria-label="t('checker.chooseFolder')" @change="onPicked" />
    </div>

    <p v-if="nothingFound" class="note" role="status">{{ t('checker.nothingFound') }}</p>
    <p v-if="leftOut > 0" class="note" role="status">{{ t('checker.leftOut', { count: leftOut, max: MAX_FILES }) }}</p>
    <p v-if="checking" class="note" role="status">{{ t('checker.checking') }}</p>

    <template v-if="shown.length > 0">
      <div class="table-wrap">
        <table>
          <thead>
            <tr>
              <th scope="col">{{ t('checker.column.file') }}</th>
              <th scope="col">{{ t('checker.column.savedIn') }}</th>
              <th scope="col">{{ t('checker.column.opensIn') }}</th>
              <th scope="col">{{ t('checker.column.type') }}</th>
              <th scope="col">{{ t('checker.column.build') }}</th>
              <th scope="col">{{ t('checker.column.workshared') }}</th>
              <th scope="col">{{ t('checker.column.size') }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, index) in shown" :key="index">
              <td class="name">{{ row.name }}</td>
              <template v-if="row.problem">
                <td colspan="5" class="problem">{{ problem(row) }}</td>
              </template>
              <template v-else>
                <td class="nowrap">{{ savedIn(row) }}</td>
                <td class="nowrap">{{ opensIn(row) }}</td>
                <td>{{ kindText(row) }}</td>
                <td class="nowrap">{{ row.build }}</td>
                <td>{{ workshared(row) }}</td>
              </template>
              <td class="nowrap">{{ sizeText(row.size) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
      <div class="after">
        <BaseButton variant="ghost" :disabled="checking" @click="downloadCsv">{{ t('checker.csv') }}</BaseButton>
        <p>
          {{ t('checker.next') }}
          <RouterLink :to="lp('/')">{{ t('checker.nextLink') }}</RouterLink>
        </p>
      </div>
    </template>

    <section v-for="section in sections" :key="section.heading">
      <h2>{{ section.heading }}</h2>
      <p>{{ section.body }}</p>
    </section>
    <p v-if="compatibility">
      <RouterLink :to="lp(guidePath(compatibility))">{{ t('guides.articles.compatibility.title') }}</RouterLink>
    </p>
  </article>
</template>

<style scoped>
.intro {
  color: var(--color-text-muted);
}

.zone {
  border: 2px dashed var(--color-border);
  border-radius: 0.75rem;
  padding: var(--space-6) var(--space-3);
  text-align: center;
  transition:
    background 0.15s,
    border-color 0.15s;
}

.zone.is-over {
  background: var(--color-primary-soft);
  border-color: var(--color-primary);
}

.actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--space-2);
}

.hint {
  margin: var(--space-2) 0 0;
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.note {
  margin: var(--space-3) 0 0;
  color: var(--color-text-muted);
}

/* Wider than the text column, so all the columns fit on a desktop screen. */
.table-wrap {
  position: relative;
  left: 50%;
  width: min(64rem, 100vw - 2rem);
  overflow-x: auto;
  margin-top: var(--space-4);
  transform: translateX(-50%);
}

table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.875rem;
}

th,
td {
  padding: var(--space-2);
  border-bottom: 1px solid var(--color-border);
  text-align: left;
  vertical-align: top;
}

.name {
  min-width: 12rem;
  overflow-wrap: anywhere;
}

.nowrap {
  white-space: nowrap;
}

.problem {
  color: var(--color-danger);
}

.after {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  margin-top: var(--space-3);
}

.after p {
  margin: 0;
}

section {
  margin-top: var(--space-6);
}
</style>
