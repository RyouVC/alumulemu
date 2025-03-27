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
                        <span>{{ formatFileSize(metadata.size) }}</span>
                    </div>
                </div>

                <!-- Dark card container -->
                <div class="w-full shadow-xl card bg-base-100">
                    <!-- Download section - positioned to overlap card -->
                    <div class="absolute z-20 flex flex-col items-end gap-2 -top-6 right-8">
                        <AluButton @click="downloadGame(selectedDownloadId || metadata.titleId)" level="success"
                            size="medium" class="shadow-lg shadow-success/10" style="width: 100%;">
                            <div class="flex items-center w-full">
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"
                                    stroke-width="1.5" stroke="currentColor" class="mr-2 size-6">
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
                                <div class="stat" id="nutrition-titleid">
                                    <div class="stat-title text-base-content/70">Title ID</div>
                                    <div class="font-mono text-lg stat-value text-base-content">{{ metadata.titleId }}
                                    </div>
                                </div>
                                <div class="stat" id="nutrition-release-date">
                                    <div class="stat-title text-base-content/70">Release Date</div>
                                    <div class="text-lg stat-value text-base-content">{{
                                        dateFromYYYYMMDD(metadata.releaseDate).toLocaleDateString() }}</div>
                                </div>
                                <div class="stat" id="nutrition-categories">
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
                                <div class="stat" id="nutrition-rating">
                                    <div class="stat-title text-base-content/70">Rating</div>
                                    <div class="text-lg stat-value text-base-content">
                                        <AgeRating :rating="metadata.rating" />
                                    </div>
                                </div>
                            </div>
                        </div>

                        <!-- Screenshots Carousel -->
                        <div v-if="metadata.screenshots && metadata.screenshots.length > 0" class="w-full mb-8">
                            <div class="relative">
                                <div class="w-full overflow-hidden rounded-lg shadow-lg carousel">
                                    <div class="flex w-full carousel-item">
                                        <div class="grid w-full grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
                                            <div v-for="(screenshot, index) in visibleScreenshots" :key="index"
                                                class="relative overflow-hidden rounded-lg shadow-md aspect-video">
                                                <img :src="screenshot" class="object-cover w-full h-full"
                                                    @click="openFullScreenImage(screenshot)" />
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <div class="absolute inset-0 flex items-center justify-between p-4 pointer-events-none">
                                    <button @click="previousScreenshotSet"
                                        class="pointer-events-auto btn btn-circle bg-base-300/70 hover:bg-base-300">❮</button>
                                    <button @click="nextScreenshotSet"
                                        class="pointer-events-auto btn btn-circle bg-base-300/70 hover:bg-base-300">❯</button>
                                </div>

                                <div class="flex justify-center w-full gap-2 py-2 mt-2">
                                    <span class="text-sm opacity-75">
                                        Showing {{ currentSetIndex * imagesPerSet + 1 }}-{{ Math.min(currentSetIndex *
                                            imagesPerSet + imagesPerSet, metadata.screenshots.length) }}
                                        of {{ metadata.screenshots.length }}
                                    </span>
                                </div>
                            </div>

                            <!-- Fullscreen image modal -->
                            <div v-if="fullScreenImage"
                                class="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-90"
                                @click="fullScreenImage = null">
                                <div class="relative max-w-5xl max-h-screen p-4">
                                    <img :src="fullScreenImage" class="object-contain max-w-full max-h-[90vh]" />
                                    <button class="absolute top-4 right-4 btn btn-circle btn-sm"
                                        @click.stop="fullScreenImage = null">✕</button>
                                </div>
                            </div>
                        </div>

                        <div id="intro" class="my-4" v-if="metadata.intro && metadata.intro.trim()">
                            <blockquote
                                class="px-4 py-3 text-lg italic border-l-4 rounded-r-lg border-primary bg-base-200/50">
                                {{ metadata.intro }}
                            </blockquote>
                        </div>

                        <div id="description" class="whitespace-pre-line">
                            <p class="whitespace-pre-line text-base-content/70">{{ metadata.description }}</p>
                        </div>

                        <div id="test-alulist">
                            <!-- <AluList>
                                <AluListRow :game="metadata">
                                </AluListRow>
                            </AluList> -->
                        </div>

                        <div class="pt-4" id="extra-info-container">
                            <h2 class="pb-4 text-2xl font-bold">Additional Information</h2>

                            <div class="grid gap-4 md:grid-cols-3">
                                <!-- Players -->
                                <div class="shadow stats stats-vertical bg-base-300" v-if="metadata.numberOfPlayers">
                                    <div class="stat">
                                        <div class="stat-title text-base-content/70">Players</div>
                                        <div class="text-lg stat-value">
                                            {{ metadata.numberOfPlayers > 1 ? `1-${metadata.numberOfPlayers}` :
                                                metadata.numberOfPlayers }}
                                        </div>
                                    </div>
                                </div>

                                <!-- Languages -->
                                <div class="shadow stats stats-vertical bg-base-300"
                                    v-if="metadata.languages && metadata.languages.length">
                                    <div class="stat">
                                        <div class="stat-title text-base-content/70">Languages</div>
                                        <div class="text-lg stat-value">
                                            <div class="flex flex-wrap gap-1">
                                                <div v-for="(language, i) in metadata.languages" :key="i"
                                                    class="badge badge-outline badge-primary">
                                                    {{ getLanguageName(language) }}
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <!-- Content Ratings -->
                                <div class="shadow stats stats-vertical bg-base-300">
                                    <div class="stat" v-if="metadata.ageRating">
                                        <div class="stat-title text-base-content/70">Age Rating</div>
                                        <div class="text-lg stat-value">
                                            <AgeRating :age-rating="metadata.ageRating" />
                                        </div>
                                    </div>
                                    <div class="stat" v-if="metadata.ratingContent && metadata.ratingContent.length">
                                        <div class="stat-title text-base-content/70">Content Warnings</div>
                                        <div class="text-lg stat-value">
                                            <div class="flex flex-wrap gap-1">
                                                <div v-for="(warning, i) in metadata.ratingContent" :key="i"
                                                    class="badge badge-outline badge-warning">
                                                    {{ warning }}
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
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
import { dateFromYYYYMMDD, formatFileSize, getLanguageName } from '../util.js';
import AluList from './alu/AluList.vue';
import AluListRow from './alu/AluListRow.vue';
import AluButton from './AluButton.vue';
import AgeRating from './alu/AgeRating.vue';

export default {
    components: {
        AluButton,
        AluListRow,
        AluList,
        AgeRating
    },
    data() {
        return {
            titleId: null,
            metadata: null,
            downloadIds: [],
            selectedDownloadId: "",
            currentSetIndex: 0,
            imagesPerSet: 3,
            fullScreenImage: null,
            // Language map removed as it's now in util.js
        };
    },
    computed: {
        visibleScreenshots() {
            if (!this.metadata || !this.metadata.screenshots) return [];
            const start = this.currentSetIndex * this.imagesPerSet;
            return this.metadata.screenshots.slice(start, start + this.imagesPerSet);
        },
        totalSets() {
            if (!this.metadata || !this.metadata.screenshots) return 0;
            return Math.ceil(this.metadata.screenshots.length / this.imagesPerSet);
        }
    },
    methods: {
        dateFromYYYYMMDD,
        formatFileSize,
        getLanguageName,
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
        nextScreenshotSet() {
            if (this.currentSetIndex < this.totalSets - 1) {
                this.currentSetIndex++;
            } else {
                this.currentSetIndex = 0; // Loop back to start
            }
        },
        previousScreenshotSet() {
            if (this.currentSetIndex > 0) {
                this.currentSetIndex--;
            } else {
                this.currentSetIndex = this.totalSets - 1; // Loop to end
            }
        },
        openFullScreenImage(imageUrl) {
            this.fullScreenImage = imageUrl;
        },
        updateImagesPerSet() {
            // Adjust number of images based on screen width
            if (window.innerWidth < 768) {
                this.imagesPerSet = 1; // Mobile: 1 image
            } else if (window.innerWidth < 1024) {
                this.imagesPerSet = 2; // Tablet: 2 images
            } else {
                this.imagesPerSet = 3; // Desktop: 3 images
            }
        },
    },
    async created() {
        this.updateImagesPerSet();
        window.addEventListener('resize', this.updateImagesPerSet);

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
    unmounted() {
        // Clean up event listeners
        window.removeEventListener('resize', this.updateImagesPerSet);
    }
};
</script>
