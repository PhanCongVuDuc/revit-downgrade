<script setup lang="ts">
import { useI18n } from 'vue-i18n';

const { t } = useI18n();
</script>

<template>
  <p class="support">{{ t('convert.add.support') }}</p>
</template>

<style scoped>
.support {
  width: min(27.5rem, 100%);
  margin: var(--space-4) 0 0;
  color: var(--color-text-muted);
  font-size: 0.875rem;
  text-align: center;
}
</style>
