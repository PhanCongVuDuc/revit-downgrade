<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';

import { CONTACT_EMAIL } from '@/shared/contact/contact';
import BaseButton from '@/shared/ui/BaseButton.vue';

import type { ReportDeletion } from '../stores/convertStore';

const props = defineProps<{
  /** The job's error kind. */
  kind: string;
  message: string | null;
  reportCode: string | null;
  /** This job's failure made the report, so the file in it is this visitor's. */
  reportOwned: boolean;
  reportDeleted: boolean;
  reportDeletion: ReportDeletion;
}>();
const emit = defineEmits<{ deleteReport: []; deleteNow: []; chooseAnother: [] }>();
const i18n = useI18n();
const { t } = i18n;

const failure = computed(() => {
  const key = i18n.te(`convert.failure.${props.kind}`) ? props.kind : 'unknown';
  return t(`convert.failure.${key}`, { message: props.message ?? '' });
});

const kept = computed(() => props.reportCode !== null && props.reportOwned && !props.reportDeleted);
const deleting = computed(() => props.reportDeletion === 'deleting');
/** Past this point only the author can delete the file, so the button would promise too much. */
const gone = computed(() => props.reportDeletion === 'gone');
</script>

<template>
  <div class="failure-panel">
    <p class="err" role="alert">{{ failure }}</p>
    <p v-if="reportDeleted" class="muted" role="status">{{ t('convert.report.deleted') }}</p>
    <template v-else-if="kept">
      <p class="muted">{{ t('convert.report.kept', { code: reportCode }) }}</p>
      <p v-if="gone" class="err small" role="alert">
        {{ t('convert.report.deleteGone', { code: reportCode, email: CONTACT_EMAIL }) }}
      </p>
      <p v-else-if="reportDeletion === 'failed'" class="err small" role="alert">
        {{ t('convert.report.deleteFailed', { code: reportCode, email: CONTACT_EMAIL }) }}
      </p>
      <BaseButton v-if="!gone" variant="ghost" :disabled="deleting" @click="emit('deleteReport')">
        {{ deleting ? t('convert.report.deleting') : t('convert.report.delete') }}
      </BaseButton>
    </template>
    <p v-else-if="reportCode" class="muted">{{ t('convert.report.known', { code: reportCode }) }}</p>
    <BaseButton v-if="!reportOwned" variant="ghost" @click="emit('deleteNow')">
      {{ t('convert.result.deleteNow') }}
    </BaseButton>
    <BaseButton variant="ghost" @click="emit('chooseAnother')">{{ t('convert.chooseAnother') }}</BaseButton>
  </div>
</template>

<style scoped>
.failure-panel {
  display: grid;
  gap: var(--space-3);
  justify-items: start;
}

.err {
  margin: 0;
  color: var(--color-danger);
  font-weight: 700;
}

.small {
  font-weight: 500;
  font-size: 0.875rem;
}

.muted {
  margin: 0;
  color: var(--color-text-muted);
  font-size: 0.875rem;
}
</style>
