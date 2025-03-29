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
        <div v-if="sortedDownloads.length" class="flex flex-col gap-4 pt-5">
          <div
            v-for="download in sortedDownloads"
            :key="download.id"
            class="card bg-base-200 shadow-xl hover:bg-base-300 transition-colors duration-200 w-full"
          >
            <div class="card-body">
              <h3 class="card-title text-base-content">Download #{{ download.id }}</h3>
              <div class="space-y-2">
                <p class="text-base-content/70 break-all">
                  <span class="font-semibold">URL:</span> {{ download.item.url }}
                </p>
                <p class="text-base-content/70">
                  <span class="font-semibold">Status:</span>
                  <span
                    :class="{
                      'text-primary':
                        download.progress.status === 'Downloading',
                      'text-warning': download.progress.status === 'Paused',
                      'text-success': download.progress.status === 'Completed',
                      'text-error': 
                        download.progress.status === 'Cancelled' || 
                        download.progress.status.startsWith('Failed'),
                    }"
                  >
                    {{ download.progress.status }}
                  </span>
                </p>
                <div class="w-full">
                  <div class="flex justify-between text-sm mb-1">
                    <span>Progress</span>
                    <span v-if="download.progress.total_size">
                      {{ formatBytes(download.progress.downloaded) }} /
                      {{ formatBytes(download.progress.total_size) }}
                      ({{ calculatePercentage(download.progress) }}%)
                    </span>
                    <span v-else>
                      {{ formatBytes(download.progress.downloaded) }} / Unknown
                    </span>
                  </div>
                  <progress
                    class="progress progress-primary w-full"
                    :value="download.progress.downloaded"
                    :max="download.progress.total_size || 100"
                  ></progress>
                  <div class="card-actions justify-end pt-4">
                    <AluButton
                      v-if="
                        download.progress.status !== 'Completed' &&
                        download.progress.status !== 'Cancelled' &&
                        !download.progress.status.startsWith('Failed')
                      "
                      level="danger"
                      size="small"
                      @click="handleCancelDownload(download.id)"
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

<script lang="ts">
import { ref, onMounted, onUnmounted, defineComponent, computed } from "vue";
import AluButton from "./AluButton.vue";
import { 
  fetchDownloads, 
  fetchStats, 
  cancelDownload,
  formatBytes,
  calculatePercentage
} from "../utils/download";
import type { 
  DownloadStats, 
  DownloadItemWithProgress 
} from "../utils/download";

export default defineComponent({
  name: "DownloadList",
  components: {
    AluButton,
  },
  setup() {
    const downloads = ref<Record<string, DownloadItemWithProgress>>({});
    const stats = ref<DownloadStats | null>(null);
    
    const sortedDownloads = computed(() => {
      // Convert downloads object to array with id included
      const downloadsArray = Object.entries(downloads.value).map(([id, download]) => ({
        id,
        ...download
      }));
      
      // Sort by ID in descending order (newest first)
      return downloadsArray.sort((a, b) => b.id.localeCompare(a.id));
    });
    
    const refreshData = async () => {
      try {
        const [downloadsData, statsData] = await Promise.all([
          fetchDownloads(),
          fetchStats()
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
    });
    
    return {
      downloads,
      sortedDownloads,
      stats,
      handleCancelDownload,
      formatBytes,
      calculatePercentage
    };
  },
});
</script>
