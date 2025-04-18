<template>
  <!-- Main container with background and blur effect -->
  <div
    id="games-main-container"
    class="min-h-screen bg-gradient-to-br from-base-300/20 to-primary/10 flex flex-col items-center justify-start w-full p-5"
  >
    <!-- Content container with backdrop blur -->
    <div
      id="games-backdrop-container"
      class="backdrop-blur-sm w-full flex flex-col items-center"
    >
      <div
        id="games-content-wrapper"
        class="container px-4 pt-8 mx-auto mt-16 md:px-8 lg:px-16"
      >
        <!-- Header section with search and rescan button -->
        <div
          id="games-search-container"
          class="flex flex-col gap-4 py-8 md:flex-row w-full justify-start"
        >
          <div class="flex flex-col gap-4 md:flex-row md:items-center">
            <h1 class="text-2xl font-bold text-base-content">Games</h1>

            <!-- Search input with DaisyUI styling -->
            <div id="games-search-input" class="flex-1 max-w-md join">
              <input
                type="text"
                v-model="searchQuery"
                placeholder="Search games..."
                class="w-full input input-bordered join-item"
              />
              <button class="btn join-item">
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  class="w-5 h-5"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                  />
                </svg>
              </button>
            </div>

            <!-- Modified AluButton to handle shift click in the parent component -->
            <AluButton
              id="games-rescan-button"
              @click="handleRescanClick"
              level="success"
              size="small"
              variant="soft"
              :loading="isScanning"
              :disabled="isScanning"
            >
              {{ isScanning ? "Scanning..." : "Rescan Games" }}
            </AluButton>
          </div>
        </div>

        <!-- Game content area -->
        <div id="games-content-area" class="flex flex-col items-center w-full">
          <!-- Status indicator -->
          <div
            id="games-error-message"
            v-if="loadingError"
            class="mb-4 alert alert-error w-full max-w-screen-2xl mx-auto"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              class="w-6 h-6 stroke-current shrink-0"
              fill="none"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"
              />
            </svg>
            <span>{{ loadingError }}</span>
          </div>

          <!-- Loading indicator - Only show while loading -->
          <div
            id="games-loading-indicator"
            v-if="isLoading"
            class="flex justify-center my-8 w-full"
          >
            <span
              class="loading loading-spinner loading-lg text-primary"
            ></span>
          </div>

          <!-- Games grid with DaisyUI responsive classes - Updated with more columns for smaller screens -->
          <div
            id="games-grid"
            v-if="games.length > 0"
            class="grid grid-cols-2 gap-3 xs:grid-cols-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 2xl:grid-cols-7 justify-items-center w-full max-w-screen-2xl mx-auto sm:gap-4 md:gap-5 lg:gap-6"
          >
            <GameTitleButton
              v-for="game in games"
              :key="game.titleId"
              :game="game"
              @get-metadata="getMetadata"
              class="transition-colors duration-200 shadow-lg card bg-base-200 hover:bg-base-300"
            />
          </div>

          <!-- Empty state -->
          <div
            id="games-empty-state"
            v-if="games.length === 0 && !isLoading && !loadingError"
            class="p-6 my-8 text-center card bg-base-200 max-w-md mx-auto"
          >
            <h3 class="text-xl font-bold">No games found</h3>
            <p class="text-base-content/70">
              {{
                searchQuery
                  ? "Try a different search term"
                  : "No games found in your library"
              }}
            </p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, watch } from "vue";
import GameTitleButton from "./GameTitleButton.vue";
import AluButton from "./AluButton.vue";
import { SearchQuery, TitleMetadata } from "@/utils/title.ts";

const isScanning = ref(false);
const isLoading = ref(true);
const games = ref([]);
const searchQuery = ref("");

const loadingError = ref(null);

let searchTimeout = null;

const loadGames = async () => {
  try {
    isLoading.value = true;
    loadingError.value = null;

    let result;
    if (searchQuery.value.trim()) {
      // Use our renamed searchAvailableGames method
      const query = new SearchQuery(searchQuery.value);
      result = await TitleMetadata.searchAvailableGames(query);
    } else {
      // Use the fetchBaseGames method for the main list
      result = await TitleMetadata.fetchBaseGames();
    }

    games.value = result;
  } catch (error) {
    console.error("Error loading games:", error);
    loadingError.value = error.message;
    games.value = [];
  } finally {
    isLoading.value = false;
  }
};

watch(searchQuery, () => {
  if (searchTimeout) {
    clearTimeout(searchTimeout);
  }

  searchTimeout = setTimeout(() => {
    loadGames();
  }, 300);
});

// New handler to differentiate between regular and shift clicks
const handleRescanClick = (event) => {
  if (event.shiftKey) {
    forceRescanGames();
  } else {
    rescanGames();
  }
};

const rescanGames = async () => {
  isScanning.value = true;
  try {
    // Use our new rescanGames method
    await TitleMetadata.rescanGames(false);
    await loadGames();
  } catch (error) {
    console.log(
      "%c YOUR ADMIN PANEL SUCKS",
      `
        font-weight: bold;
        font-size: 72px;
        background: linear-gradient(90deg, red, orange, yellow, green, blue, indigo, violet);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow:
          1px 1px 0 #ff0000,
          2px 2px 0 #ff7f00,
          3px 3px 0 #ffff00,
          4px 4px 0 #00ff00,
          5px 5px 0 #0000ff,
          6px 6px 0 #4b0082,
          7px 7px 0 #8f00ff;
      `,
    );
    console.error("Error:", error);
  } finally {
    isScanning.value = false;
  }
};

const forceRescanGames = async () => {
  isScanning.value = true;
  console.log("Force rescan games");
  try {
    // Use our new rescanGames method with the force parameter
    await TitleMetadata.rescanGames(true);
    await loadGames();
  } catch (error) {
    console.log(
      "%c YOUR ADMIN PANEL SUCKS",
      `
        font-weight: bold;
        font-size: 72px;
        background: linear-gradient(90deg, red, orange, yellow, green, blue, indigo, violet);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
        text-shadow:
          1px 1px 0 #ff0000,
          2px 2px 0 #ff7f00,
          3px 3px 0 #ffff00,
          4px 4px 0 #00ff00,
          5px 5px 0 #0000ff,
          6px 6px 0 #4b0082,
          7px 7px 0 #8f00ff;
      `,
    );
    console.error("Error:", error);
  } finally {
    isScanning.value = false;
  }
};

const getMetadata = async (titleId) => {
  try {
    window.location.href = `/metadata?tid=${titleId}`;
  } catch (error) {
    alert("Error finding metadata for game: " + error);
  }
};

onMounted(() => {
  loadGames();
});
</script>
