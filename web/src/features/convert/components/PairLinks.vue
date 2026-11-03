<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { RouterLink } from 'vue-router';

import { PAIRS, pairPath, type Pair } from '@/shared/catalog/pairs';
import { useLocalePath } from '@/shared/locale/localePath';

const props = defineProps<{ current?: Pair | null }>();
const { t } = useI18n();
const lp = useLocalePath();

// A pair's page lists the other pairs, not itself.
const links = computed(() =>
  PAIRS.filter((p) => !props.current || p.from !== props.current.from || p.to !== props.current.to),
);
</script>

<template>
  <nav class="pairs" aria-labelledby="pairs-title">
    <h2 id="pairs-title">{{ t(current ? 'convert.pairs.others' : 'convert.pairs.title') }}</h2>
    <ul>
      <li v-for="pair in links" :key="pairPath(pair)">
        <RouterLink :to="lp(pairPath(pair))">{{ t('convert.pairs.link', { ...pair }) }}</RouterLink>
      </li>
    </ul>
  </nav>
</template>

<style scoped>
.pairs {
  width: min(40rem, 100%);
  margin-top: calc(var(--space-6) * 2);
}

h2 {
  font-size: 1.125rem;
  margin: 0 0 var(--space-3);
}

ul {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(11rem, 1fr));
  gap: 0 var(--space-4);
  list-style: none;
  margin: 0;
  padding: 0;
  font-size: 0.9375rem;
}

li a {
  display: inline-flex;
  align-items: center;
  min-height: 2.75rem;
}
</style>
