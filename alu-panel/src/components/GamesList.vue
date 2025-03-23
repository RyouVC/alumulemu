<template>
    <div class="games-list">
        <button @click="rescanGames" class="rescan-button">Rescan Games</button>
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

const rescanGames = async () => {
    try {
        const response = await fetch("/admin/rescan", {
            method: "POST",
            credentials: "include",
            headers: {
                "Content-Type": "application/json",
            },
        });

        if (!response.ok) {
            throw new Error("Rescan failed");
        }
        await loadGames();
    } catch (error) {
        console.log(
            "%c YOUR ADMIN PANEL SUCKS",
            `
        font-weight: bold;
        font-size: 20px;
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
    }
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
