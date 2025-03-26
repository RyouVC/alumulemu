<template>
    <!-- fixed background -->
    <div v-if="metadata" class="fixed inset-0 h-screen bg-center bg-no-repeat bg-cover brightness-50 blur-xl opacity-70"
        :style="{ backgroundImage: `url(${metadata.bannerUrl})` }"></div>

    <div class="relative z-10">
        <div v-if="metadata" class="w-full px-4 md:px-8 lg:px-16">
            <div class="h-48"></div> <!-- Increased spacing to show more background -->

            <!-- Floating elements container -->
            <div class="relative">
                <!-- Game icon -->
                <img :src="metadata.iconUrl" :alt="metadata.name"
                    class="absolute z-20 object-cover w-48 h-48 rounded-lg shadow-xl -top-24 left-8 ring-4 shadow-base-content/20" />

                <!-- Title and basic info -->
                <div class="absolute z-20 -top-24 left-64">
                    <h1 class="text-4xl font-bold text-white">{{ metadata.name }}</h1>
                    <div class="flex items-center gap-2 mt-2 text-xl text-white">
                        <span>{{ metadata.publisher }}</span>
                        <span class="opacity-50">{{ metadata.releaseDate.slice(0, 4) }}</span>
                        <span class="opacity-50">|</span>
                        <span>{{
                            metadata.size > 1024 * 1024 * 1024
                                ? (metadata.size / (1024 * 1024 * 1024)).toFixed(2) + " GB"
                                : (metadata.size / (1024 * 1024)).toFixed(2) + " MB"
                        }}</span>
                    </div>
                </div>

                <!-- Dark card container -->
                <div class="w-full shadow-xl card bg-base-100">
                    <!-- Download section - positioned to overlap card -->
                    <div class="absolute z-20 flex flex-col items-end gap-2 -top-6 right-8">
                        <AluButton @click="downloadGame(selectedDownloadId || metadata.titleId)" level="success"
                            size="medium" class="shadow-lg shadow-success/10" style="width: 100%;">
                            <div class="flex items-center w-full">
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                                    stroke="currentColor" class="mr-2 size-6">
                                    <path stroke-linecap="round" stroke-linejoin="round"
                                        d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3" />
                                </svg>
                                <span class="flex-1 text-center">Download</span>
                            </div>
                        </AluButton>
                        <select v-if="downloadIds.length > 0" v-model="selectedDownloadId"
                            class="w-full max-w-xs select select-bordered">
                            <option v-for="id in downloadIds" :key="id" :value="id">
                                {{ id }}
                            </option>
                        </select>
                    </div>

                    <div class="pt-8 card-body">
                        <!-- Stats section with left padding -->
                        <div class="w-full pl-64 mb-6">
                            <div class="max-w-3xl shadow stats stats-vertical lg:stats-horizontal bg-base-300">
                                <div class="stat">
                                    <div class="stat-title text-base-content/70">Title ID</div>
                                    <div class="font-mono text-lg stat-value text-base-content">{{ metadata.titleId }}
                                    </div>
                                </div>
                                <div class="stat">
                                    <div class="stat-title text-base-content/70">Release Date</div>
                                    <div class="text-lg stat-value text-base-content">{{
                                        dateFromYYYYMMDD(metadata.releaseDate).toLocaleDateString() }}</div>
                                </div>
                                <div class="stat">
                                    <div class="stat-title text-base-content/70">Categories</div>
                                    <div class="text-lg stat-value">
                                        <div class="flex flex-wrap gap-1">
                                            <div v-for="(category, index) in metadata.category" :key="index"
                                                class="badge badge-outline badge-accent">
                                                {{ category }}
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div id="description" class="whitespace-pre-line">
                            <p class="whitespace-pre-line text-base-content/70">{{ metadata.description }}</p>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        <div v-else class="flex items-center justify-center h-screen">
            <span class="loading loading-spinner loading-lg text-primary"></span>
        </div>
    </div>
</template>

<script>
import { dateFromYYYYMMDD } from '../util.js';
import AluButton from './AluButton.vue';
export default {
    components: {
        AluButton
    },
    data() {
        return {
            titleId: null,
            metadata: null,
            downloadIds: [],
            selectedDownloadId: "",
        };
    },
    methods: {
        dateFromYYYYMMDD,
        async downloadGame(downloadId) {
            try {
                window.location.href = `/api/get_game/${downloadId}`;
            } catch (error) {
                alert("Error downloading game: " + error);
            }
        },
        async fetchDownloadIds(titleId) {
            try {
                const response = await fetch(
                    `/api/title_meta/${titleId}/download_ids`,
                );
                if (!response.ok) {
                    throw new Error("Failed to fetch download IDs");
                }
                const data = await response.json();
                console.log(data);
                this.downloadIds = data;
                return data;
            } catch (error) {
                console.error("Error fetching download IDs:", error);
                this.downloadIds = [];
                return [];
            }
        },
    },
    async created() {
        this.titleId = this.$route.query.tid;
        if (this.titleId) {
            try {
                const response = await fetch(`/api/title_meta/${this.titleId}`);
                if (!response.ok) {
                    throw new Error("Failed to fetch metadata");
                }
                this.metadata = await response.json();

                await this.fetchDownloadIds(this.titleId);
                if (this.downloadIds.length > 0) {
                    this.selectedDownloadId = this.downloadIds[0];
                }
            } catch (error) {
                console.error("Error fetching metadata:", error);
            }
        }
    },
    watch: {
        async titleId(newTitleId) {
            if (newTitleId) {
                await this.fetchDownloadIds(newTitleId);
            }
        },
    },
};
</script>
