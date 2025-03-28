<template>
    <!-- fixed background -->
    <div v-if="metadata" class="fixed inset-0 h-screen bg-center bg-no-repeat bg-cover brightness-50 blur-xl opacity-70"
        :style="{ backgroundImage: `url(${metadata.bannerUrl})` }"></div>

    <div class="relative z-10">

        <div v-if="metadata" class="w-full px-4 md:px-8 lg:px-16">
            <div :class="{ 'h-48': isDesktop, 'h-4': !isDesktop }"></div>
            <!-- Increased spacing to show more background -->

            <!-- Floating elements container -->
            <div class="relative">
                <!-- Game icon - only shown on desktop -->
                <img :src="metadata.iconUrl" :alt="metadata.name"
                    class="absolute z-20 object-cover w-48 h-48 rounded-lg shadow-xl -top-24 left-8 ring-4 shadow-base-content/20 hidden lg:block" />

                <!-- Title and basic info - only shown on desktop -->
                <div class="absolute z-20 -top-24 left-64 hidden lg:block">
                    <h1 class="text-4xl font-bold text-white">
                        {{ metadata.name }}
                    </h1>
                    <div class="flex items-center gap-2 mt-2 text-xl text-white">
                        <span>{{ metadata.publisher }}</span>
                        <span class="opacity-50">{{ metadata.releaseDate.slice(0, 4) }}</span>
                        <span class="opacity-50">|</span>
                        <span>{{ formatFileSize(metadata.size) }}</span>
                    </div>
                </div>

                <!-- Dark card container -->
                <div class="w-full shadow-xl card bg-base-100">
                    <!-- Title and basic info for mobile -->
                    <div class="block lg:hidden p-4">
                        <h1 class="text-2xl font-bold text-white">
                            {{ metadata.name }}
                        </h1>
                        <div class="flex items-center gap-2 mt-2 text-lg text-white">
                            <span>{{ metadata.publisher }}</span>
                            <span class="opacity-50">{{ metadata.releaseDate.slice(0, 4) }}</span>
                            <span class="opacity-50">|</span>
                            <span>{{ formatFileSize(metadata.size) }}</span>
                        </div>

                        <!-- Game icon for mobile - placed below title -->
                        <div class="flex justify-center mt-4">
                            <img :src="metadata.iconUrl" :alt="metadata.name"
                                class="object-cover w-64 h-64 rounded-lg shadow-lg ring-2 shadow-base-content/20" />
                        </div>
                    </div>

                    <!-- Download section - positioned to overlap card -->
                    <div class="flex flex-col items-end gap-2 p-4 lg:absolute lg:z-20 lg:-top-6 lg:right-8">
                        <AluButton @click="
                            downloadGame(
                                selectedDownloadId || metadata.titleId,
                            )
                            " level="success" size="medium" class="shadow-lg shadow-success/10" style="width: 100%">
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
                        <!-- Stats section with left padding for desktop, full width for mobile -->
                        <div :class="{ 'pl-64': isDesktop, 'w-full': true, 'mb-6': true }">
                            <div class="max-w-3xl shadow stats stats-vertical lg:stats-horizontal bg-base-300">
                                <div class="stat" id="nutrition-titleid">
                                    <div class="stat-title text-base-content/70">
                                        Title ID
                                    </div>
                                    <div class="font-mono text-lg stat-value text-base-content">
                                        {{ metadata.titleId }}
                                    </div>
                                </div>
                                <div class="stat" id="nutrition-release-date">
                                    <div class="stat-title text-base-content/70">
                                        Release Date
                                    </div>
                                    <div class="text-lg stat-value text-base-content">
                                        {{
                                            dateFromYYYYMMDD(
                                                metadata.releaseDate,
                                            ).toLocaleDateString()
                                        }}
                                    </div>
                                </div>
                                <div class="stat" id="nutrition-categories">
                                    <div class="stat-title text-base-content/70">
                                        Categories
                                    </div>
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
                                    <div class="stat-title text-base-content/70">
                                        Rating
                                    </div>
                                    <div class="text-lg stat-value text-base-content">
                                        <AgeRating :rating="metadata.rating" />
                                    </div>
                                </div>
                            </div>
                        </div>

                        <!-- Screenshots Carousel -->
                        <div v-if="
                            metadata.screenshots &&
                            metadata.screenshots.length > 0
                        " class="w-full mb-8">
                            <div class="relative">
                                <div class="w-full overflow-hidden rounded-lg shadow-lg carousel"
                                    ref="carouselContainer" @mousedown="startDrag" @mousemove="onDrag"
                                    @mouseup="endDrag" @mouseleave="endDrag" @touchstart="startTouch"
                                    @touchmove="onTouch" @touchend="endTouch" @wheel="onWheel">
                                    <div class="flex w-full carousel-item">
                                        <div class="grid w-full grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-3">
                                            <div v-for="(screenshot, index) in visibleScreenshots" :key="index"
                                                class="relative overflow-hidden rounded-lg shadow-md aspect-video">
                                                <img :src="screenshot" class="object-cover w-full h-full" @click="
                                                    openFullScreenImage(
                                                        screenshot,
                                                    )
                                                    " />
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <div class="absolute inset-0 flex items-center justify-between p-4 pointer-events-none">
                                    <button @click="previousScreenshotSet"
                                        class="pointer-events-auto btn btn-circle bg-base-300/70 hover:bg-base-300">
                                        ❮
                                    </button>
                                    <button @click="nextScreenshotSet"
                                        class="pointer-events-auto btn btn-circle bg-base-300/70 hover:bg-base-300">
                                        ❯
                                    </button>
                                </div>

                                <div class="flex justify-center w-full gap-2 py-2 mt-2">
                                    <div class="flex gap-2">
                                        <button v-for="index in totalSets" :key="index"
                                            @click="currentSetIndex = index - 1" :class="[
                                                'btn btn-xs btn-circle',
                                                currentSetIndex === index - 1 ? 'btn-primary' : 'btn-ghost'
                                            ]">
                                            {{ index }}
                                        </button>
                                    </div>
                                    <span class="text-sm opacity-75 ml-2">
                                        Showing
                                        {{
                                            currentSetIndex * imagesPerSet + 1
                                        }}-{{
                                            Math.min(
                                                currentSetIndex * imagesPerSet +
                                                imagesPerSet,
                                                metadata.screenshots.length,
                                            )
                                        }}
                                        of {{ metadata.screenshots.length }}
                                    </span>
                                </div>
                            </div>

                            <!-- Fullscreen image modal -->
                            <div v-if="fullScreenImage"
                                class="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-75"
                                @click="fullScreenImage = null" @keydown="handleModalKeydown" tabindex="0"
                                ref="imageModal">
                                <div class="relative max-w-5xl p-4 flex flex-col items-center">
                                    <!-- Image container with hover effect for buttons -->
                                    <div class="relative image-container">
                                        <img :src="fullScreenImage" class="object-contain max-w-full max-h-[80vh]" />

                                        <!-- Navigation arrows - only visible on hover -->
                                        <div
                                            class="absolute inset-0 flex items-center justify-between opacity-0 hover-buttons">
                                            <button @click.stop="navigateFullscreenImage('prev')"
                                                class="btn btn-circle bg-base-300/30 hover:bg-base-300/60 ml-4">
                                                ❮
                                            </button>
                                            <button @click.stop="navigateFullscreenImage('next')"
                                                class="btn btn-circle bg-base-300/30 hover:bg-base-300/60 mr-4">
                                                ❯
                                            </button>

                                            <!-- Close button also hidden until hover -->
                                            <button
                                                class="absolute top-4 right-4 btn btn-circle btn-sm bg-base-300/30 hover:bg-base-300/60"
                                                @click.stop="fullScreenImage = null">
                                                ✕
                                            </button>
                                        </div>
                                    </div>

                                    <!-- Counter below the image -->
                                    <div class="mt-4 text-center text-white/80">
                                        {{ getCurrentImageIndex() + 1 }} / {{ metadata.screenshots.length }}
                                    </div>
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
                            <p class="whitespace-pre-line text-base-content/70">
                                {{ metadata.description }}
                            </p>
                        </div>

                        <div id="test-alulist">
                            <!-- <AluList>
                                <AluListRow :game="metadata"> </AluListRow>
                            </AluList> -->
                        </div>

                        <div class="pt-4" id="extra-info-container">
                            <h2 class="pb-4 text-2xl font-bold">
                                Additional Information
                            </h2>

                            <div class="grid gap-4 md:grid-cols-3">
                                <!-- Players -->
                                <div class="shadow stats stats-vertical bg-base-300" v-if="metadata.numberOfPlayers">
                                    <div class="stat">
                                        <div class="stat-title text-base-content/70">
                                            Players
                                        </div>
                                        <div class="text-lg stat-value">
                                            {{
                                                metadata.numberOfPlayers > 1
                                                    ? `1-${metadata.numberOfPlayers}`
                                                    : metadata.numberOfPlayers
                                            }}
                                        </div>
                                    </div>
                                </div>

                                <!-- Languages -->
                                <div class="shadow stats stats-vertical bg-base-300" v-if="
                                    metadata.languages &&
                                    metadata.languages.length
                                ">
                                    <div class="stat">
                                        <div class="stat-title text-base-content/70">
                                            Languages
                                        </div>
                                        <div class="text-lg stat-value">
                                            <div class="flex flex-wrap gap-1">
                                                <div v-for="(language, i) in metadata.languages" :key="i"
                                                    class="badge badge-outline badge-primary">
                                                    {{
                                                        getLanguageName(
                                                            language,
                                                        )
                                                    }}
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                <!-- Content Ratings -->
                                <div class="shadow stats stats-vertical bg-base-300">
                                    <div class="stat" v-if="metadata.ageRating">
                                        <div class="stat-title text-base-content/70">
                                            Age Rating
                                        </div>
                                        <div class="text-lg stat-value">
                                            <AgeRating :age-rating="metadata.ageRating" />
                                        </div>
                                    </div>
                                    <div class="stat" v-if="metadata.ratingContent && metadata.ratingContent.length">
                                        <div class="stat-title text-base-content/70">
                                            Content Warnings
                                        </div>
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

        <div v-else-if="isLoading" class="flex items-center justify-center h-screen">
            <span class="loading loading-spinner loading-lg text-primary"></span>
        </div>


        <div v-else class="flex items-center justify-center h-screen">
            <ErrorDisplay message="Failed to load game data" />
        </div>
    </div>
</template>

<script>
import { dateFromYYYYMMDD, formatFileSize, getLanguageName } from "../util.js";
import { TitleMetadata } from "@/utils/title.ts";
import AluList from "./alu/AluList.vue";
import AluListRow from "./alu/AluListRow.vue";
import AluButton from "./AluButton.vue";
import AgeRating from "./alu/AgeRating.vue";
import ErrorDisplay from "./ErrorDisplay.vue";

export default {
    components: {
        AluButton,
        AluListRow,
        AluList,
        AgeRating,
        ErrorDisplay
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
            windowWidth: window.innerWidth,
            // New props for swipe/scroll functionality
            dragStart: null,
            touchStartX: null,
            isDragging: false,
            swipeThreshold: 50, // Minimum swipe distance to trigger navigation
            wheelDebounceTimer: null,
            // Error handling
            errorMessage: "",
            isLoading: false
        };
    },
    computed: {
        visibleScreenshots() {
            if (!this.metadata || !this.metadata.screenshots) return [];
            const start = this.currentSetIndex * this.imagesPerSet;
            return this.metadata.screenshots.slice(
                start,
                start + this.imagesPerSet,
            );
        },
        totalSets() {
            if (!this.metadata || !this.metadata.screenshots) return 0;
            return Math.ceil(
                this.metadata.screenshots.length / this.imagesPerSet,
            );
        },
        isDesktop() {
            return this.windowWidth >= 1024;
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
                this.showError("Error downloading game: " + error.message || "Unknown error");
            }
        },
        // Replace the local fetchDownloadIds with a method that uses TitleMetadata.fetchDownloadIds
        async fetchDownloadIds(titleId) {
            try {
                const data = await TitleMetadata.fetchDownloadIds(titleId);
                console.log(data);
                this.downloadIds = data;
                return data;
            } catch (error) {
                console.error("Error fetching download IDs:", error);
                this.showError(`Error fetching download IDs: ${error.message || "Unknown error"}`);
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
            // Focus the modal after it's mounted to enable keyboard navigation
            this.$nextTick(() => {
                if (this.$refs.imageModal) {
                    this.$refs.imageModal.focus();
                }
            });
        },

        // Get the current index of the fullscreen image
        getCurrentImageIndex() {
            if (!this.fullScreenImage || !this.metadata || !this.metadata.screenshots) {
                return -1;
            }
            return this.metadata.screenshots.findIndex(url => url === this.fullScreenImage);
        },

        // Handle navigation in fullscreen mode
        navigateFullscreenImage(direction) {
            if (!this.metadata || !this.metadata.screenshots || !this.fullScreenImage) return;

            const currentIndex = this.getCurrentImageIndex();
            if (currentIndex === -1) return;

            let newIndex;
            if (direction === 'next') {
                newIndex = (currentIndex + 1) % this.metadata.screenshots.length;
            } else {
                newIndex = (currentIndex - 1 + this.metadata.screenshots.length) % this.metadata.screenshots.length;
            }

            this.fullScreenImage = this.metadata.screenshots[newIndex];
        },

        // Handle keyboard events for the modal
        handleModalKeydown(event) {
            // Navigate with arrow keys
            if (event.key === 'ArrowRight') {
                this.navigateFullscreenImage('next');
            } else if (event.key === 'ArrowLeft') {
                this.navigateFullscreenImage('prev');
            } else if (event.key === 'Escape') {
                this.fullScreenImage = null;
            }

            // Prevent page scrolling when using arrow keys in modal
            if (['ArrowRight', 'ArrowLeft', 'ArrowUp', 'ArrowDown'].includes(event.key)) {
                event.preventDefault();
            }
        },

        // Mouse drag handlers
        startDrag(event) {
            if (event.button !== 0) return; // Only respond to left mouse button
            this.isDragging = true;
            this.dragStart = event.clientX;
            event.preventDefault(); // Prevent text selection during drag
        },
        onDrag(event) {
            if (!this.isDragging || !this.dragStart) return;
            const diffX = event.clientX - this.dragStart;

            // Detect if drag is significant enough
            if (Math.abs(diffX) > this.swipeThreshold) {
                if (diffX > 0) {
                    this.previousScreenshotSet();
                } else {
                    this.nextScreenshotSet();
                }
                this.isDragging = false;
                this.dragStart = null;
            }
        },
        endDrag() {
            this.isDragging = false;
            this.dragStart = null;
        },
        // Touch handlers for mobile
        startTouch(event) {
            this.touchStartX = event.touches[0].clientX;
        },
        onTouch(event) {
            if (!this.touchStartX) return;

            const currentX = event.touches[0].clientX;
            const diffX = currentX - this.touchStartX;

            // Detect if swipe is significant enough
            if (Math.abs(diffX) > this.swipeThreshold) {
                if (diffX > 0) {
                    this.previousScreenshotSet();
                } else {
                    this.nextScreenshotSet();
                }
                this.touchStartX = null;
            }
        },
        endTouch() {
            this.touchStartX = null;
        },
        // Mouse wheel handler
        onWheel(event) {
            // Clear any existing timer
            if (this.wheelDebounceTimer) {
                clearTimeout(this.wheelDebounceTimer);
            }

            // Debounce wheel events to prevent rapid-fire navigation
            this.wheelDebounceTimer = setTimeout(() => {
                if (event.deltaY > 0) {
                    this.nextScreenshotSet();
                } else {
                    this.previousScreenshotSet();
                }
            }, 100);
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
        updateResponsiveLayout() {
            this.$nextTick(() => {
                const statsSection = document.querySelector('.stats');
                if (!statsSection) return; // Ensure the element exists

                if (this.windowWidth < 768) {
                    statsSection.classList.remove('lg:stats-horizontal');
                    statsSection.classList.add('stats-vertical');
                } else {
                    statsSection.classList.remove('stats-vertical');
                    statsSection.classList.add('lg:stats-horizontal');
                }
            });
        },
        handleResize() {
            this.windowWidth = window.innerWidth;
            this.updateImagesPerSet();
            this.updateResponsiveLayout();
        },
        // Error handling methods
        showError(message) {
            this.errorMessage = message;
            // Auto-dismiss after 10 seconds
            setTimeout(() => {
                if (this.errorMessage === message) {
                    this.errorMessage = "";
                }
            }, 10000);
        },

        retryLoading() {
            if (this.titleId) {
                this.loadMetadata(this.titleId);
            }
        },

        async loadMetadata(titleId) {
            this.isLoading = true;
            this.errorMessage = "";
            try {
                // Use the TitleMetadata.fetchById method instead of direct fetch
                this.metadata = await TitleMetadata.fetchMetaViewById(titleId);
                await this.fetchDownloadIds(titleId);
                if (this.downloadIds.length > 0) {
                    this.selectedDownloadId = this.downloadIds[0];
                }
            } catch (error) {
                console.error("Error fetching metadata:", error);
                this.metadata = null;
                this.showError(`Failed to load game data: ${error.message || "Unknown error"}`);
            } finally {
                this.isLoading = false;
            }
        },
    },
    async created() {
        this.updateImagesPerSet();

        this.titleId = this.$route.query.tid;
        if (this.titleId) {
            this.loadMetadata(this.titleId);
        }
    },
    mounted() {
        window.addEventListener('resize', this.handleResize);
        this.updateResponsiveLayout();
    },
    watch: {
        async titleId(newTitleId) {
            if (newTitleId) {
                this.loadMetadata(newTitleId);
            }
        },
    },
    unmounted() {
        // Clean up event listeners
        window.removeEventListener("resize", this.handleResize);
    },
};
</script>

<style scoped>
/* Cursor styles for carousel interaction */
.carousel {
    cursor: grab;
}

.carousel:active {
    cursor: grabbing;
}

/* Make modal focusable without showing outline */
[tabindex="0"]:focus {
    outline: none;
}

/* Style for image container with hover buttons */
.image-container {
    position: relative;
    overflow: hidden;
}

/* Show buttons on hover */
.image-container:hover .hover-buttons {
    opacity: 1;
    transition: opacity 0.3s ease;
}

.hover-buttons {
    transition: opacity 0.3s ease;
}
</style>
