<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';

import BaseButton from '@/shared/ui/BaseButton.vue';
import BaseCheckbox from '@/shared/ui/BaseCheckbox.vue';

const props = defineProps<{
  /** The Revit version the file was saved in. */
  year: number;
  /** The file ends in empty data, as if it was not fully downloaded or copied. */
  incomplete: boolean;
  /** The "convert anyway" request is on its way. */
  confirming: boolean;
}>();
const emit = defineEmits<{ confirm: []; chooseAnother: [] }>();
const { t } = useI18n();
// Never ticked for the user: converting anyway is only for a file they opened themselves.
const opened = ref(false);
const canConfirm = computed(() => opened.value && !props.confirming);

function confirm() {
  if (canConfirm.value) {
    emit('confirm');
  }
}
</script>

<template>
  <div class="needs-check-panel">
    <p class="title" role="alert">
      {{ incomplete ? t('convert.needsCheck.incomplete') : t('convert.needsCheck.damaged') }}
    </p>
    <p class="text">{{ t('convert.needsCheck.intro') }}</p>
    <ul class="text checks">
      <li>{{ t('convert.needsCheck.checkCopy') }}</li>
      <li>{{ t('convert.needsCheck.checkRevit', { year }) }}</li>
    </ul>
    <p class="muted">{{ t('convert.needsCheck.nothingKept') }}</p>
    <BaseButton variant="primary" @click="emit('chooseAnother')">{{ t('convert.chooseAnother') }}</BaseButton>
    <form class="anyway" @submit.prevent="confirm">
      <p class="text">{{ t('convert.needsCheck.anywayIntro') }}</p>
      <BaseCheckbox v-model="opened">{{ t('convert.needsCheck.opened', { year }) }}</BaseCheckbox>
      <BaseButton type="submit" variant="ghost" :disabled="!canConfirm">
        {{ confirming ? t('convert.needsCheck.confirming') : t('convert.needsCheck.confirm') }}
      </BaseButton>
    </form>
  </div>
</template>

<style scoped>
.needs-check-panel {
  display: grid;
  gap: var(--space-3);
  justify-items: start;
}

.title {
  margin: 0;
  font-weight: 700;
}

.text {
  margin: 0;
  font-size: 0.9375rem;
}

.checks {
  padding-left: var(--space-4);
  display: grid;
  gap: var(--space-1);
}

.muted {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.875rem;
}

.anyway {
  display: grid;
  gap: var(--space-3);
  justify-items: start;
  width: 100%;
  border-top: 1px solid var(--color-border);
  padding-top: var(--space-3);
}
</style>
