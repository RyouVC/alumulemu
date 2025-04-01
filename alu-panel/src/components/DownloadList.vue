<template>
  <div
    class="min-h-screen bg-gradient-to-br from-base-300/20 to-primary/10 flex flex-col items-center justify-start w-full p-5">
    <div class="backdrop-blur-sm w-full flex flex-col items-center">
      <div class="container px-4 pt-8 mx-auto mt-16 md:px-8 lg:px-16">
        <div v-if="stats" class="mb-8">
          <div class="flex justify-between items-center mb-4">
            <h2 class="text-2xl font-bold text-base-content p-5 pt-9 pl-0">
              Download Stats
            </h2>
            <div class="flex gap-3">
              <AluButton level="primary" @click="openUrlDialog">
                Add Download
              </AluButton>
              <AluButton v-if="
                stats.completed > 0 || stats.failed > 0 || stats.cancelled > 0
              " level="secondary" @click="handleCleanup" :disabled="isCleaning">
                Clean Up Downloads
              </AluButton>
            </div>
          </div>
          <div class="stats shadow bg-base-200 w-full">
            <div class="stat">
              <div class="stat-title">Total</div>
              <div class="stat-value">{{ stats.total }}</div>
            </div>
            <div class="stat">
              <div class="stat-title">Queued</div>
              <div class="stat-value text-info">{{ stats.queued }}</div>
            </div>
            <div class="stat">
              <div class="stat-title">Downloading</div>
              <div class="stat-value text-primary">{{ stats.downloading }}</div>
            </div>
            <div class="stat">
              <div class="stat-title">Paused</div>
              <div class="stat-value text-warning">{{ stats.paused }}</div>
            </div>
            <div class="stat">
              <div class="stat-title">Completed</div>
              <div class="stat-value text-success">{{ stats.completed }}</div>
            </div>
            <div class="stat">
              <div class="stat-title">Cancelled</div>
              <div class="stat-value text-error">{{ stats.cancelled }}</div>
            </div>
            <div class="stat">
              <div class="stat-title">Failed</div>
              <div class="stat-value text-error">{{ stats.failed }}</div>
            </div>
          </div>
        </div>
        <div v-if="sortedDownloads.length" class="flex flex-col gap-4 pt-5">
          <div v-for="download in sortedDownloads" :key="download.id"
            class="card bg-base-200 shadow-xl hover:bg-base-300 transition-colors duration-200 w-full">
            <div class="card-body">
              <h3 class="card-title text-base-content">
                Download #{{ download.id }}
              </h3>
              <div class="space-y-2">
                <p class="text-base-content/70 break-all">
                  <span class="font-semibold">URL:</span>
                  {{ download.item.url }}
                </p>
                <p class="text-base-content/70">
                  <span class="font-semibold">Status:</span>
                    <span :class="{
                    'text-primary': getStatusString(download.progress.status) === 'Downloading',
                    'text-warning': getStatusString(download.progress.status) === 'Paused',
                    'text-success': getStatusString(download.progress.status) === 'Completed',
                    'text-error': 
                      getStatusString(download.progress.status) === 'Cancelled' || 
                      getStatusString(download.progress.status).startsWith('Failed'),
                    }">
                    {{ download.progress.status }}
                  </span>
                </p>
                <div class="w-full">
                  <div class="flex items-center gap-4">
                    <div class="flex items-center gap-2">
                      <span>Progress</span>
                      <span v-if="download.progress.total_size">
                        {{ formatBytes(download.progress.downloaded) }} /
                        {{ formatBytes(download.progress.total_size) }}
                        ({{ calculatePercentage(download.progress) }}%)
                      </span>
                      <span v-else>
                        {{ formatBytes(download.progress.downloaded) }} /
                        Unknown
                      </span>
                    </div>
                    <progress class="progress progress-primary flex-1" :value="download.progress.downloaded"
                      :max="download.progress.total_size || 100"></progress>
                    <AluButton v-if="
                      !['Completed', 'Cancelled'].includes(getStatusString(download.progress.status)) &&
                      !getStatusString(download.progress.status).startsWith('Failed')
                    " level="danger" size="small" @click="handleCancelDownload(download.id)">
                      Cancel Download
                    </AluButton>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
        <div v-else class="card bg-base-200 p-6 text-center">
          <p class="text-base-content/70">No downloads available</p>
        </div>
      </div>
    </div>
  </div>

  <Teleport to="body">
    <!-- URL download modal -->
    <div v-if="isUrlDialogOpen" class="fixed inset-0 z-[9999] flex items-center justify-center bg-black/50"
      @click.self="closeUrlDialog">
      <div class="p-6 rounded-lg shadow-xl bg-base-100 w-96 max-w-[90vw]">
        <h3 class="mb-4 text-xl font-bold">Download from URL</h3>
        <div class="pt-4 form-control">
          <input type="text" placeholder="https://example.com/game.nsp" v-model="downloadUrl"
            class="w-full input input-bordered" />
        </div>
        <div class="flex justify-end gap-2 mt-4 pt-4 border-t border-base-300">
          <AluButton @click="closeUrlDialog" size="small">Cancel</AluButton>
          <AluButton level="primary" :disabled="!isValidUrl" @click="submitUrlDownload" size="small">Download
          </AluButton>
        </div>
      </div>
    </div>

    <!-- Toast container for notifications -->
    <div class="toast toast-end z-[9999] p-4 mb-4 mr-4">
      <div v-for="(toast, index) in toasts" :key="index" class="alert my-2" :class="toast.type">
        <span>{{ toast.message }}</span>
      </div>
    </div>
  </Teleport>
</template>

<script lang="ts">
import { ref, onMounted, onUnmounted, defineComponent, computed } from "vue";
import AluButton from "./AluButton.vue";
import {
  fetchDownloads,
  fetchStats,
  cancelDownload,
  cleanupDownloads,
  formatBytes,
  calculatePercentage,
  getStatusString,
} from "../utils/download";
import { importGameURL } from "../utils/import";
import type {
  DownloadStats,
  DownloadItemWithProgress,
} from "../utils/download";

interface Toast {
  id: number;
  message: string;
  type: string;
  timeoutId?: number;
}

export default defineComponent({
  name: "DownloadList",
  components: {
    AluButton,
  },
  setup() {
    const downloads = ref<Record<string, DownloadItemWithProgress>>({});
    const stats = ref<DownloadStats | null>(null);
    const isCleaning = ref<boolean>(false);

    // URL modal state
    const isUrlDialogOpen = ref(false);
    const downloadUrl = ref("");

    // Toast state
    const toasts = ref<Toast[]>([]);
    let nextToastId = 0;

    const sortedDownloads = computed(() => {
      // Convert downloads object to array with id included
      const downloadsArray = Object.entries(downloads.value).map(
        ([id, download]) => ({
          id,
          ...download,
        }),
      );

      // Sort by ID in descending order (newest first)
      return downloadsArray.sort((a, b) => b.id.localeCompare(a.id));
    });

    const isValidUrl = computed(() => {
      if (!downloadUrl.value) return false;
      try {
        const url = new URL(downloadUrl.value);
        return url.protocol === "http:" || url.protocol === "https:";
      } catch (e) {
        return false;
      }
    });

    const refreshData = async () => {
      try {
        const [downloadsData, statsData] = await Promise.all([
          fetchDownloads(),
          fetchStats(),
        ]);

        downloads.value = downloadsData;
        stats.value = statsData;
      } catch (error) {
        console.error("Error refreshing download data:", error);
      }
    };

    const handleCancelDownload = async (id: string) => {
      try {
        await cancelDownload(id);
        await refreshData();
      } catch (error) {
        console.error("Error cancelling download:", error);
      }
    };

    const handleCleanup = async () => {
      if (isCleaning.value) return;

      try {
        isCleaning.value = true;
        const result = await cleanupDownloads();
        console.log(`Cleaned up downloads, remaining: ${result.count}`);
        await refreshData();
      } catch (error) {
        console.error("Error cleaning up downloads:", error);
      } finally {
        isCleaning.value = false;
      }
    };

    const openUrlDialog = () => {
      isUrlDialogOpen.value = true;
    };

    const closeUrlDialog = () => {
      isUrlDialogOpen.value = false;
      downloadUrl.value = "";
    };

    const submitUrlDownload = async () => {
      if (!isValidUrl.value) return;

      try {
        await importGameURL(downloadUrl.value);
        showToastNotification("Download added successfully", "alert-success");
        await refreshData();
        closeUrlDialog();
      } catch (error) {
        console.error("Error adding download:", error);
        const errorMsg = error instanceof Error ? error.message : "Error adding download";
        showToastNotification(errorMsg, "alert-error");
      }
    };

    const showToastNotification = (message: string, type = "alert-info", duration = 3000) => {
      // Create a unique ID for this toast
      const id = nextToastId++;

      // Add the toast to our list
      const toast: Toast = {
        id,
        message,
        type,
      };

      toasts.value.push(toast);

      // Auto-hide the toast after duration
      const timeoutId = window.setTimeout(() => {
        // Remove this toast when the timeout expires
        toasts.value = toasts.value.filter(t => t.id !== id);
      }, duration);

      // Store the timeout ID so we can clear it if needed
      toast.timeoutId = timeoutId;
    };

    // Set up polling to refresh downloads automatically
    let pollingInterval: number | undefined;

    onMounted(() => {
      refreshData();

      // Start polling for updates every 2 seconds
      pollingInterval = window.setInterval(() => {
        refreshData();
      }, 2000);
    });

    // Clean up interval when component is unmounted
    onUnmounted(() => {
      if (pollingInterval) {
        clearInterval(pollingInterval);
      }

      // Clear all active toast timeouts
      toasts.value.forEach(toast => {
        if (toast.timeoutId) {
          clearTimeout(toast.timeoutId);
        }
      });
    });

    return {
      downloads,
      sortedDownloads,
      stats,
      isCleaning,
      isUrlDialogOpen,
      downloadUrl,
      toasts,
      isValidUrl,
      handleCancelDownload,
      handleCleanup,
      formatBytes,
      calculatePercentage,
      getStatusString,
      openUrlDialog,
      closeUrlDialog,
      submitUrlDownload,
    };
  },
});
</script>
