<template>
  <div>
    <nav class="fixed top-0 left-0 right-0 z-50 p-4 bg-gray-800 mb-5">
      <div class="flex items-center">
        <h1
          class="font-bold text-white no-underline mx-2.5 router-link-active:text-green-500"
        >
          Alumulemu |
        </h1>

        <RouterLink
          to="/"
          class="font-bold text-gray-800 no-underline mx-2.5 router-link-active:text-green-500"
          >Games</RouterLink
        >

        <RouterLink
          to="/users"
          class="font-bold text-gray-800 no-underline mx-2.5 router-link-active:text-green-500"
          >Users</RouterLink
        >
        <RouterLink
          to="/titledb"
          class="font-bold text-gray-800 no-underline mx-2.5 router-link-active:text-green-500"
          >TitleDB</RouterLink
        >
        <RouterLink
          to="/downloads"
          class="font-bold text-gray-800 no-underline mx-2.5 router-link-active:text-green-500"
          >Downloads</RouterLink
        >
      </div>
    </nav>

    <div class="mt-24">
      <RouterView />
    </div>
  </div>
</template>

<script setup>
import { h } from "vue";
import { RouterLink, RouterView } from "vue-router";
</script>
