<template>
  <div
    class="min-h-screen bg-gradient-to-br from-base-300/20 to-primary/10 flex flex-col items-center justify-start w-full p-5"
  >
    <div class="backdrop-blur-sm w-full flex flex-col items-center">
      <div class="container px-4 pt-8 mx-auto mt-16 md:px-8 lg:px-16">
        <div v-if="stats" class="mb-8">
          <h2 class="text-2xl font-bold mb-4 text-base-content p-5 pt-9 pl-0">
            Download Stats
          </h2>
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

        <div
          v-if="Object.keys(downloads).length"
          class="grid gap-4 md:grid-cols-2 lg:grid-cols-3 pt-5"
        >
          <div
            v-for="(download, id) in downloads"
            :key="id"
            class="card bg-base-200 shadow-xl hover:bg-base-300 transition-colors duration-200"
          >
            <div class="card-body">
              <h3 class="card-title text-base-content">Download #{{ id }}</h3>
              <div class="space-y-2">
                <p class="text-base-content/70 break-all">
                  <span class="font-semibold">URL:</span> {{ download.url }}
                </p>
                <p class="text-base-content/70">
                  <span class="font-semibold">Status:</span>
                  <span
                    :class="{
                      'text-primary':
                        download.progress.status === 'downloading',
                      'text-warning': download.progress.status === 'paused',
                      'text-success': download.progress.status === 'completed',
                      'text-error': download.progress.status === 'failed',
                    }"
                  >
                    {{ download.progress.status }}
                  </span>
                </p>
                <div class="w-full">
                  <div class="flex justify-between text-sm mb-1">
                    <span>Progress</span>
                    <span
                      >{{ download.progress.downloaded }} /
                      {{ download.progress.total_size || "Unknown" }}</span
                    >
                  </div>
                  <progress
                    class="progress progress-primary w-full"
                    :value="download.progress.downloaded"
                    :max="download.progress.total_size || 100"
                  ></progress>
                  <div class="card-actions justify-end pt-4">
                    <AluButton
                      v-if="
                        download.progress.status !== 'completed' &&
                        download.progress.status !== 'cancelled' &&
                        download.progress.status !== 'failed'
                      "
                      level="danger"
                      size="small"
                      @click="cancelDownload(id)"
                    >
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
</template>

<script>
import { ref, onMounted } from "vue";
import AluButton from "./AluButton.vue";

export default {
  name: "DownloadList",
  components: {
    AluButton,
  },
  setup() {
    const downloads = ref({});
    const stats = ref(null);

    const fetchDownloads = async () => {
      try {
        const response = await fetch("/api/downloads/");
        if (!response.ok) {
          throw new Error("Failed to fetch downloads");
        }
        downloads.value = await response.json();
      } catch (error) {
        console.error("Error fetching downloads:", error);
      }
    };

    const fetchStats = async () => {
      try {
        const response = await fetch("/api/downloads/stats");
        if (!response.ok) {
          throw new Error("Failed to fetch stats");
        }
        stats.value = await response.json();
      } catch (error) {
        console.error("Error fetching stats:", error);
      }
    };

    const cancelDownload = async (id) => {
      try {
        const response = await fetch(`/api/downloads/${id}/cancel`, {
          method: "GET",
        });
        if (!response.ok) {
          throw new Error("Failed to cancel download");
        }

        await Promise.all([fetchDownloads(), fetchStats()]);
      } catch (error) {
        console.error("Error cancelling download:", error);
      }
    };

    onMounted(() => {
      fetchDownloads();
      fetchStats();
    });

    return {
      downloads,
      stats,
      cancelDownload,
    };
  },
};
</script>
