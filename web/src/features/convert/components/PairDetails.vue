<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';

import { pairVersions, type Pair } from '@/shared/catalog/pairs';
import { revitHelpUrl, VERSION_STEPS } from '@/shared/catalog/versionChanges';

const props = defineProps<{ pair: Pair }>();
const { t } = useI18n();

/** How many versions the file goes down, and the versions it passes through on the way. */
const steps = computed(() => props.pair.from - props.pair.to);
const route = computed(() => pairVersions(props.pair).join(' → '));
/** The version steps this downgrade goes through, newest first. */
const changes = computed(() => VERSION_STEPS.filter((s) => s.year <= props.pair.from && s.year > props.pair.to));
</script>

<template>
  <section class="pair-details" aria-labelledby="pair-details-title">
    <h2 id="pair-details-title">{{ t('convert.pairPage.details.title', { ...pair }) }}</h2>
    <p>{{ t('convert.pairPage.details.why', { ...pair }) }}</p>
    <p>{{ t('convert.pairPage.details.steps', { ...pair, route, n: steps }, steps) }}</p>
    <p>{{ t('convert.pairPage.details.time', { n: steps }, steps) }}</p>
    <p>{{ t('convert.pairPage.details.opensIn', { ...pair }) }}</p>
    <p>{{ t('convert.pairPage.details.files', { ...pair }) }}</p>
  </section>
  <section class="pair-details" aria-labelledby="pair-changes-title">
    <h2 id="pair-changes-title">{{ t('convert.changes.title', { ...pair }) }}</h2>
    <p>{{ t('convert.changes.intro') }}</p>
    <div v-for="step in changes" :key="step.year" class="step">
      <h3>{{ t('convert.changes.step', { from: step.year, to: step.year - 1 }) }}</h3>
      <p>
        <strong>{{ t('convert.changes.removedLabel', { to: step.year - 1 }) }}</strong>
        {{ t(`convert.changes.${step.year}.removed`) }}
      </p>
      <p v-if="step.other">
        <strong>{{ t('convert.changes.otherLabel') }}</strong>
        {{ t(`convert.changes.${step.year}.other`) }}
      </p>
      <p class="more">
        <a :href="revitHelpUrl(step.year)" target="_blank" rel="noopener">
          {{ t('convert.changes.whatsNew', { year: step.year }) }}
        </a>
      </p>
    </div>
  </section>
</template>

<style scoped>
.pair-details {
  width: min(40rem, 100%);
  margin-top: calc(var(--space-6) * 2);
}

h2 {
  font-size: 1.125rem;
  margin: 0 0 var(--space-3);
}

h3 {
  font-size: 1rem;
  margin: var(--space-4) 0 var(--space-1);
}

.step p {
  margin: var(--space-1) 0;
}

.more a {
  display: inline-flex;
  align-items: center;
  min-height: 2.75rem;
}
</style>
