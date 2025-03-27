<template>
    <!-- Main container with background and blur effect -->
    <div class="min-h-screen bg-gradient-to-br from-base-300/20 to-primary/10">
        <!-- Content container with backdrop blur -->
        <div class="backdrop-blur-sm">
            <div class="container px-4 pt-8 mx-auto mt-16 md:px-8 lg:px-16">
                <!-- Header section with search -->
                <div class="flex flex-col gap-4 py-8 md:flex-row md:items-center">
                    <h1 class="text-2xl font-bold text-base-content">
                        Title Database
                    </h1>

                    <!-- Search input with DaisyUI styling -->
                    <div class="flex-1 max-w-md join">
                        <input type="text" v-model="searchQuery" placeholder="Search title database..."
                            class="w-full input input-bordered join-item" />
                        <button class="btn join-item">
                            <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" fill="none" viewBox="0 0 24 24"
                                stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                    d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                            </svg>
                        </button>
                    </div>
                </div>

                <!-- Status indicator -->
                <div v-if="loadingError" class="mb-4 alert alert-error">
                    <svg xmlns="http://www.w3.org/2000/svg" class="w-6 h-6 stroke-current shrink-0" fill="none"
                        viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                            d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <span>{{ loadingError }}</span>
                </div>

                <!-- Loading indicator - Only show while loading -->
                <div v-if="isLoading" class="flex justify-center my-8">
                    <span class="loading loading-spinner loading-lg text-primary"></span>
                </div>

                <!-- Games grid with DaisyUI responsive classes -->
                <div v-if="titles.length > 0">
                    <AluList>
                        <AluListRow v-for="title in titles" :game="title" />
                    </AluList>
                </div>

                <!-- Empty state -->
                <div v-if="titles.length === 0 && !isLoading && !loadingError"
                    class="p-6 my-8 text-center card bg-base-200">
                    <h3 class="text-xl font-bold">No titles found</h3>
                    <p class="text-base-content/70">
                        {{
                            searchQuery
                                ? "Try a different search term"
                                : "Start typing to search the title database"
                        }}
                    </p>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup>
import { ref, onMounted, watch } from "vue";
import GameTitleButton from "./GameTitleButton.vue";
import AluList from "./alu/AluList.vue";
import AluListRow from "./alu/AluListRow.vue";
import { SearchQuery, TitleMetadata } from '../title.ts';

const isLoading = ref(false);
const titles = ref([]);
const searchQuery = ref("");
const loadingError = ref(null);

let searchTimeout = null;

const searchTitles = async () => {
    try {
        // Don't search if less than 2 characters
        if (searchQuery.value.trim().length < 2) {
            titles.value = [];
            return;
        }
        console.log("Searching titles with query:", searchQuery.value);

        isLoading.value = true;
        loadingError.value = null;

        // Use the SearchQuery class and TitleMetadata.searchGames method
        const query = new SearchQuery(searchQuery.value);
        const results = await TitleMetadata.searchAllGames(query);
        titles.value = results;
    } catch (error) {
        console.error("Error searching titles:", error);
        loadingError.value = error.message;
        titles.value = [];
    } finally {
        isLoading.value = false;
    }
};

watch(searchQuery, () => {
    if (searchTimeout) {
        clearTimeout(searchTimeout);
    }

    searchTimeout = setTimeout(() => {
        searchTitles();
    }, 300);
});

const getMetadata = async (titleId) => {
    try {
        window.location.href = `/metadata?tid=${titleId}`;
    } catch (error) {
        alert("Error finding metadata for title: " + error);
    }
};

onMounted(() => {
    // Don't load anything initially, wait for user to search
    titles.value = [];
});
</script>
