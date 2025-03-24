import { createRouter, createWebHistory } from "vue-router";
import GamesView from "../views/GamesView.vue";
import UsersView from "../views/UsersView.vue";
import MetadataView from "../views/MetadataView.vue";

const router = createRouter({
  history: createWebHistory("/"),
  routes: [
    // {
    //   path: "/",
    //   redirect: "/games",
    // },
    {
      path: "/",
      name: "games",
      component: GamesView,
    },
    {
      path: "/users",
      name: "users",
      component: UsersView,
    },
    {
      path: "/metadata",
      name: "metadata",
      component: MetadataView,
    },
  ],
});

export default router;
