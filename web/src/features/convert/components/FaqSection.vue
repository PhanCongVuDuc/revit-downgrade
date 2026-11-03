<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';

import type { Pair } from '@/shared/catalog/pairs';

const props = defineProps<{ pair?: Pair | null }>();
const { t } = useI18n();

// Phrased the way people search for this, so the answers are also what search engines index. A pair's page asks
// about its own two versions first.
const questions = computed(() => {
  const pair = props.pair;
  const general = ['supported', 'lost', 'backsave', 'safe'].map((key) => ({
    key,
    q: t(`convert.faq.items.${key}.q`),
    a: t(`convert.faq.items.${key}.a`),
  }));
  if (pair) {
    return [
      ...['howTo', 'saveOlder'].map((key) => ({
        key,
        q: t(`convert.faq.pairItems.${key}.q`, { ...pair }),
        a: t(`convert.faq.pairItems.${key}.a`, { ...pair }),
      })),
      ...general,
    ];
  }
  return [
    ...['howTo', 'saveOlder'].map((key) => ({
      key,
      q: t(`convert.faq.items.${key}.q`),
      a: t(`convert.faq.items.${key}.a`),
    })),
    ...general,
  ];
});
</script>

<template>
  <section class="faq" aria-labelledby="faq-title">
    <h2 id="faq-title">{{ t('convert.faq.title') }}</h2>
    <details v-for="item in questions" :key="item.key">
      <summary>{{ item.q }}</summary>
      <p>{{ item.a }}</p>
    </details>
  </section>
</template>

<style scoped>
.faq {
  width: min(40rem, 100%);
  margin-top: calc(var(--space-6) * 2);
}

h2 {
  font-size: 1.125rem;
  margin: 0 0 var(--space-3);
}

details {
  border-bottom: 1px solid var(--color-border);
}

summary {
  cursor: pointer;
  font-weight: 600;
  padding: var(--space-3) 0;
  min-height: 2.75rem;
  box-sizing: border-box;
}

p {
  color: var(--color-text-muted);
  margin: 0 0 var(--space-3);
}
</style>
