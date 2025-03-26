<template>
    <!-- fixed background -->
    <div v-if="metadata" class="fixed inset-0 h-screen bg-center bg-no-repeat bg-cover brightness-50 blur-xl opacity-70"
        :style="{ backgroundImage: `url(${metadata.bannerUrl})` }"></div>

    <div class="relative z-10">
        <div v-if="metadata" class="w-full">
            <div class="h-[25vh]"></div>

            <div class="relative">
                <img :src="metadata.iconUrl" :alt="metadata.name"
                    class="absolute z-20 object-cover w-48 h-48 rounded-lg shadow-xl left-8 -top-24" />

                <!-- Title outside the box -->
                <div class="absolute z-20 text-white left-64 -top-24">
                    <div class="flex items-center gap-4 mb-4">
                        <h1 class="text-4xl font-bold text-white">
                            {{ metadata.name }}
                        </h1>
                        <h1 class="text-4xl font-bold text-gray-300">
                            {{ metadata.releaseDate.slice(0, 4) }}
                        </h1>
                    </div>
                    <p class="text-xl">
                        {{ metadata.publisher }} |

                        {{
                            metadata.size > 1024 * 1024 * 1024
                                ? (
                                    metadata.size /
                                    (1024 * 1024 * 1024)
                                ).toFixed(2) + " GB"
                                : (metadata.size / (1024 * 1024)).toFixed(2) +
                                " MB"
                        }}
                    </p>
                </div>


                <div id="nutrition-label" class="absolute z-20 text-white left-64 top-6">
                    <div class="overflow-x-auto shadow-md bg-base-100 rounded-box">
                        <div class="stats stats-vertical lg:stats-horizontal shadow">
                            <div class="stat">
                                <div class="stat-title">Title ID</div>
                                <div class="stat-value text-lg">{{ metadata.titleId }}</div>
                            </div>
                            <div class="stat">
                                <div class="stat-title">Release Date</div>
                                <div class="stat-value text-lg">{{ dateFromYYYYMMDD(metadata.releaseDate).toLocaleDateString() }}</div>
                            </div>
                            <div class="stat">
                                <div class="stat-title">Categories</div>
                                <div class="stat-value text-lg">
                                    <div class="flex flex-wrap gap-1">
                                        <div v-for="(category, index) in metadata.category" :key="index" class="badge badge-outline badge-accent">
                                            {{ category }}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <div class="bg-gray-900 min-h-[70vh] relative pt-8 rounded-lg">
                    <div class="absolute z-20 flex flex-col items-center overflow-visible right-8 -top-6">
                        <AluButton
                        @click="downloadGame(selectedDownloadId || metadata.titleId)"
                        level="success"
                        size="medium"
                        class="btn-wide"
                        >
                            Download
                        </AluButton>
                    </div>
                    <div class="flex justify-end pr-8">
                        <select name="ids" id="ids" v-model="selectedDownloadId"
                            class="z-10 w-48 py-1 font-semibold text-white bg-gray-800 rounded-lg shadow-lg"
                            v-if="downloadIds.length > 0">
                            <option v-for="id in downloadIds" :key="id" :value="id">
                                {{ id }}
                            </option>
                        </select>
                    </div>
                    <div class="px-8 pt-20">
                        <div class="text-white">
                            <h2 class="mb-2 text-xl font-semibold">
                                Description
                            </h2>
                            <p class="whitespace-pre-line">
                                {{ metadata.description }}
                            </p>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        <div v-else class="text-white">Loading...</div>
    </div>
</template>

<script>
import { dateFromYYYYMMDD } from '../util.js'; // Import the utility function
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
        dateFromYYYYMMDD, // Make the imported function available to the template
        async downloadGame(downloadId) {
            try {
                window.location.href = `/api/get_game/${downloadId}`;
            } catch (error) {
                alert("Error downloading game: " + error);
            }
        },
        async getDownloadIds(titleId) {
            try {
                const response = await fetch(
                    `/api/title_meta/${titleId}/download_ids`,
                );
                if (!response.ok) {
                    throw new Error("Failed to fetch download IDs");
                }
                const data = await response.json();
                console.log(data);
                return data;
            } catch (error) {
                console.error("Error fetching download IDs:", error);
                return [];
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
                let box = document.getElementById("ids");
                if (box && this.downloadIds.length > 0) {
                    box.value = this.downloadIds[0];
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
