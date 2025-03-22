<template>
    <div class="games-list">
        <div v-for="game in games" :key="game.url" class="game-item">
            <h3>{{ getGameTitle(game.url) }}</h3>
            <p>Size: {{ (game.size / (1024 * 1024)).toFixed(2) }} MB</p>
            <button @click="downloadGame(getGameTitleId(game.url))">
                Download
            </button>
        </div>
    </div>
</template>

<script setup>
import { ref, onMounted } from "vue";

const games = ref([]);
// you wanna touch me but i'm not tangible, ears on my head cause i'm an animal
const getGameTitle = (url) => {
    const titleMatch = url.match(/#(.*?)\[/);
    return titleMatch ? titleMatch[1].trim() : "Unknown Title";
};

const getGameTitleId = (url) => {
    const titleIdMatch = url.match(/\[(.*?)\]/);
    return titleIdMatch ? titleIdMatch[1] : "";
};

const loadGames = async () => {
    try {
        const response = await fetch("/");
        const data = await response.json();
        games.value = data.files || [];
    } catch (error) {
        console.error("Error loading games:", error);
    }
};

const downloadGame = async (titleId) => {
    try {
        window.location.href = `/api/get_game/${titleId}`;
    } catch (error) {
        alert("Error downloading game: " + error);
    }
};

onMounted(() => {
    loadGames();
});
</script>

<style scoped>
.game-item {
    margin-bottom: 1rem;
    padding: 1rem;
    border: 1px solid #ddd;
    border-radius: 4px;
}
</style>
