<template>
    <div id="app">
        <nav>
            <RouterLink to="/games">Games</RouterLink> |
            <RouterLink to="/users">Users</RouterLink>
        </nav>

        <RouterView />
    </div>
</template>

<script setup>
import { RouterLink, RouterView } from "vue-router";
</script>

<style>
#app {
    padding: 20px;
}

nav {
    padding: 1rem;
    background: #f5f5f5;
    margin-bottom: 20px;
}

nav a {
    font-weight: bold;
    color: #2c3e50;
    text-decoration: none;
    margin: 0 10px;
}

nav a.router-link-active {
    color: #42b983;
}
</style>
